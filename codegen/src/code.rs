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
pub(crate) struct Layout {
    pub(crate) declarations: Vec<usize>,
    pub(crate) imports:      Vec<usize>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
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
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
enum Source {
    Constant(u64),
    Register(usize),
    Closure(usize), // Value from current closure
    Alloc(usize),   // New closure for decl
    None,
}

struct Context<'a> {
    module: &'a Module,
    rom:    &'a rom::Layout,
    code:   &'a mut Assembler,
    state:  MachineState,
}

impl<'a> Context<'a> {
    fn find_decl(&self, symbol: usize) -> Option<(usize, &'a Declaration)> {
        self.module
            .declarations
            .iter()
            .enumerate()
            .find(|decl| decl.1.procedure[0] == symbol)
    }

    fn closure(&self) -> Vec<usize> {
        if let Some(Expression::Symbol(s)) = self.state.registers[0] {
            if let Some((_, decl)) = self.find_decl(s) {
                decl.closure.clone()
            } else {
                panic!("r0 symbol is not a closure.")
            }
        } else {
            panic!("r0 does not contain symbol.")
        }
    }

    pub fn find(&self, expr: &Expression) -> Source {
        use Expression::*;
        use Source::*;
        match expr {
            Number(i) => Constant(self.module.numbers[*i]),
            Literal(i) => Constant(self.rom.strings[*i] as u64),
            Import(i) => Constant(self.rom.imports[*i] as u64),
            Symbol(i) => {
                // Check registers
                if let Some(i) = self
                    .state
                    .registers
                    .iter()
                    .position(|e| e == &Some(expr.clone()))
                {
                    return Register(i);
                }

                // Check current closure
                if let Some(i) = self.closure().iter().position(|s| s == i) {
                    return Closure(i);
                }

                // New closure
                if let Some((i, decl)) = self.find_decl(*i) {
                    if decl.closure.is_empty() {
                        // Empty closures are constant allocated
                        Constant(self.rom.closures[i] as u64)
                    } else {
                        // We need to allocate a closure
                        Alloc(i)
                    }
                } else {
                    None
                }
            }
        }
    }
}

fn get_literal(module: &Module, rom_layout: &rom::Layout, expr: &Expression) -> Option<u64> {
    Some(match expr {
        Expression::Number(i) => module.numbers[*i],
        Expression::Symbol(i) => {
            if !module.names[*i] {
                // Not a name (must be argument or closure content)
                return None;
            }
            let (j, decl) = module
                .declarations
                .iter()
                .enumerate()
                .find(|(_, decl)| decl.procedure[0] == *i)
                .unwrap();
            if !decl.closure.is_empty() {
                // Symbol requires a closure
                return None;
            }
            rom_layout.closures[j] as u64
        }
        Expression::Import(i) => rom_layout.imports[*i] as u64,
        Expression::Literal(i) => rom_layout.strings[*i] as u64,
    })
}

fn code_transition(ctx: &mut Context, current: &MachineState, target: &MachineState) {
    let closure = if let Some(Expression::Symbol(i)) = current.registers[0] {
        let decl = ctx
            .module
            .declarations
            .iter()
            .find(|decl| decl.procedure[0] == i)
            .expect("r0 should be a closure if anything.");
        decl.closure.to_vec()
    } else {
        vec![]
    };

    // Rough order:
    // * Drop registers? (depends on type)
    // * Shuffle registers? (Can also have duplicates and drops)
    // * Create closures
    // * Load closure values
    // * Load all the literals
    // * Copy registers

    // Iterate target left to right
    for (i, expr) in target.registers.iter().enumerate() {
        if let Some(expr) = expr {
            println!("r{} = {:?}", i, expr);
            if let Some(literal) = get_literal(ctx.module, ctx.rom, expr) {
                println!("{:?} is literal {:?}", expr, literal);
                assemble_literal(ctx.code, i, literal);
            } else if let Expression::Symbol(s) = expr {
                if let Some(reg) = current
                    .registers
                    .iter()
                    .position(|p| *p == Some(expr.clone()))
                {
                    println!("{:?} is arg {:?}", expr, reg);
                } else if let Some(var) = closure.iter().position(|p| *p == *s) {
                    println!("{:?} is closure param {:?}", expr, var);
                } else if ctx.module.names[*s] {
                    let cdecl = ctx
                        .module
                        .declarations
                        .iter()
                        .find(|decl| decl.procedure[0] == *s)
                        .unwrap();
                    assert!(!cdecl.closure.is_empty());
                    println!(
                        "{:?} is a closure of {:?}{:?}",
                        expr, cdecl.procedure[0], cdecl.closure
                    );
                    // Allocate closure
                    Bump::alloc(ctx.code, i, (1 + cdecl.closure.len()) * 8);
                } else {
                    dbg!(current);
                    dbg!(target);
                    dbg!(expr);
                    panic!("Can't handle symbol");
                }
            } else {
                panic!("Can't handle expression");
            }
        }
    }
}

fn assemble_decl(ctx: &mut Context, decl: &Declaration) {
    // Transition into the correct machine state
    let current = MachineState::from_symbols(&decl.procedure);
    let target = MachineState::from_expressions(&decl.call);
    code_transition(ctx, &current, &target);

    // Call the closure
    dynasm!(ctx.code
        ; jmp QWORD [r0]
    );
    // TODO: Support
    // * non closure jump `jmp r0`,
    // * constant jump `jmp OFFSET` and
    // * fall-through.
}

pub(crate) fn compile(module: &Module, rom: &rom::Layout) -> (Vec<u8>, Layout) {
    let mut layout = Layout::default();
    let mut code = dynasmrt::x64::Assembler::new().unwrap();
    let main_index = module
        .symbols
        .iter()
        .position(|s| s == "main")
        .expect("No main found.");
    let main = &module.declarations[main_index];

    dynasm!(code
        // Prelude, write rsp to RAM[END-8]. End of ram is initialized with with
        // the OS provided stack frame.
        // TODO: Replace constant with expression
        ; mov QWORD[0x0040_1ff8], rsp

        // Jump to closure at rom zero
        ; mov r0d, DWORD (rom.closures[main_index]) as i32
        ; jmp QWORD [r0]
    );
    let state = MachineState::from_symbols(&main.procedure);
    {
        let mut ctx = Context {
            module,
            rom,
            code: &mut code,
            state,
        };

        // Declarations
        for decl in &module.declarations {
            layout.declarations.push(CODE_START + ctx.code.offset().0);
            assemble_decl(&mut ctx, decl);
        }
        // Intrinsic functions
        for import in &module.imports {
            layout.imports.push(CODE_START + ctx.code.offset().0);
            intrinsic(ctx.code, import);
        }
    };
    let code = code.finalize().expect("Finalize after commit.");
    (code.to_vec(), layout)
}
