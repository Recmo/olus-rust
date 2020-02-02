use crate::ast;
use bitvec;
use serde::{Deserialize, Serialize};

type BitVec = bitvec::vec::BitVec<bitvec::order::Lsb0, u64>;

// TODO: Use entity-component system like the specs crate?
// TODO:
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Module {
    pub symbols:      Vec<String>,
    pub names:        BitVec,
    pub imports:      Vec<String>,
    pub strings:      Vec<String>,
    pub numbers:      Vec<u64>,
    pub declarations: Vec<Declaration>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub struct Declaration {
    pub procedure: Vec<usize>, // Only symbols
    pub call:      Vec<Expression>,
    pub closure:   Vec<usize>, // TODO: BitVec
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum Expression {
    Symbol(usize),
    Import(usize),
    Literal(usize),
    Number(usize),
}

impl Module {
    fn symbol(&mut self, n: usize, s: String) -> usize {
        if self.symbols.len() <= n {
            self.symbols
                .extend(std::iter::repeat(String::default()).take(1 + n - self.symbols.len()));
        }
        assert!(self.symbols.len() > n);
        self.symbols[n] = s;
        n
    }

    fn convert(&mut self, expr: ast::Expression) -> Expression {
        use ast::Expression::*;
        match expr {
            Reference(Some(n), s) => Expression::Symbol(self.symbol(n, s)),
            Reference(None, s) => {
                Expression::Import(if let Some(i) = self.imports.iter().position(|e| e == &s) {
                    i
                } else {
                    self.imports.push(s);
                    self.imports.len() - 1
                })
            }
            Literal(s) => {
                Expression::Literal(if let Some(i) = self.strings.iter().position(|e| e == &s) {
                    i
                } else {
                    self.strings.push(s);
                    self.strings.len() - 1
                })
            }
            Number(n) => {
                Expression::Number(if let Some(i) = self.numbers.iter().position(|e| e == &n) {
                    i
                } else {
                    self.numbers.push(n);
                    self.numbers.len() - 1
                })
            }
            _ => panic!("Need to bind and digest sugar first."),
        }
    }

    pub fn find_names(&mut self) {
        self.names = BitVec::repeat(false, self.symbols.len());
        for decl in &self.declarations {
            self.names.set(decl.procedure[0], true);
        }
    }

    pub fn compute_closures(&mut self) {
        for decl in self.declarations.iter_mut() {
            let mut provided = BitVec::repeat(false, self.symbols.len());
            for i in &decl.procedure {
                provided.set(*i, true);
            }
            let mut required = BitVec::repeat(false, self.symbols.len());
            for e in &decl.call {
                if let Expression::Symbol(i) = e {
                    let is_name = self.names[*i];
                    if !is_name {
                        required.set(*i, true);
                    } else {
                        // TODO: Recursive closures!
                        println!("Ignoring closure for {} in {}", *i, decl.procedure[0]);
                        unimplemented!();
                    }
                }
            }
            let closure = required & !provided;

            // First approximation: Closure is call - procedure
            decl.closure = (0..self.symbols.len())
                .filter(|i| closure[*i])
                .collect::<Vec<_>>();
            // If a closure element is a name, it will be recursively replaced
            // by the associated closure. But note that we still filter out
            // procedure.
        }
    }
}

impl From<&ast::Statement> for Module {
    /// Requires the block to be desugared
    fn from(block: &ast::Statement) -> Self {
        let mut module = Module::default();
        if let ast::Statement::Block(statements) = block {
            module.declarations = statements
                .iter()
                .map(|statement| {
                    match statement {
                        ast::Statement::Closure(a, b) => {
                            Declaration {
                                procedure: a
                                    .iter()
                                    .map(|binder| {
                                        module.symbol(
                                            binder.0.expect("Must be bound"),
                                            binder.1.clone(),
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                                call:      b
                                    .iter()
                                    .map(|expr| module.convert(expr.clone()))
                                    .collect::<Vec<_>>(),
                                closure:   Vec::new(),
                            }
                        }
                        _ => panic!("Expected closure"),
                    }
                })
                .collect::<Vec<_>>();
        } else {
            panic!("Expected block")
        }
        module.find_names();
        module.compute_closures();
        module
    }
}
