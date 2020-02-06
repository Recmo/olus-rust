use crate::allocator::{Allocator, Bump};
use dynasm::dynasm;
use dynasmrt::{DynasmApi, SimpleAssembler};
use pathfinding::directed::{astar::astar, fringe::fringe, idastar::idastar};
use serde::{Deserialize, Serialize};
use std::collections::HashSet as Set;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
enum Value {
    Unspecified,
    Literal(u64),
    Symbol(usize),
    Reference(Vec<Value>),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
struct State {
    registers: [Value; 16],
    // TODO: Flags (carry, parity, adjust, zero, sign, direction, overflow)
}

/// Single instruction
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
enum Transition {
    /// Set register `dest` to literal `value`
    Set { dest: usize, value: u64 },
    /// Copy register `source` into `dest`
    Copy { dest: usize, source: usize },
    /// Swap contents of registers `source` and `dest`
    Swap { dest: usize, source: usize },
    /// Allocate empty `Reference` of size `size` in register `dest`
    // TODO: Dealloc
    Alloc { dest: usize, size: usize },
    /// Read 64 bits from `[source + offset]` into register `dest`
    Read {
        dest:   usize,
        source: usize,
        offset: usize,
    },
    /// Write register `source` into `[dest + offset]`
    Write {
        dest:   usize,
        offset: usize,
        source: usize,
    },
}

impl Value {
    pub(crate) fn is_specified(&self) -> bool {
        *self != Value::Unspecified
    }

    /// A goal is satisfied if all specified values are in place.
    pub(crate) fn satisfies(&self, goal: &Self) -> bool {
        use Value::*;
        if *goal == Unspecified {
            true
        } else {
            if let Reference(goals) = goal {
                if let Reference(values) = self {
                    values.iter().zip(goals.iter()).all(|(a, b)| a.satisfies(b))
                } else {
                    false
                }
            } else {
                self == goal
            }
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Unspecified
    }
}

impl State {
    pub(crate) fn symbols(&self) -> Set<usize> {
        fn recurse(set: &mut Set<usize>, value: &Value) {
            use Value::*;
            match value {
                Symbol(s) => {
                    let _ = set.insert(*s);
                }
                Reference(values) => values.iter().for_each(|v| recurse(set, v)),
                _ => {}
            }
        }
        let mut result = Set::default();
        self.registers.iter().for_each(|v| recurse(&mut result, v));
        result
    }

    pub(crate) fn literals(&self) -> Set<u64> {
        fn recurse(set: &mut Set<u64>, value: &Value) {
            use Value::*;
            match value {
                Literal(l) => {
                    let _ = set.insert(*l);
                }
                Reference(values) => values.iter().for_each(|v| recurse(set, v)),
                _ => {}
            }
        }
        let mut result = Set::default();
        self.registers.iter().for_each(|v| recurse(&mut result, v));
        result
    }

    pub(crate) fn alloc_sizes(&self) -> Set<usize> {
        fn recurse(set: &mut Set<usize>, value: &Value) {
            use Value::*;
            match value {
                Reference(v) => {
                    let _ = set.insert(v.len());
                }
                Reference(values) => values.iter().for_each(|v| recurse(set, v)),
                _ => {}
            }
        }
        let mut result = Set::default();
        self.registers.iter().for_each(|v| recurse(&mut result, v));
        result
    }

    /// A goal is reachable if it contains a subset of our symbols.
    pub(crate) fn reachable(&self, goal: &Self) -> bool {
        goal.symbols().is_subset(&self.symbols())
    }

    /// A goal is satisfied if all specified values are in place.
    pub(crate) fn satisfies(&self, goal: &Self) -> bool {
        self.registers
            .iter()
            .zip(goal.registers.iter())
            .all(|(a, b)| a.satisfies(b))
    }

