// Required for dynasm!
#![feature(proc_macro_hygiene)]

mod intrinsics;
mod macho;

use crate::{
    intrinsics::{sys_exit, sys_print},
    macho::{Assembly, CODE_START, ROM_START},
};
use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi, DynasmLabelApi};
use parser::Mir::{Declaration, Expression, Module};
use std::{
    error::Error,
    fs,
    fs::File,
    io::{prelude::*, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
};

// For Dynasm syntax see
// <https://censoredusername.github.io/dynasm-rs/language/langref_x64.html#register>

// Oluś default calling convention:
// r0: current closure pointer
// r1..r15: arguments

fn assemble_alloc(code: &mut Assembler, size: usize) {
    // Read current free memory pointer
    dynasm!(code
        ; mov r15d, DWORD [RAM_START]
    );

    // Add size to free memory pointer
    if size <= 255 {
        dynasm!(code
            ; add DWORD [RAM_START], BYTE size as i8
        );
    } else if size <= u32::max().into() {
        dynasm!(code
            ; add DWORD [RAM_START], DWORD size as i32
        );
    } else {
        panic!("Can not allocate more than 4GB.");
    }
}

fn assemble_decl(code: &mut Assembler, module: &Module, decl: &Declaration) {
    for (i, expr) in decl.call.iter().enumerate() {
        let literal = get_literal(module, expr);
        // TODO: Support taking values from arguments or closures (may need to use xchg)
        // TODO: Support creating closures
        dbg!(i, expr, literal);
        assemble_literal(code, i, literal);
    }
    // Make a closure call
    dynasm!(code
        ; jmp QWORD [r0]
    );
    // TODO: Support
    // * non closure jump `jmp r0`,
    // * constant jump `jmp OFFSET` and
    // * fall-through.
}

fn get_literal(module: &Module, expr: &Expression) -> u64 {
    match expr {
        Expression::Number(i) => module.numbers[*i],
        Expression::Symbol(i) => (ROM_START + *i * 8) as u64,
        Expression::Import(i) => (ROM_START + (module.symbols.len() + *i) * 8) as u64,
        Expression::Literal(i) => {
            let mut offset = ROM_START + (module.symbols.len() + module.imports.len()) * 8;
            for string in module.strings.iter().take(*i) {
                offset += 4 + string.len();
            }
            offset as u64
        }
    }
}

fn assemble_literal(code: &mut Assembler, reg: usize, literal: u64) {
    if literal <= u32::max_value().into() {
        let literal = literal as i32;
        match reg {
            0 => dynasm!(code; mov r0d, DWORD literal),
            1 => dynasm!(code; mov r1d, DWORD literal),
            2 => dynasm!(code; mov r2d, DWORD literal),
            3 => dynasm!(code; mov r3d, DWORD literal),
            4 => dynasm!(code; mov r4d, DWORD literal),
            5 => dynasm!(code; mov r5d, DWORD literal),
            6 => dynasm!(code; mov r6d, DWORD literal),
            7 => dynasm!(code; mov r7d, DWORD literal),
            8 => dynasm!(code; mov r8d, DWORD literal),
            9 => dynasm!(code; mov r9d, DWORD literal),
            10 => dynasm!(code; mov r10d, DWORD literal),
            11 => dynasm!(code; mov r11d, DWORD literal),
            12 => dynasm!(code; mov r12d, DWORD literal),
            13 => dynasm!(code; mov r13d, DWORD literal),
            14 => dynasm!(code; mov r14d, DWORD literal),
            15 => dynasm!(code; mov r15d, DWORD literal),
            _ => panic!("Unknown register"),
        }
    } else {
        let literal = literal as i64;
        match reg {
            0 => dynasm!(code; mov r0, QWORD literal),
            1 => dynasm!(code; mov r1, QWORD literal),
            2 => dynasm!(code; mov r2, QWORD literal),
            3 => dynasm!(code; mov r3, QWORD literal),
            4 => dynasm!(code; mov r4, QWORD literal),
            5 => dynasm!(code; mov r5, QWORD literal),
            6 => dynasm!(code; mov r6, QWORD literal),
            7 => dynasm!(code; mov r7, QWORD literal),
            8 => dynasm!(code; mov r8, QWORD literal),
            9 => dynasm!(code; mov r9, QWORD literal),
            10 => dynasm!(code; mov r10, QWORD literal),
            11 => dynasm!(code; mov r11, QWORD literal),
            12 => dynasm!(code; mov r12, QWORD literal),
            13 => dynasm!(code; mov r13, QWORD literal),
            14 => dynasm!(code; mov r14, QWORD literal),
            15 => dynasm!(code; mov r15, QWORD literal),
            _ => panic!("Unknown register"),
        }
    }
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
        ; mov QWORD[0x401ff8], rsp

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
        match import.as_ref() {
            "exit" => sys_exit(&mut code),
            "print" => sys_print(&mut code),
            _ => panic!("Unknown import"),
        }
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
    for decl in module.declarations.iter() {
        dynasm!(rom
            ; .qword (CODE_START + code_offsets[index]) as i64
        );
        index += 1;
    }
    for import in module.imports.iter() {
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
