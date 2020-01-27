#![forbid(unsafe_code)]
#![warn(
    // Enable sets of warnings
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    rust_2018_idioms,
    future_incompatible,
    unused,

    // Additional unused warnings (not included in `unused`)
    unused_lifetimes,
    unused_qualifications,
    unused_results,

    // Additional misc. warnings
    anonymous_parameters,
    deprecated_in_future,
    elided_lifetimes_in_paths,
    explicit_outlives_requirements,
    keyword_idents,
    macro_use_extern_crate,
    // TODO: missing_docs,
    missing_doc_code_examples,
    private_doc_tests,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    variant_size_differences
)]
// Required for dynasm!
#![feature(proc_macro_hygiene)]
#![feature(const_in_array_repeat_expressions)]

mod intrinsics;
mod macho;
mod memory;
mod utils;

use crate::{
    intrinsics::intrinsic,
    macho::{Assembly, CODE_START, ROM_START},
    memory::{Allocator, Bump},
    utils::assemble_literal,
};
use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi};
use parser::mir::{Declaration, Expression, Module};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::PathBuf};

// For Dynasm syntax see
// <https://censoredusername.github.io/dynasm-rs/language/langref_x64.html#register>

// Oluś default calling convention:
// r0: current closure pointer
// r1..r15: arguments

// TODO: Two phase: first lay out code, then

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
struct MemoryLayout {
    code:     Vec<usize>,
    closures: Vec<usize>,
    imports:  Vec<usize>,
    strings:  Vec<usize>,
}

impl MemoryLayout {
    fn from_module(module: &Module) -> MemoryLayout {
        let mut result = MemoryLayout::default();
        let mut offset: usize;

        // Compute code layout by generating mock code
        // (correct size, but wrong values in pointers)

        // Compute const closure and import closure layout
        offset = ROM_START;
        for _decl in module.declarations {
            result.closures.push(offset);
            offset += 8;
        }
        for _import in module.imports {
            result.imports.push(offset);
            offset += 8;
        }
        for string in module.strings {
            result.closures.push(offset);
            offset += 4 + string.len();
        }
        result
    }
}

struct MachineState {
    registers: [Option<Expression>; 16],
    // TODO: Flags
}

impl MachineState {
    fn from_symbols(symbols: &[usize]) -> MachineState {
        assert!(symbols.len() <= 16);
        let mut registers = [None; 16];
        for (index, symbol) in symbols.iter().enumerate() {
            registers[index] = Some(Expression::Symbol(*symbol));
        }
        MachineState { registers }
    }

    fn from_expressions(exprs: &[Expression]) -> MachineState {
        assert!(exprs.len() <= 16);
        let mut registers = [Option::<Expression>::None; 16];
        for (index, expr) in exprs.iter().enumerate() {
            registers[index] = Some(expr.clone());
        }
        MachineState { registers }
    }
}

fn code_transition(
    module: &Module,
    code: &mut Assembler,
    current: &MachineState,
    target: &MachineState,
) {
    // Rough order:
    // * Drop registers? (depends on type)
    // * Shuffle registers? (Can also have duplicates and drops)
    // * Create closures
    // * Load closure values
    // * Load all the literals
    // * Copy registers
    for (i, expr) in target.registers.iter().enumerate() {
        if let Some(expr) = expr {
            println!("r{} = {:?}", i, expr);
            if let Some(literal) = get_literal(module, expr) {
                println!("{:?} is literal {:?}", expr, literal);
                assemble_literal(code, i, literal);
            } else if let Expression::Symbol(s) = expr {
                if let Some(reg) = current.registers.iter().position(|p| *p == Some(*expr)) {
                    println!("{:?} is arg {:?}", expr, reg);
                } else if let Some(var) = current.registers.iter().position(|p| *p == Some(*expr)) {
                    println!("{:?} is closure param {:?}", expr, var);
                } else if module.names[*s] {
                    let cdecl = module
                        .declarations
                        .iter()
                        .find(|decl| decl.procedure[0] == *s)
                        .unwrap();
                    assert!(!cdecl.closure.is_empty());
                    println!(
                        "{:?} is a closure of {:?}{:?}",
                        expr, cdecl.procedure[0], cdecl.closure
                    );
                    //
                    Bump::alloc(code, i, (1 + cdecl.closure.len()) * 8);
                } else {
                    panic!("Can't handle symbol");
                }
            } else {
                panic!("Can't handle expression");
            }
        }
    }
}

