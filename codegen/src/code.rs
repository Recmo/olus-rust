use crate::{
    allocator::{Allocator, Bump},
    intrinsic,
    machine::{Allocation, State, Value},
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
        const DUMMY_SIZE: usize = 1 << 10; // ~ 1kiB of code
        let declarations: Vec<usize> = (0..module.declarations.len())
            .map(|i| CODE_START + i * DUMMY_SIZE)
            .collect();
        let imports: Vec<usize> = (0..module.imports.len())
            .map(|i| declarations.last().unwrap() + (i + 1) * DUMMY_SIZE)
            .collect();
        Layout {
            declarations,
            imports,
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

struct Context<'a> {
    module:    &'a Module,
    code:      &'a Layout,
    rom:       &'a rom::Layout,
    ram_start: usize,
    asm:       &'a mut Assembler,
}

impl<'a> Context<'a> {
    fn find_decl(&self, symbol: usize) -> Option<(usize, &'a Declaration)> {
        self.module
            .declarations
            .iter()
            .enumerate()
            .find(|decl| decl.1.procedure[0] == symbol)
    }
}

fn closure_val(ctx: &mut Context<'_>, symbol: usize) -> Vec<Value> {
    let (index, decl) = ctx.find_decl(symbol).expect("Expected closure symbol");
    let mut result = vec![Value::Literal(ctx.code.declarations[index] as u64)];
    for symbol in &decl.closure {
        result.push(Value::Symbol(*symbol));
    }
    result
}

fn assemble_decl(ctx: &mut Context<'_>, decl: &Declaration) {
    // Initial state has one closure expanded
    // TODO: Don't expand constant closures
    let mut initial = State::default();
    for (i, symbol) in decl.procedure.iter().enumerate() {
        initial.registers[i] = Value::Symbol(*symbol);
    }
    if !decl.closure.is_empty() {
        initial
            .allocations
            .push(Allocation(closure_val(ctx, decl.procedure[0])));
        initial.registers[0] = Value::Reference {
            index:  0,
            offset: 0,
        };
    }

    println!("Initial:\n{}", initial);
    let available = initial.symbols();

    // Goal state is the call with closures expanded as needed
    let mut goal = State::default();
    for (i, expr) in decl.call.iter().enumerate() {
        goal.registers[i] = match *expr {
            Expression::Literal(i) => Value::Literal(ctx.rom.strings[i] as u64),
            Expression::Number(n) => Value::Literal(ctx.module.numbers[n]),
            Expression::Import(i) => Value::Literal(ctx.rom.imports[i] as u64),
            Expression::Symbol(s) => {
                if available.contains(&s) {
                    Value::Symbol(s)
                } else {
                    let val = Value::Reference {
                        index:  goal.allocations.len(),
                        offset: 0,
                    };
                    // TODO: recursively allocate closures
                    goal.allocations.push(Allocation(closure_val(ctx, s)));
                    val
                }
            }
        };
    }
    println!("Goal:\n{}", goal);

    // Transition into the correct machine state
    let path = initial.transition_to(&goal);
    println!("Path: {:?}", path);
    for transition in path {
        transition.assemble(ctx.asm);
    }

    // Call the closure
    dynasm!(ctx.asm
        ; jmp QWORD [r0]
    );
}

pub(crate) fn compile(
    module: &Module,
    code: &Layout,
    rom: &rom::Layout,
    ram_start: usize,
) -> (Vec<u8>, Layout) {
    assert_eq!(rom.closures.len(), module.declarations.len());
    assert_eq!(rom.imports.len(), module.imports.len());
    assert_eq!(rom.strings.len(), module.strings.len());
    assert_eq!(code.declarations.len(), module.declarations.len());
    assert_eq!(code.imports.len(), module.imports.len());

    let mut layout = Layout::default();
    let mut asm = dynasmrt::x64::Assembler::new().unwrap();
    let main_symbol = module
        .symbols
        .iter()
        .position(|s| s == "main")
        .expect("No symbol 'main' found.");
    let main_index = module
        .declarations
        .iter()
        .position(|decl| decl.procedure[0] == main_symbol)
        .expect("Symbol 'main' is not a name.");
    let main = &module.declarations[main_index];
    assert_eq!(main.closure.len(), 0);

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
            code,
            rom,
            ram_start,
            asm: &mut asm,
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
