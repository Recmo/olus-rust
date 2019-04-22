use serde::{Deserialize, Serialize};

// An identifier occupies a binder spot.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub struct Binder(pub String);

// An expression occupies a reference spot.
// Fructose is an inline declaration in parenthesis. It occupies one reference
// spot which is linked to one implicit binding spot.
// Galactose is a call statement in parenthesis.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum Expression {
    Reference(String),
    Fructose(Vec<Binder>, Vec<Expression>),
    Galactose(Vec<Expression>),
    Literal(String),
}

// Glucose is a closure with an empty Call followed by a Call on the next line.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum Statement {
    Closure(Vec<Binder>, Vec<Expression>),
    Call(Vec<Expression>),
    Block(Vec<Statement>),
}
