use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

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

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Value::*;
        match *self {
            Unspecified => write!(f, "?"),
            Literal(n) => write!(f, "0x{:016x}", n),
            Symbol(n) => write!(f, "#{}", n),
            Reference { index, offset } => write!(f, "{}[{}]", index, offset),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::{
        arbitrary::any,
        prop_oneof,
        strategy::{Just, LazyTupleUnion, Strategy},
    };

    fn arb_value(num_allocations: usize) -> impl Strategy<Value = Value> {
        prop_oneof![
            Just(Value::Unspecified),
            any::<u64>().prop_map(Value::Literal),
            any::<usize>().prop_map(Value::Symbol),
            (0..num_allocations, any::<isize>())
                .prop_map(|(index, offset)| Value::Reference { index, offset }),
        ]
    }
}
