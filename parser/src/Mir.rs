use crate::ast;
use bitvec;
use serde::{Deserialize, Serialize};

type BitVec = bitvec::vec::BitVec<bitvec::order::Lsb0, u64>;

// TODO: Use entity-component system like the specs crate?
// TODO:
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Module {
    pub symbols: Vec<String>,

    /// Bitvector of which symbols are names and not arguments
    pub names: BitVec,

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

    pub fn provided_mask(&self, decl: &Declaration) -> BitVec {
        let mut mask = BitVec::repeat(false, self.symbols.len());
        for i in &decl.procedure {
            mask.set(*i, true);
        }
        mask
    }

    pub fn required_mask(&self, decl: &Declaration) -> BitVec {
        let mut mask = BitVec::repeat(false, self.symbols.len());
        for e in &decl.call {
            if let Expression::Symbol(s) = e {
                mask.set(*s, true);
            }
        }
        mask
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

    pub fn declaration<'a>(&'a self, name: usize) -> Option<&'a Declaration> {
        self.declarations
            .iter()
            .find(|decl| decl.procedure[0] == name)
    }

    pub fn find_names(&mut self) {
        self.names = BitVec::repeat(false, self.symbols.len());
        for decl in &self.declarations {
            self.names.set(decl.procedure[0], true);
        }
    }

    fn closure_rec(&self, decl: &Declaration, provided: &BitVec) -> BitVec {
        // TODO: Reformulate as a linear problem over GF(2)^{N x M} and
        // solve using (sparse) matrices.
        let context = self.provided_mask(decl) | provided.clone();
        let required = self.required_mask(decl);
        let mut closure = required & !context.clone();
        let names = closure.clone() & self.names.clone();
        // If a closure element is a name, it will be recursively replaced
        // by the associated closure. But note that we still filter out
        // procedure.
        for name in (0..self.symbols.len()).filter(|i| names[*i]) {
            closure.set(name, false);
            closure |= self.closure_rec(self.declaration(name).unwrap(), &context);
        }

        // Can not have any names in the closure.
        assert!((closure.clone() & self.names.clone()).not_any());
        closure
    }

    pub fn compute_closures(&mut self) {
        assert_eq!(self.names.len(), self.symbols.len());
        let empty = BitVec::repeat(false, self.symbols.len());
        let closures = self
            .declarations
            .iter()
            .map(|decl| self.closure_rec(decl, &empty))
            .collect::<Vec<_>>();
        for (decl, closure) in self.declarations.iter_mut().zip(closures.into_iter()) {
            decl.closure = (0..self.symbols.len())
                .filter(|i| closure[*i])
                .collect::<Vec<_>>();
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
