use crate::{
    allocator::{Allocator, Bump},
    BitVec, OffsetAssembler,
};
use dynasm::dynasm;
use dynasmrt::{DynasmApi, SimpleAssembler};
use itertools::Itertools;
use pathfinding::directed::{astar::astar, fringe::fringe, idastar::idastar};
use serde::{Deserialize, Serialize};
use std::collections::HashSet as Set;

// TODO: Explore exotic instructions that can potentially accomplish the same
// in fewer bytes/cycles:
// * Immediate writes 1-4 bytes
// * Sign extended immediate loads
// * Stack operators: PUSH, POP
// * String operations: LODS, STOS

type Reg = u8;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
enum Value {
    Unspecified,
    Literal(u64),
    Symbol(usize),
    Reference { index: usize, offset: isize },
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
struct Allocation(Vec<Value>);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
enum Flag {
    Carry     = 0,
    Parity    = 1,
    Adjust    = 2,
    Zero      = 3,
    Sign      = 4,
    Direction = 5,
    Overflow  = 6,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Default)]
struct State {
    registers:   [Value; 16],
    flags:       [Value; 7],
    // TODO: Implement Eq to ignore permutation of allocations.
    allocations: Vec<Allocation>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
struct StateIterator<'a> {
    state: &'a State,
    index: StateIterator,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
enum StateIteratorIndex {
    Register(usize),
    Flags(usize),
    Allocation(usize, usize),
    Done,
}

/// Single instruction
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
enum Transition {
    /// Set register `dest` to literal `value`
    Set { dest: Reg, value: u64 },
    /// Copy register `source` into `dest`
    Copy { dest: Reg, source: Reg },
    /// Swap contents of registers `source` and `dest`
    Swap { dest: Reg, source: Reg },
    /// Read 64 bits from `[source + offset]` into register `dest`
    Read {
        dest:   Reg,
        source: Reg,
        offset: usize,
    },
    /// Write register `source` into `[dest + offset]`
    Write {
        dest:   Reg,
        offset: Reg,
        source: usize,
    },
    /// Allocate empty `Reference` of size `size` in register `dest`
    Alloc { dest: Reg, size: usize },
    /// Drop the allocation referenced to
    Drop { dest: Reg },
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

impl State {
    pub fn is_valid(&self) -> bool {
        use Value::*;
        // Make sure all references are N:1 to allocations
        let seen = BitVec::repeat(false, self.allocations.len());
        for val in &self.registers {
            if let Reference { index, .. } = val {
                if let Some(bit) = seen.get_mut(index) {
                    *bit = false;
                } else {
                    return false;
                }
            }
        }
        for alloc in &self.allocations {
            for val in &alloc {
                if let Reference { index, .. } = val {
                    if let Some(bit) = seen.get_mut(index) {
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
                Literal(n) if n <= 1 => {}
                _ => return false,
            }
        }

        // Otherwise it is valid
        true
    }

    pub fn is_valid_transition(&self, transition: Transition) -> bool {
        //
        unimplemented!()
    }

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

    pub(crate) fn apply(&mut self, transition: Transition) {
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

    fn after(&self, transition: Transition) -> Self {
        let mut result = self.clone();
        result.apply(transition);
        result
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
        result
    }

    /// Compute the optimal sequence of `Transition`s
    pub(crate) fn transition(&self, goal: &Self) -> Vec<Transition> {
        assert!(self.reachable(goal));

        // Find the optimal transition using pathfinder
        let (path, cost) = astar(
            self,
            |n| {
                dbg!(n)
                    .transitions(goal)
                    .iter()
                    .map(|t| (n.after(t), t.cost()))
                    .collect::<Vec<_>>()
            },
            |n| n.min_distance(goal),
            |n| n.satisfies(goal),
        )
        .expect("Could not find valid transition path");
        dbg!(&path);
        dbg!(&cost);

        // Pathfinder gives a list of nodes visited, not the path taken.
        // So take all the pairs of nodes and find the best transition
        // between them.
        let mut result = Vec::default();
        for (from, to) in path.iter().tuple_windows() {
            let mut cost = usize::max_value();
            let mut best = None;
            for transition in from.transitions(goal) {
                let dest = from.after(&transition);
                if dest == *to && transition.cost() < cost {
                    cost = transition.cost();
                    best = Some(transition);
                }
            }
            result.push(best.expect("Could not reproduce path"));
        }
        result
    }
}

impl<'a> IntoIterator for &'a State {
    type Item = Value;
    type IntoIter: StateIterator<'a>;

    fn into_iter(self) -> Self::IntoIter { 
        StateIterator {
            state: self,
            index: StateIteratorIndex::Register(0)
        }
     }
}

impl Transition {
    pub(crate) fn cost(&self) -> usize {
        self.time() * 100 + self.size()
    }

    /// Code size in bytes
    pub(crate) fn size(&self) -> usize {
        let mut asm = OffsetAssembler::default();
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
            Swap { .. } => 6,
            Alloc { .. } => 24, // TODO: Better estimate
            Read { .. } => 6,
            Write { .. } => 12,
        }
    }

    pub(crate) fn apply(state: &mut State) {
        unimplemented!()
    }

    pub(crate) fn assemble<A: DynasmApi>(&self, asm: &mut A) {
        use Transition::*;
        match self {
            Set { dest, value } => {
                if *value == 0 {
                    // See <https://stackoverflow.com/questions/33666617/what-is-the-best-way-to-set-a-register-to-zero-in-x86-assembly-xor-mov-or-and/33668295#33668295>
                    match *dest {
                        // TODO: This clears flags too! -> Separate instruction
                        // TODO: Better encoding
                        // For registers < 8 REX.W is not required
                        0 => dynasm!(asm; xor r0d, r0d),
                        1 => dynasm!(asm; xor r1d, r1d),
                        2 => dynasm!(asm; xor r2d, r2d),
                        3 => dynasm!(asm; xor r3d, r3d),
                        4 => dynasm!(asm; xor r4d, r4d),
                        5 => dynasm!(asm; xor r5d, r5d),
                        6 => dynasm!(asm; xor r6d, r6d),
                        7 => dynasm!(asm; xor r7d, r7d),
                        // Dynamically emit opcode with REX.W
                        // Eventhough it doesn't matter for size, using 32-bit
                        // zero extending helps performance on some processors.
                        d => dynasm!(asm; xor Rd(d as u8), Rd(d as u8)),
                    }
                } else if *value <= u32::max_value() as u64 {
                    match *dest {
                        // For registers < 8 REX.W is not required
                        0 => dynasm!(asm; mov r0d, DWORD *value as i32),
                        1 => dynasm!(asm; mov r1d, DWORD *value as i32),
                        2 => dynasm!(asm; mov r2d, DWORD *value as i32),
                        3 => dynasm!(asm; mov r3d, DWORD *value as i32),
                        4 => dynasm!(asm; mov r4d, DWORD *value as i32),
                        5 => dynasm!(asm; mov r5d, DWORD *value as i32),
                        6 => dynasm!(asm; mov r6d, DWORD *value as i32),
                        7 => dynasm!(asm; mov r7d, DWORD *value as i32),
                        d => dynasm!(asm; mov Rd(d as u8), DWORD *value as i32),
                    }
                } else {
                    dynasm!(asm; mov Rq(*dest as u8), QWORD *value as i64);
                }
            }
            Copy { dest, source } => {
                if dest != source {
                    // TODO: Can avoid REX.W for <8 reg?
                    // TODO: Could use Rd if we know source is 32 bit
                    dynasm!(asm; mov Rq(*dest as u8), Rq(*source as u8));
                }
            }
            Swap { dest, source } => {
                if dest != source {
                    // TODO: Can avoid REX.W for <8 reg?
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
    fn test_set_size() {
        use Transition::*;
        for dest in 0..=7 {
            assert_eq!(Set { dest, value: 0 }.size(), 2);
            assert_eq!(Set { dest, value: 1 }.size(), 5);
            assert_eq!(
                Set {
                    dest,
                    value: (1 << 32) - 1
                }
                .size(),
                5
            );
            assert_eq!(
                Set {
                    dest,
                    value: 1 << 32
                }
                .size(),
                10
            );
            assert_eq!(
                Set {
                    dest,
                    value: u64::max_value()
                }
                .size(),
                10
            );
        }
        for dest in 8..=15 {
            assert_eq!(Set { dest, value: 0 }.size(), 3);
            assert_eq!(Set { dest, value: 1 }.size(), 6);
            assert_eq!(
                Set {
                    dest,
                    value: (1 << 32) - 1
                }
                .size(),
                6
            );
            assert_eq!(
                Set {
                    dest,
                    value: 1 << 32
                }
                .size(),
                10
            );
            assert_eq!(
                Set {
                    dest,
                    value: u64::max_value()
                }
                .size(),
                10
            );
        }
    }

    #[test]
    fn transition_test1() {
        use Value::*;
        let mut start = State::default();
        start.registers[0] = Reference(vec![Literal(1), Symbol(2)]);
        start.registers[1] = Symbol(3);
        let mut goal = State::default();
        goal.registers[0] = Reference(vec![Literal(2)]);
        goal.registers[1] = Reference(vec![Literal(3), Symbol(3)]);
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
