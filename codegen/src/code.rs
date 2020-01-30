use crate::{
    allocator::{Allocator, Bump},
    intrinsic,
    macho::CODE_START,
    rom,
    utils::assemble_literal,
};
use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi};
use parser::mir::{Declaration, Expression, Module};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub struct Layout {
    pub declarations: Vec<usize>,
    pub imports:      Vec<usize>,
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
        let mut registers = [None; 16];
        for (index, expr) in exprs.iter().enumerate() {
            registers[index] = Some(expr.clone());
        }
        MachineState { registers }
    }
}

// Where to find a particular expression
enum Source {
    Constant(u64),
    Register(usize),
    Closure(usize), // Value from current closure
    Alloc(usize),   // New closure for decl
    None,
}

// struct Context<'a> {
//     module: &'a Module,
//     layout: &'a MemoryLayout,
//     code:   &'a mut Assembler,
//     state:  MachineState,
// }

// impl<'a> Context<'a> {
//     fn find_decl(&self, symbol: usize) -> Option<(usize, &'a Declaration)> {
//         self.module
//             .declarations
//             .iter()
//             .enumerate()
//             .find(|decl| decl.1.procedure[0] == symbol)
//     }

//     fn closure(&self) -> Vec<usize> {
//         if let Some(Expression::Symbol(s)) = self.state.registers[0] {
//             if let Some((_, decl)) = self.find_decl(s) {
//                 decl.closure.clone()
//             } else {
//                 panic!("r0 symbol is not a closure.")
//             }
//         } else {
//             panic!("r0 does not contain symbol.")
//         }
//     }

//     pub fn find(&self, expr: &Expression) -> Source {
//         use Expression::*;
//         use Source::*;
//         match expr {
//             Number(i) => Constant(self.module.numbers[*i]),
//             Literal(i) => Constant(self.layout.strings[*i] as u64),
//             Import(i) => Constant(self.layout.imports[*i] as u64),
//             Symbol(i) => {
//                 // Check registers
//                 if let Some(i) = self
//                     .state
//                     .registers
//                     .iter()
//                     .position(|e| e == &Some(expr.clone()))
//                 {
//                     return Register(i);
//                 }

//                 // Check current closure
//                 if let Some(i) = self.closure().iter().position(|s| s == i) {
//                     return Closure(i);
//                 }

//                 // New closure
//                 if let Some((i, decl)) = self.find_decl(*i) {
//                     if decl.closure.is_empty() {
//                         // Empty closures are constant allocated
//                         Constant(self.layout.closures[i] as u64)
//                     } else {
//                         // We need to allocate a closure
//                         Alloc(i)
//                     }
//                 } else {
//                     None
//                 }
//             }
//         }
//     }
// }

fn get_literal(module: &Module, rom_layout: &rom::Layout, expr: &Expression) -> Option<u64> {
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
            rom_layout.closures[*i] as u64
        }
        Expression::Import(i) => rom_layout.imports[*i] as u64,
        Expression::Literal(i) => rom_layout.strings[*i] as u64,
    })
}

fn code_transition(
    module: &Module,
    rom_layout: &rom::Layout,
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
            if let Some(literal) = get_literal(module, rom_layout, expr) {
                println!("{:?} is literal {:?}", expr, literal);
                assemble_literal(code, i, literal);
            } else if let Expression::Symbol(s) = expr {
                if let Some(reg) = current
                    .registers
                    .iter()
                    .position(|p| *p == Some(expr.clone()))
                {
                    println!("{:?} is arg {:?}", expr, reg);
                } else if let Some(var) = current
                    .registers
                    .iter()
                    .position(|p| *p == Some(expr.clone()))
                {
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

fn assemble_decl(
    module: &Module,
    rom_layout: &rom::Layout,
    code: &mut Assembler,
    decl: &Declaration,
) {
    // Transition into the correct machine state
    let current = MachineState::from_symbols(&decl.procedure);
    let target = MachineState::from_expressions(&decl.call);
    code_transition(module, rom_layout, code, &current, &target);

    // Call the closure
    dynasm!(code
        ; jmp QWORD [r0]
    );
    // TODO: Support
    // * non closure jump `jmp r0`,
    // * constant jump `jmp OFFSET` and
    // * fall-through.
}

pub fn compile(module: &Module, rom_layout: &rom::Layout) -> (Vec<u8>, Layout) {
    let mut layout = Layout::default();
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
        ; mov r0d, DWORD (rom_layout.closures[main_index]) as i32
        ; jmp QWORD [r0]
    );
    // Declarations
    for decl in &module.declarations {
        layout.declarations.push(CODE_START + code.offset().0);
        assemble_decl(module, rom_layout, &mut code, decl);
    }
    // Intrinsic functions
    for import in &module.imports {
        layout.imports.push(CODE_START + code.offset().0);
        intrinsic(&mut code, import);
    }
    let code = code.finalize().expect("Finalize after commit.");
    (code.to_vec(), layout)
}
