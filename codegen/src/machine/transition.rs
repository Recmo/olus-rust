use super::{Allocation, Register, State, Value};
use crate::OffsetAssembler;
use dynasmrt::DynasmApi;
use serde::{Deserialize, Serialize};

// TODO: Explore exotic instructions that can potentially accomplish the same
// in fewer bytes/cycles:
// * Immediate writes 1-4 bytes
// * Sign extended immediate loads
// * Stack operators: PUSH, POP
// * String operations: LODS, STOS

/// Single instruction
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub(crate) enum Transition {
    /// Set register `dest` to literal `value`
    Set { dest: Register, value: u64 },
    /// Copy register `source` into `dest`
    Copy { dest: Register, source: Register },
    /// Swap contents of registers `source` and `dest`
    /// (Swap is required in rare cases where no register can be freed. It's
    /// also smaller.)
    Swap { dest: Register, source: Register },
    /// Read 64 bits from `[source + offset]` into register `dest`
    Read {
        dest:   Register,
        source: Register,
        offset: isize,
    },
    /// Write register `source` into `[dest + offset]`
    Write {
        dest:   Register,
        offset: isize,
        source: Register,
    },
    /// Allocate empty `Reference` of size `size` in register `dest`
    Alloc { dest: Register, size: usize },
    /// Drop the allocation referenced to
    Drop { dest: Register },
}

impl Transition {
    pub(crate) fn applies(&self, state: &State) -> bool {
        // TODO: Does not check if it overwrites a last Reference. We could do
        // this quickly by tracking reference counts in Allocations. This is also
        // a good foundation for deferred reference counting, once we implement that.
        use Transition::*;
        use Value::*;
        match *self {
            Set { dest, .. } => true,
            Copy { dest, source } => state.get_register(source).is_specified(),
            Swap { dest, source } => {
                state.get_register(dest).is_specified() || state.get_register(source).is_specified()
            }
            Read {
                dest,
                source,
                offset,
            } => {
                match state.get_reference(source, offset) {
                    Some(val) => val.is_specified(),
                    None => false,
                }
            }
            Write {
                dest,
                offset,
                source,
            } => {
                state.get_register(source).is_specified()
                    && state.get_reference(dest, offset).is_some()
            }
            Alloc { dest, size } => size > 0,
            Drop { dest } => {
                match state.get_register(dest) {
                    Reference { .. } => true,
                    _ => false,
                }
            }
        }
    }

    pub(crate) fn apply(&self, state: &mut State) {
        use Transition::*;
        use Value::*;
        debug_assert!(self.applies(state));
        match *self {
            Set { dest, value } => state.registers[dest.as_u8() as usize] = Literal(value),
            Copy { dest, source } => {
                state.registers[dest.as_u8() as usize] = state.get_register(source)
            }
            Swap { dest, source } => {
                state
                    .registers
                    .as_mut()
                    .swap(dest.as_u8() as usize, source.as_u8() as usize)
            }
            Read {
                dest,
                source,
                offset,
            } => {
                state.registers[dest.as_u8() as usize] = state.get_reference(dest, offset).unwrap()
            }
            Write {
                dest,
                offset,
                source,
            } => *(state.get_mut_reference(dest, offset).unwrap()) = state.get_register(source),
            Alloc { dest, size } => {
                state.registers[dest.as_u8() as usize] = Reference {
                    index:  state.allocations.len(),
                    offset: 0,
                };
                state.allocations.push(Allocation(vec![Unspecified; size]));
            }
            Drop { .. } => {
                // TODO: Make sure all references are gone and remaining references to other
                // allocations have their indices correctly updated. Use swap_remove to make
                // it easier.
                unimplemented!()
            }
        }
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_set_size() {
        use Transition::*;
        for dest in (0..=7).map(Register) {
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
        for dest in (8..=15).map(Register) {
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
