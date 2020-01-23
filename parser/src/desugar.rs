use crate::Ast::*;
use std::collections::HashMap;

pub trait Visitor {
    fn visit_binder(&mut self, _: &mut Option<usize>, _: &mut String) {}

    fn visit_expression(&mut self, _: &mut Expression) {}
    fn leave_expression(&mut self, _: &mut Expression) {}
    fn visit_reference(&mut self, _: &mut Option<usize>, _: &mut String) {}
    fn visit_fructose(&mut self, _: &mut Vec<Binder>, _: &mut Vec<Expression>) {}
    fn visit_galactose(&mut self, _: &mut Vec<Expression>) {}
    fn visit_literal(&mut self, _: &mut String) {}
    fn visit_number(&mut self, _: &mut u64) {}

    fn visit_statement(&mut self, _: &mut Statement) {}
    fn leave_statement(&mut self, _: &mut Statement) {}
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
        visitor.leave_expression(self);
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
        visitor.leave_statement(self);
    }
}

/// Bind References to their Binders and flattens Blocks.
pub fn bind(block: &mut Statement) -> usize {
    // Number binders starting from zero
    struct NumberBinders(usize);
    impl Visitor for NumberBinders {
        fn visit_binder(&mut self, n: &mut Option<usize>, _: &mut String) {
            *n = Some(self.0);
            self.0 += 1;
        }
    }
    let mut number_binders = NumberBinders(0);
    block.visit(&mut number_binders);
    let num_binders = number_binders.0;

    // Bind references
    struct BindReferences(HashMap<String, usize>);
    impl Visitor for BindReferences {
        fn visit_binder(&mut self, n: &mut Option<usize>, s: &mut String) {
            // TODO: Scoping.
            // TODO: Forward looking.
            self.0.insert(s.to_string(), n.unwrap());
        }
        fn visit_reference(&mut self, n: &mut Option<usize>, s: &mut String) {
            *n = self.0.get(s).cloned();
        }
    }
    let mut bind_references = BindReferences(HashMap::new());
    block.visit(&mut bind_references);

    // Flatten blocks
    struct Flatten(Vec<Statement>);
    impl Visitor for Flatten {
        fn visit_statement(&mut self, s: &mut Statement) {
            match s {
                Statement::Block(_) => {}
                _ => self.0.push(s.clone()),
            }
        }
    }
    let mut flatten = Flatten(Vec::new());
    block.visit(&mut flatten);
    *block = Statement::Block(flatten.0);

    num_binders
}

fn merge(target: &mut Vec<Expression>, call: Vec<Expression>) {
    // Empty expressions get replaces in entirety
    if target.is_empty() {
        *target = call;
        return;
    }

    // Visit expression
    struct State(bool, Vec<Expression>);
    impl Visitor for State {
        fn visit_fructose(&mut self, _: &mut Vec<Binder>, tcall: &mut Vec<Expression>) {
            self.visit_galactose(tcall);
        }
        fn visit_galactose(&mut self, tcall: &mut Vec<Expression>) {
            if !self.0 && tcall.is_empty() {
                *tcall = self.1.clone();
                self.0 = true;
            }
        }
    }
    let mut state = State(false, call);
    for expr in target {
        expr.visit(&mut state);
        if state.0 {
            return;
        }
    }
    panic!("Can not digest glucose.");
}

/// Fill empty calls with following statement
pub fn glucase(statements: &[Statement]) -> Vec<Statement> {
    let mut result = Vec::new();
    let mut closure: Option<(Vec<Binder>, Vec<Expression>)> = None;
    for statement in statements {
        match statement {
            Statement::Block(_) => panic!("Blocks not allowed here."),
            Statement::Closure(a, b) => {
                if let Some((c, d)) = closure {
                    // TODO: Assert that result has no empty calls
                    result.push(Statement::Closure(c, d));
                }
                closure = Some((a.clone(), b.clone()));
            }
            Statement::Call(a) => {
                if let Some((_, d)) = &mut closure {
                    merge(d, a.clone());
                } else {
                    panic!("Call without preceding closure.")
                }
            }
        }
    }
    result
}

pub fn glucase_wrap(block: &mut Statement) {
    if let Statement::Block(statements) = block {
        *statements = glucase(&statements);
    }
}

/// Converts all Fructose to Closures.
pub fn fructase(block: &mut Statement, binder_id: &mut usize) {
    struct State(usize, Vec<Statement>);
    impl Visitor for State {
        fn leave_expression(&mut self, e: &mut Expression) {
            *e = if let Expression::Fructose(p, c) = e {
                let replacement = Expression::Reference(Some(self.0), String::default());
                let mut procedure = Vec::new();
                std::mem::swap(p, &mut procedure);
                let mut call = Vec::new();
                std::mem::swap(c, &mut call);
                procedure.insert(0, Binder(Some(self.0), String::default()));
                self.0 += 1;
                // TODO: For glucase may need merge with sibling
                self.1.push(Statement::Closure(procedure, call));
                replacement
            } else {
                // TODO: Avoid copies
                e.clone()
            }
        }
    }
    let mut state = State(*binder_id, Vec::new());
    block.visit(&mut state);
    *binder_id = state.0;
    if let Statement::Block(statements) = block {
        statements.extend(state.1);
    } else {
        panic!("Statement must be a block.")
    }
}

pub fn galac_vec(exprs: &mut Vec<Expression>, binder_id: &mut usize) {
    // Find first Galactose or return
    if let Some(index) = exprs.iter().position(|e| match e {
        Expression::Galactose(_) => true,
        _ => false,
    }) {
        // Invert Galactose into Fructose

        // Replace galactose by a reference and fetch the call vec
        let mut temp = Expression::Reference(Some(*binder_id), String::default());
        std::mem::swap(&mut exprs[index], &mut temp);
        let mut call = match temp {
            Expression::Galactose(c) => c,
            _ => panic!("No Galactose at index."),
        };

        // Swap expression and call
        std::mem::swap(exprs, &mut call);

        // Append new fructose to the expression in the last position
        exprs.push(Expression::Fructose(
            vec![Binder(Some(*binder_id), String::default())],
            call,
        ));

        // Update next binder id
        *binder_id += 1;

        // Iterate till fix-point
        // TODO: What about iterating on `call`?
        galac_vec(exprs, binder_id)
    }
}

pub fn galactase(block: &mut Statement, binder_id: &mut usize) {
    struct State(usize);
    impl Visitor for State {
        fn visit_closure(&mut self, _: &mut Vec<Binder>, exprs: &mut Vec<Expression>) {
            galac_vec(exprs, &mut self.0);
        }
        fn visit_fructose(&mut self, _: &mut Vec<Binder>, exprs: &mut Vec<Expression>) {
            galac_vec(exprs, &mut self.0);
        }
        fn visit_galactose(&mut self, exprs: &mut Vec<Expression>) {
            galac_vec(exprs, &mut self.0);
        }
    }
    let mut state = State(*binder_id);
    block.visit(&mut state);
    *binder_id = state.0;
}

pub fn desugar(block: &mut Statement) {
    let mut binder_count = bind(block);
    glucase_wrap(block);
    galactase(block, &mut binder_count);
    fructase(block, &mut binder_count);
}
