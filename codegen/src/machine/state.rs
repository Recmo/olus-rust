use super::Value;

use crate::{BitVec, Set};
use serde::{Deserialize, Serialize};
use std::slice::Iter as SliceIter;

// TODO: Explore exotic instructions that can potentially accomplish the same
// in fewer bytes/cycles:
// * Immediate writes 1-4 bytes
// * Sign extended immediate loads
// * Stack operators: PUSH, POP
// * String operations: LODS, STOS

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
pub(crate) struct State {
    pub(crate) registers:   [Value; 16],
    pub(crate) flags:       [Value; 7],
    // TODO: Implement Eq to ignore permutation of allocations.
    pub(crate) allocations: Vec<Allocation>,
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

impl Allocation {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn iter(&self) -> <&Self as IntoIterator>::IntoIter {
        self.into_iter()
    }
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

        // Otherwise it is valid
        true
    }

    pub(crate) fn symbols(&self) -> Set<usize> {
        self.into_iter()
            .filter_map(|val| {
                match val {
                    Value::Symbol(s) => Some(*s),
                    _ => None,
                }
            })
            .collect()
    }

    pub(crate) fn literals(&self) -> Set<u64> {
        self.into_iter()
            .filter_map(|val| {
                match val {
                    Value::Literal(l) => Some(*l),
                    _ => None,
                }
            })
            .collect()
    }

    pub(crate) fn alloc_sizes(&self) -> Set<usize> {
        self.allocations.iter().map(|a| a.0.len()).collect()
    }

    /// A goal is reachable if it contains a subset of our symbols.
    pub(crate) fn reachable(&self, goal: &Self) -> bool {
        debug_assert!(self.is_valid());
        debug_assert!(goal.is_valid());

        // Only Symbols matter, everything else can be constructed.
        goal.symbols().is_subset(&self.symbols())
    }

    /// A goal is satisfied if all specified values are in place.
    pub(crate) fn satisfies(&self, goal: &Self) -> bool {
        fn valsat(reference_checks: &mut Set<(usize, usize)>, ours: &Value, goal: &Value) -> bool {
            match goal {
                Unspecified => true,
                Reference {
                    index: goal_index,
                    offset: goal_offset,
                } => {
                    match ours {
                        Reference {
                            index: our_index,
                            offset: our_offset,
                        } if our_offset == goal_offset => {
                            reference_checks.insert((*our_index, *goal_index));
                            true
                        }
                        _ => false,
                    }
                }
                val => ours == val,
            }
        }

        use Value::*;
        debug_assert!(self.is_valid());
        debug_assert!(goal.is_valid());

        // Values satisfy if `goal` is unspecified, they are identical or they are
        // references with the same offset and the allocations satisfy.
        let mut reference_checks: Set<(usize, usize)> = Set::default();

        // Check registers and flags
        let ours = self.registers.iter().chain(self.flags.iter());
        let theirs = goal.registers.iter().chain(goal.flags.iter());
        if !ours
            .zip(theirs)
            .all(|(a, b)| valsat(&mut reference_checks, a, b))
        {
            return false;
        }

        // Check correspondences between allocations, taking care of recursions
        let mut checked = Set::default();
        let mut done = reference_checks.is_empty();
        while !done {
            // Swap `reference_checks` for an empty one.
            let mut to_check = Set::default();
            std::mem::swap(&mut reference_checks, &mut to_check);

            // Check previous values of `reference_check`.
            for (our_index, their_index) in to_check {
                let ours = &self.allocations[our_index];
                let theirs = &goal.allocations[their_index];
                if ours.len() != theirs.len()
                    || !ours
                        .iter()
                        .zip(theirs.iter())
                        .all(|(a, b)| valsat(&mut reference_checks, a, b))
                {
                    return false;
                }
                checked.insert((our_index, their_index));
            }

            // Remove already checked relationships
            reference_checks = reference_checks
                .difference(&checked)
                .map(|(a, b)| (*a, *b))
                .collect();
            done = reference_checks.is_empty();
        }

        return true;
    }
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
            Allocation(_, iter) => {
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
