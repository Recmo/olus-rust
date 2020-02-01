use crate::{
    allocator::{Allocator, Bump},
    intrinsic,
    macho::CODE_START,
    rom,
    utils::{
        assemble_literal, assemble_mov, assemble_read, assemble_write_const, assemble_write_read,
        assemble_write_reg,
    },
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

impl Layout {
    pub(crate) fn dummy(module: &Module) -> Layout {
        const DUMMY: usize = usize::max_value();
        Layout {
            declarations: vec![DUMMY; module.declarations.len()],
            imports:      vec![DUMMY; module.imports.len()],
        }
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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
struct MachineState {
    registers: [Option<Expression>; 16],
    // TODO: Flags (carry, parity, adjust, zero, sign, direction, overflow)
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

struct Context<'a> {
    module: &'a Module,
    rom:    &'a rom::Layout,
    code:   &'a Layout,
    asm:    &'a mut Assembler,
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
                // TODO: Make sure this is actually a closure meant for the
                // current context and not something temporary.
                decl.closure.clone()
            } else {
                vec![]
            }
        } else {
            vec![]
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

fn code_transition(ctx: &mut Context<'_>, target: &MachineState) {
    // Rough order:
    // * Drop registers? (depends on type)
    // * Shuffle registers? (Can also have duplicates and drops)
    // * Create closures
    // * Load closure values
    // * Load all the literals
    // * Copy registers

    // Iterate target right to left
    // TODO: Strategic ordering
    for (i, expr) in target.registers.iter().enumerate().rev() {
        if let Some(expr) = expr {
            match ctx.find(expr) {
                Source::Constant(n) => assemble_literal(ctx.asm, i, n),
                Source::Register(j) => assemble_mov(ctx.asm, i, j),
                Source::Closure(j) => assemble_read(ctx.asm, i, j),
                Source::Alloc(j) => {
                    let decl = &ctx.module.declarations[j];

                    // Closure: [<code pointer>, <value>, ...]
                    let size = 8 * (1 + decl.closure.len());

                    // Allocate space for the closure and put pointer in reg
                    Bump::alloc(ctx.asm, i, size);

                    // Write code pointer
                    assemble_write_const(ctx.asm, i, 0, ctx.code.declarations[j] as u64);

                    // Write values
                    for (j, sym) in decl.closure.iter().enumerate() {
                        let offset = 8 * (1 + j);
                        match ctx.find(&Expression::Symbol(*sym)) {
                            Source::Constant(_) => panic!("Constants don't go into closures."),
                            Source::Register(k) => assemble_write_reg(ctx.asm, i, offset, k),
                            Source::Closure(k) => assemble_write_read(ctx.asm, i, offset, k),
                            Source::Alloc(_) => panic!("Nested closures unsupported!"),
                            Source::None => panic!("Could not find value for closure."),
                        }
                    }
                }
                Source::None => panic!("Don't know how to create {:?}", expr),
            };
            ctx.state.registers[i] = Some(expr.clone());
        }
    }
}

fn assemble_decl(ctx: &mut Context, decl: &Declaration) {
    // Transition into the correct machine state
    ctx.state = MachineState::from_symbols(&decl.procedure);
    let target = MachineState::from_expressions(&decl.call);
    code_transition(ctx, &target);

    // Call the closure
    dynasm!(ctx.asm
        ; jmp QWORD [r0]
    );
    // TODO: Support
    // * non closure jump `jmp r0`,
    // * constant jump `jmp OFFSET` and
    // * fall-through.
}

pub(crate) fn compile(module: &Module, rom: &rom::Layout, code: &Layout) -> (Vec<u8>, Layout) {
    assert_eq!(rom.closures.len(), module.declarations.len());
    assert_eq!(rom.imports.len(), module.imports.len());
    assert_eq!(rom.strings.len(), module.strings.len());
    assert_eq!(code.declarations.len(), module.declarations.len());
    assert_eq!(code.imports.len(), module.imports.len());

    let mut layout = Layout::default();
    let mut asm = dynasmrt::x64::Assembler::new().unwrap();
    let main_index = module
        .symbols
        .iter()
        .position(|s| s == "main")
        .expect("No main found.");
    let main = &module.declarations[main_index];

    dynasm!(asm
        // Prelude, write rsp to RAM[END-8]. End of ram is initialized with with
        // the OS provided stack frame.
        // TODO: Replace constant with expression
        ; mov QWORD[0x0040_1ff8], rsp

        // Jump to closure at rom zero
        ; mov r0d, DWORD (rom.closures[main_index]) as i32
        ; jmp QWORD [r0]
    );
    {
        let mut ctx = Context {
            module,
            rom,
            code,
            asm: &mut asm,
            state: MachineState::default(),
        };

        // Declarations
        for decl in &module.declarations {
            layout.declarations.push(CODE_START + ctx.asm.offset().0);
            assemble_decl(&mut ctx, decl);
        }
        // Intrinsic functions
        for import in &module.imports {
            layout.imports.push(CODE_START + ctx.asm.offset().0);
            intrinsic(ctx.asm, import);
        }
    };
    let asm = asm.finalize().expect("Finalize after commit.");
    (asm.to_vec(), layout)
}
