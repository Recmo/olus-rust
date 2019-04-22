use crate::AST::*;
use std::collections::HashMap;

pub trait Visitor {
    fn visit_binder(&mut self, _: &mut Option<u64>, _: &mut String) {}

    fn visit_expression(&mut self, _: &mut Expression) {}
    fn visit_reference(&mut self, _: &mut Option<u64>, _: &mut String) {}
    fn visit_fructose(&mut self, _: &mut Vec<Binder>, _: &mut Vec<Expression>) {}
    fn visit_galactose(&mut self, _: &mut Vec<Expression>) {}
    fn visit_literal(&mut self, _: &mut String) {}
    fn visit_number(&mut self, _: &mut u64) {}

    fn visit_statement(&mut self, _: &mut Statement) {}
    fn visit_closure(&mut self, _: &mut Vec<Binder>, _: &mut Vec<Expression>) {}
    fn visit_call(&mut self, _: &mut Vec<Expression>) {}
    fn visit_block(&mut self, _: &mut Vec<Statement>) {}
}

pub trait Host {
    fn visit<V: Visitor>(&mut self, visitor: &mut V);
}

impl Host for Binder {
    fn visit<V: Visitor>(&mut self, visitor: &mut V) {
        visitor.visit_binder(&mut self.0, &mut self.1);
    }
}

impl Host for Expression {
    fn visit<V: Visitor>(&mut self, visitor: &mut V) {
        visitor.visit_expression(self);
        match self {
            Expression::Reference(a, b) => visitor.visit_reference(a, b),
            Expression::Fructose(a, b) => {
                visitor.visit_fructose(a, b);
                for ai in a.iter_mut() {
                    ai.visit(visitor);
                }
                for bi in b.iter_mut() {
                    bi.visit(visitor);
                }
            }
            Expression::Galactose(a) => {
                visitor.visit_galactose(a);
                for ai in a.iter_mut() {
                    ai.visit(visitor);
                }
            }
            Expression::Literal(a) => visitor.visit_literal(a),
            Expression::Number(a) => visitor.visit_number(a),
        }
    }
}

impl Host for Statement {
    fn visit<V: Visitor>(&mut self, visitor: &mut V) {
        visitor.visit_statement(self);
        match self {
            Statement::Closure(a, b) => {
                visitor.visit_closure(a, b);
                for ai in a.iter_mut() {
                    ai.visit(visitor);
                }
                for bi in b.iter_mut() {
                    bi.visit(visitor);
                }
            }
            Statement::Call(a) => {
                visitor.visit_call(a);
                for ai in a.iter_mut() {
                    ai.visit(visitor);
                }
            }
            Statement::Block(a) => {
                visitor.visit_block(a);
                for ai in a.iter_mut() {
                    ai.visit(visitor);
                }
            }
        }
    }
}

// Visit AST and label binders and references
pub fn bind(block: &mut Statement) -> u64 {
    // Number binders starting from zero
    struct NumberBinders(u64);
    impl Visitor for NumberBinders {
        fn visit_binder(&mut self, n: &mut Option<u64>, _: &mut String) {
            *n = Some(self.0);
            self.0 += 1;
        }
    }
    let mut number_binders = NumberBinders(0);
    block.visit(&mut number_binders);
    let num_binders = number_binders.0;

    // Bind references
    struct BindReferences(HashMap<String, u64>);
    impl Visitor for BindReferences {
        fn visit_binder(&mut self, n: &mut Option<u64>, s: &mut String) {
            self.0.insert(s.to_string(), n.unwrap());
        }
        fn visit_reference(&mut self, n: &mut Option<u64>, s: &mut String) {
            self.0.get(s).map(|i| *n = Some(*i));
        }
    }
    let mut bind_references = BindReferences(HashMap::new());
    block.visit(&mut bind_references);

    num_binders
}

pub fn fructase(block: &mut Statement) {}

pub fn galactase(block: &mut Statement) {}

pub fn glucase(block: &mut Statement) {}

pub fn flatten(block: &mut Statement) {}

pub fn desugar(block: &mut Statement) {
    bind(block);
    fructase(block);
    galactase(block);
    glucase(block);
    flatten(block);
}
