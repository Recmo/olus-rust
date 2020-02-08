use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub(crate) enum Value {
    Unspecified,
    Literal(u64),
    Symbol(usize),
    Reference { index: usize, offset: isize },
}

impl Value {
    pub(crate) fn is_specified(&self) -> bool {
        *self != Value::Unspecified
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Unspecified
    }
}
