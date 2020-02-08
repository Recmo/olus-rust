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