    pub(crate) fn apply(&mut self, transition: &Transition) {
        use Transition::*;
        use Value::*;
        match transition {
            // TODO: Copy/read a reference creates an alias!
            // TODO: Overwriting a reference avoids a drop!
            Set { dest, value } => self.registers[*dest] = Literal(*value),
            Copy { dest, source } => {
                self.registers[*dest] = self.registers[*source].clone();
                if let Value::Reference(..) = &self.registers[*source] {
                    // HACK to avoid some aliasing
                    self.registers[*source] = Value::Unspecified;
                }
            }
            Swap { dest, source } => self.registers[..].swap(*dest, *source),
            Alloc { dest, size } => self.registers[*dest] = Reference(vec![Unspecified; *size]),
            Read {
                dest,
                source,
                offset,
            } => {
                self.registers[*dest] = if let Reference(values) = &self.registers[*source] {
                    values[*offset].clone()
                } else {
                    panic!("Invalid read")
                }
            }
            Write {
                dest,
                offset,
                source,
            } => {
                // TODO: Prevent creating a recursive `Reference`
                let value = self.registers[*source].clone();
                if let Reference(values) = &mut self.registers[*dest] {
                    values[*offset] = value;
                } else {
                    panic!("Invalid write")
                }
            }
        }
    }

    // Heuristic function.
    // Admissable: it is always <= the real cost
    // TODO: Is it consistent?
    fn min_distance(&self, goal: &Self) -> usize {
        if self.satisfies(goal) {
            return 0;
        }
        if !self.reachable(goal) {
            // Not absolute max so we can still add some small costs
            return usize::max_value() >> 2;
        }
        // Compute a sort of hamming distance
        // TODO: Better estimate
        let mut distance = 0;
        for (value, goal) in self.registers.iter().zip(goal.registers.iter()) {
            if !value.satisfies(goal) {
                distance += 3;
            }
        }
        // Add any allocations that need to be done
        // TODO: Multiset
        distance += goal.alloc_sizes().difference(&self.alloc_sizes()).count() * 24;
        // TODO: Make sure this closely matches ts.cost()
        dbg!(distance);
        distance * 100 + 2
    }

    // Generate all potentially useful transitions towards a goal
    // TODO: Return non-allocating generator
    fn transitions(&self, goal: &Self) -> Vec<Transition> {
        let mut result = Vec::default();

        // TODO: Registers unspecified in current and goal are
        // interchangeable, so only pick one.

        // Allocate for goal sizes
        for size in goal.alloc_sizes().into_iter() {
            for dest in 0..=15 {
                result.push(Transition::Alloc { dest, size });
            }
        }

        // Generate Set transitions for each goal literal and register.
        for value in goal.literals().into_iter() {
            for dest in 0..=15 {
                result.push(Transition::Set { dest, value });
            }
        }

        // Copy and swap registers around
        for source in 0..=15 {
            // No point in copying from unspecified regs
            if !self.registers[source].is_specified() {
                continue;
            }

            // Generate moves and swaps between registers
            for dest in 0..=15 {
                // Copy to any reg
                if source != dest {
                    result.push(Transition::Copy { dest, source });
                }
                // Swap two regs
                if source < dest && self.registers[dest].is_specified() {
                    result.push(Transition::Swap { dest, source });
                }
            }

            // Generate reads and writes
            if let Value::Reference(values) = &self.registers[source] {
                for offset in 0..values.len() {
                    for dest in 0..=15 {
                        // TODO: No point in reading unspecified
                        result.push(Transition::Read {
                            dest,
                            source,
                            offset,
                        });

                        // Writes have source and dest flipped
                        if self.registers[dest].is_specified() {
                            result.push(Transition::Write {
                                dest: source,
                                offset,
                                source: dest,
                            });
                        }
                    }
                }
            }
        }

        dbg!(result.len());
        result
    }

    /// Compute the optimal sequence of `Transition`s
    pub(crate) fn transition(&self, goal: &Self) -> Vec<Transition> {
        assert!(self.reachable(goal));

        // Find the optimal transition using pathfinder
        let (path, cost) = astar(
            self,
            |n: &Self| {
                n.transitions(goal)
                    .into_iter()
                    .map(|t| {
                        (
                            {
                                let mut n = n.clone();
                                n.apply(&t);
                                n
                            },
                            t.cost(),
                        )
                    })
                    .collect::<Vec<(Self, usize)>>()
            },
            |n: &Self| n.min_distance(goal),
            |n: &Self| n.satisfies(goal),
        )
        .expect("Could not find valid transition path");

        // Pathfinder gives a list of nodes visited, not the path taken.
        dbg!(path);
        dbg!(cost);

        // TODO: Convert to path by finding best edge between each pair of nodes.

        unimplemented!()
    }
}

impl Transition {
    pub(crate) fn cost(&self) -> usize {
        self.time() * 100 + self.size()
    }

    /// Code size in bytes
    pub(crate) fn size(&self) -> usize {
        // TODO: Create dummy offset only assembler
        let mut asm = SimpleAssembler::new();
        self.assemble(&mut asm);
        asm.offset().0
    }

