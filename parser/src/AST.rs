use serde::{Deserialize, Serialize};

// An identifier occupies a binder spot.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub struct Binder(pub Option<usize>, pub String);

// An expression occupies a reference spot.
// Fructose is an inline declaration in parenthesis. It occupies one reference
// spot which is linked to one implicit binding spot.
// Galactose is a call statement in parenthesis.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
#[allow(clippy::use_self)] // 'Self' confuses Serde
pub enum Expression {
    Reference(Option<usize>, String),
    Fructose(Vec<Binder>, Vec<Expression>),
    Galactose(Vec<Expression>),
    Literal(String),
    Number(u64),
}

// Glucose is a closure with an empty Call followed by a Call on the next line.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
#[allow(clippy::clippy::use_self)] // 'Self' confuses Serde
pub enum Statement {
    Closure(Vec<Binder>, Vec<Expression>),
    Call(Vec<Expression>),
    Block(Vec<Statement>),
}
