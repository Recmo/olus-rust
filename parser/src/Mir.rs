use crate::Ast;
use serde::{Deserialize, Serialize};

// TODO: Use entity-component system like the specs crate?
// TODO:
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub struct Module {
    pub symbols: Vec<String>,
    pub imports: Vec<String>,
    pub strings: Vec<String>,
    pub numbers: Vec<u64>,
    pub declarations: Vec<Declaration>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub struct Declaration {
    pub procedure: Vec<usize>, // Only symbols
    pub call: Vec<Expression>,
    pub closure: Vec<usize>, // Only symbols
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum Expression {
    Symbol(usize),
    Import(usize),
    Literal(usize),
    Number(usize),
}

fn symbol(m: &mut Module, n: usize, s: String) -> usize {
    if m.symbols.len() <= n {
        m.symbols
            .extend(std::iter::repeat(String::default()).take(1 + n - m.symbols.len()));
    }
    assert!(m.symbols.len() > n);
    m.symbols[n] = s;
    n
}

fn convert(m: &mut Module, expr: Ast::Expression) -> Expression {
    use Ast::Expression::*;
    match expr {
        Reference(Some(n), s) => Expression::Symbol(symbol(m, n, s)),
        Reference(None, s) => {
            Expression::Symbol(if let Some(i) = m.imports.iter().position(|e| e == &s) {
                i
            } else {
                m.imports.push(s);
                m.imports.len() - 1
            })
        }
        Literal(s) => {
            dbg!(&s);
            Expression::Literal(if let Some(i) = m.strings.iter().position(|e| e == &s) {
                i
            } else {
                m.strings.push(s);
                m.strings.len() - 1
            })
        }
        Number(n) => Expression::Number(if let Some(i) = m.numbers.iter().position(|e| e == &n) {
            i
        } else {
            m.numbers.push(n);
            m.numbers.len() - 1
        }),
        _ => panic!("Need to bind and digest sugar first."),
    }
}

/// Requires the block to be desugared
pub fn ast_to_module(block: Ast::Statement) -> Module {
    let mut module = Module::default();
    if let Ast::Statement::Block(statements) = block {
        module.declarations = statements
            .iter()
            .map(|statement| match statement {
                Ast::Statement::Closure(a, b) => Declaration {
                    procedure: a
                        .iter()
                        .map(|binder| {
                            symbol(
                                &mut module,
                                binder.0.expect("Must be bound"),
                                binder.1.clone(),
                            )
                        })
                        .collect::<Vec<_>>(),
                    call: b
                        .iter()
                        .map(|expr| convert(&mut module, expr.clone()))
                        .collect::<Vec<_>>(),
                    closure: Vec::new(),
                },
                _ => panic!("Expected closure"),
            })
            .collect::<Vec<_>>();
    } else {
        panic!("Expected block")
    }
    module
}