fn assemble_decl(code: &mut Assembler, module: &Module, decl: &Declaration) {
    // Transition into the correct machine state
    let current = MachineState::from_symbols(decl.symbols);
    let target = MachineState::from_expressions(decl.call);
    code_transition(module, code, current, target);

    // Call the closure
    dynasm!(code
        ; jmp QWORD [r0]
    );
    // TODO: Support
    // * non closure jump `jmp r0`,
    // * constant jump `jmp OFFSET` and
    // * fall-through.
}

fn get_literal(module: &Module, expr: &Expression) -> Option<u64> {
    Some(match expr {
        Expression::Number(i) => module.numbers[*i],
        Expression::Symbol(i) => {
            if !module.names[*i] {
                // Not a name (must be argument or closure content)
                return None;
            }
            let decl = module
                .declarations
                .iter()
                .find(|decl| decl.procedure[0] == *i)
                .unwrap();
            if !decl.closure.is_empty() {
                // Symbol requires a closure
                return None;
            }
            (ROM_START + *i * 8) as u64
        }
        Expression::Import(i) => (ROM_START + (module.symbols.len() + *i) * 8) as u64,
        Expression::Literal(i) => {
            let mut offset = ROM_START + (module.symbols.len() + module.imports.len()) * 8;
            for string in module.strings.iter().take(*i) {
                offset += 4 + string.len();
            }
            offset as u64
        }
    })
}

pub fn compile_code(module: &Module) -> (Vec<u8>, Vec<usize>) {
    let mut offsets = Vec::default();
    let mut code = dynasmrt::x64::Assembler::new().unwrap();

    let main_index = module
        .symbols
        .iter()
        .position(|s| s == "main")
        .expect("No main found.");

    dynasm!(code
        // Prelude, write rsp to RAM[END-8]. End of ram is initialized with with
        // the OS provided stack frame.
        // TODO: Replace constant with expression
        ; mov QWORD[0x0040_1ff8], rsp

        // Jump to closure at rom zero
        // TODO: Lookup closure with name `main`
        ; mov r0d, DWORD (ROM_START + main_index * 8) as i32
        ; jmp QWORD [r0]
    );
    // Declarations
    for decl in module.declarations.iter() {
        offsets.push(code.offset().0);
        assemble_decl(&mut code, module, decl);
    }
    // Intrinsic functions
    for import in module.imports.iter() {
        offsets.push(code.offset().0);
        intrinsic(&mut code, import);
    }
    let code = code.finalize().expect("Finalize after commit.");
    (code.to_vec(), offsets)
}

pub fn compile_rom(module: &Module, code_offsets: Vec<usize>) -> Vec<u8> {
    assert_eq!(
        code_offsets.len(),
        module.declarations.len() + module.imports.len()
    );
    // Assemble rom
    let mut rom = dynasmrt::x64::Assembler::new().unwrap();
    let mut index = 0;
    for _decl in module.declarations.iter() {
        dynasm!(rom
            ; .qword (CODE_START + code_offsets[index]) as i64
        );
        index += 1;
    }
    for _import in module.imports.iter() {
        dynasm!(rom
            ; .qword (CODE_START + code_offsets[index]) as i64
        );
        index += 1;
    }
    for string in module.strings.iter() {
        dynasm!(rom
            ; .dword string.len() as i32
            ; .bytes string.bytes()
        );
    }
    let rom = rom.finalize().expect("Finalize after commit.");
    rom.to_vec()
}

pub fn codegen(module: &Module, destination: &PathBuf) -> Result<(), Box<dyn Error>> {
    let (code, offsets) = compile_code(module);
    dbg!(&offsets);
    let rom = compile_rom(module, offsets);
    let ram = vec![];
    let assembly = Assembly { code, rom, ram };
    assembly.save(destination)
}
