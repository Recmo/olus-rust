use super::Value;

use crate::BitVec;
use serde::{Deserialize, Serialize};
use std::slice::Iter as SliceIter;

// TODO: Explore exotic instructions that can potentially accomplish the same
// in fewer bytes/cycles:
// * Immediate writes 1-4 bytes
// * Sign extended immediate loads
// * Stack operators: PUSH, POP
// * String operations: LODS, STOS

pub(crate) type Reg = u8;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub(crate) struct State {
    registers:   [Value; 16],
    flags:       [Value; 7],
    // TODO: Implement Eq to ignore permutation of allocations.
    allocations: Vec<Allocation>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub(crate) enum Flag {
    Carry     = 0,
    Parity    = 1,
    Adjust    = 2,
    Zero      = 3,
    Sign      = 4,
    Direction = 5,
    Overflow  = 6,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub(crate) struct Allocation(Vec<Value>);

#[derive(Clone, Debug)]
pub(crate) struct StateIterator<'a> {
    state: &'a State,
    index: StateIteratorIndex<'a>,
}

#[derive(Clone, Debug)]
pub(crate) enum StateIteratorIndex<'a> {
    Register(SliceIter<'a, Value>),
    Flags(SliceIter<'a, Value>),
    Allocations(SliceIter<'a, Allocation>),
    Allocation(SliceIter<'a, Allocation>, SliceIter<'a, Value>),
    Done,
}

impl State {
    pub fn is_valid(&self) -> bool {
        use Value::*;
        // Make sure all references are N:1 to allocations
        let mut seen = BitVec::repeat(false, self.allocations.len());
        for val in &self.registers {
            if let Reference { index, .. } = val {
                if let Some(mut bit) = seen.get_mut(*index) {
                    *bit = false;
                } else {
                    return false;
                }
            }
        }
        for alloc in &self.allocations {
            for val in alloc {
                if let Reference { index, .. } = val {
                    if let Some(mut bit) = seen.get_mut(*index) {
                        *bit = false;
                    } else {
                        return false;
                    }
                }
            }
        }
        if seen.not_all() {
            return false;
        }

        // Flags can only hold symbol, unspecified or boolean 0 / 1
        for flag in &self.flags {
            match flag {
                Unspecified | Symbol(_) => {}
                Literal(n) if *n <= 1 => {}
                _ => return false,
            }
        }

        dbg!(self
            .registers
            .iter()
            .chain(self.flags.iter())
            .chain(self.allocations.iter().flat_map(|a| a.into_iter())));

        // Otherwise it is valid
        true
    }

    // pub(crate) fn symbols(&self) -> Set<usize> {
    // fn recurse(set: &mut Set<usize>, value: &Value) {
    // use Value::*;
    // match value {
    // Symbol(s) => {
    // let _ = set.insert(*s);
    // }
    // Reference(values) => values.iter().for_each(|v| recurse(set, v)),
    // _ => {}
    // }
    // }
    // let mut result = Set::default();
    // self.registers.iter().for_each(|v| recurse(&mut result, v));
    // result
    // }
    //
    // pub(crate) fn literals(&self) -> Set<u64> {
    // fn recurse(set: &mut Set<u64>, value: &Value) {
    // use Value::*;
    // match value {
    // Literal(l) => {
    // let _ = set.insert(*l);
    // }
    // Reference(values) => values.iter().for_each(|v| recurse(set, v)),
    // _ => {}
    // }
    // }
    // let mut result = Set::default();
    // self.registers.iter().for_each(|v| recurse(&mut result, v));
    // result
    // }
    //
    // pub(crate) fn alloc_sizes(&self) -> Set<usize> {
    // fn recurse(set: &mut Set<usize>, value: &Value) {
    // use Value::*;
    // match value {
    // Reference(v) => {
    // let _ = set.insert(v.len());
    // }
    // Reference(values) => values.iter().for_each(|v| recurse(set, v)),
    // _ => {}
    // }
    // }
    // let mut result = Set::default();
    // self.registers.iter().for_each(|v| recurse(&mut result, v));
    // result
    // }
    //
    // /// A goal is reachable if it contains a subset of our symbols.
    // pub(crate) fn reachable(&self, goal: &Self) -> bool {
    //     goal.symbols().is_subset(&self.symbols())
    // }

    // /// A goal is satisfied if all specified values are in place.
    // pub(crate) fn satisfies(&self, goal: &Self) -> bool {
    //     self.registers
    //         .iter()
    //         .zip(goal.registers.iter())
    //         .all(|(a, b)| a.satisfies(b))
    // }
}

impl<'a> IntoIterator for &'a Allocation {
    type IntoIter = SliceIter<'a, Value>;
    type Item = &'a Value;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a State {
    type IntoIter = StateIterator<'a>;
    type Item = &'a Value;

    fn into_iter(self) -> Self::IntoIter {
        StateIterator {
            state: self,
            index: StateIteratorIndex::Register(self.registers.iter()),
        }
    }
}

impl<'a> Iterator for StateIterator<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        use StateIteratorIndex::*;
        match &mut self.index {
            Register(iter) => {
                iter.next().or_else(|| {
                    self.index = Flags(self.state.flags.iter());
                    self.next()
                })
            }
            Flags(iter) => {
                iter.next().or_else(|| {
                    self.index = Allocations(self.state.allocations.iter());
                    self.next()
                })
            }
            Allocations(iter) => {
                if let Some(alloc) = iter.next() {
                    self.index = Allocation(iter.clone(), alloc.into_iter());
                    self.next()
                } else {
                    self.index = Done;
                    self.next()
                }
            }
            Allocation(outer, iter) => {
                iter.next().or_else(|| {
                    // Satisfy borrow checker
                    if let Allocation(outer, _) = &self.index {
                        self.index = Allocations(outer.clone());
                        self.next()
                    } else {
                        panic!("Impossible state");
                    }
                })
            }
            Done => None,
        }
    }
}