    /// Run time in clock cycles ⨉ 12
    /// See <https://www.agner.org/optimize/instruction_tables.pdf>
    // TODO: Account for dependency chains
    // TODO: Measure and calibrate these numbers
    pub(crate) fn time(&self) -> usize {
        use Transition::*;
        // Timings are minimum (throughput) from Fog's Skylake table
        match self {
            Set { .. } => 3,
            Copy { .. } => 3,
            // See https://stackoverflow.com/questions/26469196/swapping-2-registers-in-8086-assembly-language16-bits
            // See https://stackoverflow.com/questions/45766444/why-is-xchg-reg-reg-a-3-micro-op-instruction-on-modern-intel-architectures
            Swap { .. } => 12,
            Alloc { .. } => 24, // TODO: Better estimate
            Read { .. } => 6,
            Write { .. } => 12,
        }
    }

    pub(crate) fn assemble<A: DynasmApi>(&self, asm: &mut A) {
        use Transition::*;
        match self {
            Set { dest, value } => {
                if *value == 0 {
                    dynasm!(asm; xor Rq(*dest as u8), Rq(*dest as u8));
                } else if *value <= u32::max_value() as u64 {
                    dynasm!(asm; mov Rd(*dest as u8), DWORD *value as i32);
                } else {
                    dynasm!(asm; mov Rq(*dest as u8), QWORD *value as i64);
                }
            }
            Copy { dest, source } => {
                if dest != source {
                    // TODO: Could use Rd if we know source is 32 bit
                    dynasm!(asm; mov Rq(*dest as u8), Rq(*source as u8));
                }
            }
            Swap { dest, source } => {
                if dest != source {
                    // TODO: Swap order of arguments?
                    dynasm!(asm; xchg Rq(*dest as u8), Rq(*source as u8));
                }
            }
            Read {
                dest,
                source,
                offset,
            } => {
                let offset = 8 * offset;
                dynasm!(asm; mov Rq(*dest as u8), QWORD [Rq(*source as u8) + offset as i32]);
            }
            Write {
                dest,
                offset,
                source,
            } => {
                let offset = 8 * offset;
                dynasm!(asm; mov QWORD [Rq(*dest as u8) + offset as i32], Rq(*source as u8));
            }
            Alloc { dest, size } => {
                // TODO: ram_start as allocator member
                // TODO: Take a generic Allocator as argument
                Bump::alloc(asm, 0x3000, *dest, *size);
            }
        }
    }
}

// impl MachineState {
// fn from_symbols(symbols: &[usize]) -> MachineState {
// assert!(symbols.len() <= 16);
// let mut registers = [None; 16];
// for (index, symbol) in symbols.iter().enumerate() {
// registers[index] = Some(Expression::Symbol(*symbol));
// }
// MachineState { registers }
// }
//
// fn from_expressions(exprs: &[Expression]) -> MachineState {
// assert!(exprs.len() <= 16);
// let mut registers = [None; 16];
// for (index, expr) in exprs.iter().enumerate() {
// registers[index] = Some(expr.clone());
// }
// MachineState { registers }
// }
//
// fn satisfies(&self, other: &MachineState) -> bool {
// for (left, right) in self.registers.iter().zip(other.registers.iter()) {
// if right.is_some() && left != right {
// return false;
// }
// }
// true
// }
//
// Heuristic distance from self to other.
//
// If `self` contains all the values necessary to construct `other`, it
// will return the number of set other registers that do not match
// the self.
//
// If `other` can not be constructed from `self` it will return
// `usize::max_value()`
// fn heuristic_distance(&self, other: &MachineState) -> usize {}
// }

mod test {
    use super::*;

    #[test]
    fn transition_test1() {
        let mut start = State::default();
        start.registers[3] = Value::Symbol(3);
        let mut goal = State::default();
        goal.registers[9] = Value::Reference(vec![Value::Symbol(3)]);
        assert_eq!(start.transition(&goal), vec![]);
    }

    #[test]
    fn transition_test2() {
        let mut start = State::default();
        for i in 0..=3 {
            start.registers[i] = Value::Symbol(i);
        }
        let mut goal = State::default();
        for i in 0..=3 {
            goal.registers[i] = Value::Symbol(3 - i);
        }
        dbg!(&start, &goal);
        assert_eq!(start.transition(&goal), vec![]);
    }
}
