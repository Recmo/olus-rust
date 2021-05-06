use std::unimplemented;

use parser::mir::{Declaration, Expression, Module};

pub struct Interpeter<'module> {
    module: &'module Module,
}

pub struct State<'module> {
    module: &'module Module,
    call:   Vec<Value<'module>>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Value<'module> {
    Builtin(String),
    Closure(Closure<'module>),
    String(String),
    Number(u64),
}

#[derive(Clone, PartialEq, Debug)]
struct Closure<'module> {
    declaration: &'module Declaration,
    closure:     Vec<Value<'module>>,
}

impl<'module> Interpeter<'module> {
    pub fn new(module: &'module Module) -> Self {
        dbg!(module);
        Self { module }
    }

    pub fn eval_by_name(&self, name: &str, arguments: &[Value<'module>]) {
        // Find name
        let index = self
            .module
            .symbols
            .iter()
            .position(|item| item == name)
            .expect("Function not found");
        if !self.module.names[index] {
            panic!("Symbol is not a proper name");
        }

        // Set initial state
        let closure = Value::Closure(Closure {
            declaration: self
                .module
                .declaration(index)
                .expect("Symbol is not a proper name"),
            closure:     vec![],
        });
        let mut state = State {
            module: self.module,
            call:   std::iter::once(closure)
                .chain(arguments.iter().cloned())
                .collect(),
        };

        // Run till completion
        state.run();
    }
}

impl<'module> State<'module> {
    fn run(&mut self) {
        while self.step() {}
    }

    fn step(&mut self) -> bool {
        self.pretty_print();
        match self.call.first() {
            Some(Value::Builtin(s)) => {
                match s.as_ref() {
                    "print" => self.print().is_some(),
                    "exit" => self.exit().is_some(),
                    "isZero" => self.is_zero().is_some(),
                    "sub" => self.sub().is_some(),
                    "add" => self.add().is_some(),
                    "divmod" => self.divmod().is_some(),
                    "mul" => self.mul().is_some(),
                    _ => unimplemented!(),
                }
            }
            Some(Value::Closure(closure)) => {
                self.call = closure
                    .declaration
                    .call
                    .iter()
                    .map(|expr| {
                        match expr {
                            Expression::Symbol(s) => {
                                self.resolve(*s).expect("Could not resolve symbol value")
                            }
                            Expression::Import(i) => {
                                Value::Builtin(self.module.imports[*i].clone())
                            }
                            Expression::Literal(i) => {
                                Value::String(self.module.strings[*i].clone())
                            }
                            Expression::Number(i) => Value::Number(self.module.numbers[*i]),
                        }
                    })
                    .collect();
                true
            }
            Some(_) => panic!("Can not execute"),
            None => false,
        }
    }

    fn resolve(&self, symbol: usize) -> Option<Value<'module>> {
        // Resolve only works in a closure
        let closure = match self.call.first()? {
            Value::Closure(closure) => Some(closure),
            _ => None,
        }?;
        let decl = closure.declaration;

        // Check argument values
        let value = decl
            .procedure
            .iter()
            .zip(self.call.iter())
            .find(|(s, _)| **s == symbol)
            .map(|(_, v)| v);
        if value.is_some() {
            return value.cloned();
        }

        // Check closure values
        let value = decl
            .closure
            .iter()
            .zip(closure.closure.iter())
            .find(|(s, _)| **s == symbol)
            .map(|(_, v)| v);
        if value.is_some() {
            return value.cloned();
        }

        // Create new closure?
        if let Some(declaration) = self.module.declaration(symbol) {
            return declaration
                .closure
                .iter()
                .map(|s| self.resolve(*s))
                .collect::<Option<Vec<_>>>()
                .map(|closure| {
                    Value::Closure(Closure {
                        declaration,
                        closure,
                    })
                });
        }

        // Builtin?
        println!("Could not resolve symbol {}", self.module.symbols[symbol]);
        return None;
    }

    pub fn pretty_print(&self) {
        print!("\n⇒ ");
        for value in &self.call {
            match value {
                Value::Builtin(name) => print!("{} ", name),
                Value::String(s) => print!("“{}” ", s),
                Value::Number(n) => print!("{} ", n),
                Value::Closure(c) => {
                    let symbol = c.declaration.procedure[0];
                    let name = &self.module.symbols[symbol];
                    if name.is_empty() {
                        print!("λ{} ", symbol);
                    } else {
                        print!("{} ", name);
                    }
                }
            }
        }
        println!("");
    }

    fn print(&mut self) -> Option<()> {
        assert_eq!(
            self.call.first(),
            Some(&Value::Builtin("print".to_string()))
        );
        assert_eq!(self.call.len(), 3);
        let string = match &self.call[1] {
            Value::String(s) => Some(s),
            _ => None,
        }?;
        print!("{}", string);
        self.call = vec![self.call[2].clone()];
        Some(())
    }

    fn exit(&mut self) -> Option<()> {
        assert_eq!(self.call.first(), Some(&Value::Builtin("exit".to_string())));
        assert_eq!(self.call.len(), 2);
        let code = match &self.call[1] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        println!("[EXIT] {}", code);
        self.call = vec![];
        Some(())
    }

    fn is_zero(&mut self) -> Option<()> {
        assert_eq!(
            self.call.first(),
            Some(&Value::Builtin("isZero".to_string()))
        );
        assert_eq!(self.call.len(), 4);
        let n = match &self.call[1] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        self.call = vec![self.call[if *n == 0 { 2 } else { 3 }].clone()];
        Some(())
    }

    fn sub(&mut self) -> Option<()> {
        assert_eq!(self.call.first(), Some(&Value::Builtin("sub".to_string())));
        assert_eq!(self.call.len(), 4);
        let a = match &self.call[1] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        let b = match &self.call[2] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        self.call = vec![self.call[3].clone(), Value::Number(a - b)];
        Some(())
    }

    fn add(&mut self) -> Option<()> {
        assert_eq!(self.call.first(), Some(&Value::Builtin("add".to_string())));
        assert_eq!(self.call.len(), 4);
        let a = match &self.call[1] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        let b = match &self.call[2] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        self.call = vec![self.call[3].clone(), Value::Number(a + b)];
        Some(())
    }

    fn divmod(&mut self) -> Option<()> {
        assert_eq!(
            self.call.first(),
            Some(&Value::Builtin("divmod".to_string()))
        );
        assert_eq!(self.call.len(), 4);
        let a = match &self.call[1] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        let b = match &self.call[2] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        self.call = vec![
            self.call[3].clone(),
            Value::Number(a / b),
            Value::Number(a % b),
        ];
        Some(())
    }

    fn mul(&mut self) -> Option<()> {
        assert_eq!(self.call.first(), Some(&Value::Builtin("mul".to_string())));
        assert_eq!(self.call.len(), 4);
        let a = match &self.call[1] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        let b = match &self.call[2] {
            Value::Number(n) => Some(n),
            _ => None,
        }?;
        self.call = vec![self.call[3].clone(), Value::Number(a * b)];
        Some(())
    }
}
