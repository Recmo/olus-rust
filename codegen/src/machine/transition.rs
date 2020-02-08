use super::{State, Value};
use crate::OffsetAssembler;
use dynasmrt::DynasmApi;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

pub(crate) type Reg = u8;

/// Single instruction
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub(crate) enum Transition {
    /// Set register `dest` to literal `value`
    Set { dest: Reg, value: u64 },
    /// Copy register `source` into `dest`
    Copy { dest: Reg, source: Reg },
    /// Swap contents of registers `source` and `dest`
    /// (Swap is required in rare cases where no register can be freed. It's
    /// also smaller.)
    Swap { dest: Reg, source: Reg },
    /// Read 64 bits from `[source + offset]` into register `dest`
    Read {
        dest:   Reg,
        source: Reg,
        offset: isize,
    },
    /// Write register `source` into `[dest + offset]`
    Write {
        dest:   Reg,
        offset: isize,
        source: Reg,
    },
    /// Allocate empty `Reference` of size `size` in register `dest`
    Alloc { dest: Reg, size: usize },
    /// Drop the allocation referenced to
    Drop { dest: Reg },
}

impl State {
    fn valid_reg(reg: Reg) -> bool {
        reg < 16
    }

    fn resolve_read(&self, reg: Reg, offset: isize) -> Option<Value> {
        match self.registers.get(reg as usize)? {
            Value::Reference {
                index,
                offset: roffset,
            } => {
                let alloc = self.allocations.get(*index)?;
                let offset: usize = (offset + roffset).try_into().ok()?;
                alloc.0.get(offset).map(|a| *a)
            }
            _ => None,
        }
    }
}

impl Transition {
    pub(crate) fn applies(&self, state: &mut State) -> bool {
        // TODO: Does not check if it overwrites the a last Reference. We could do
        // this quickly by tracking reference counts in Allocations. This is also
        // a good foundation for deferred reference counting, once we implement that.
        use Transition::*;
        use Value::*;
        match self {
            Set { dest, .. } => State::valid_reg(*dest),
            Copy { dest, source } => {
                State::valid_reg(*dest)
                    && State::valid_reg(*source)
                    && state.registers[*source as usize].is_specified()
            }
            Swap { dest, source } => {
                State::valid_reg(*dest)
                    && State::valid_reg(*source)
                    && (state.registers[*dest as usize].is_specified()
                        || state.registers[*source as usize].is_specified())
            }
            Read {
                dest,
                source,
                offset,
            } => {
                State::valid_reg(*dest)
                    && match state.resolve_read(*source, *offset) {
                        Some(val) => val.is_specified(),
                        None => false,
                    }
            }
            Write {
                dest,
                offset,
                source,
            } => {
                State::valid_reg(*source)
                    && state.registers[*source as usize].is_specified()
                    && state.resolve_read(*dest, *offset).is_some()
            }
            Alloc { dest, size } => State::valid_reg(*dest) && *size > 0,
            Drop { dest } => {
                State::valid_reg(*dest)
                    && match state.registers[*dest as usize] {
                        Reference { .. } => true,
                        _ => false,
                    }
            }
        }
    }

    pub(crate) fn apply(&self, state: &mut State) {
        unimplemented!()
    }
}

// Costs
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
            Read { .. } => 6,
            Write { .. } => 12,
            Alloc { .. } => 24, // TODO: Better estimate
            Drop { .. } => 24,  // TODO: Better estimate
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
    use crate::machine::{State, Value};

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
}
