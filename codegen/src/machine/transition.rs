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

// TODO: Explore transforming literals into other literals:
// * Mov8/16/32
// * Add/Sub/Xor

// TODO: Track flags, offer alternatives for XOR zeroing that do not clear
// flags.

// TODO:
// Read constant from memory?

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
                state.registers[dest.as_u8() as usize] =
                    state.get_reference(source, offset).unwrap()
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
            Drop { dest } => {
                // TODO: Make sure all references are gone and remaining references to other
                // allocations have their indices correctly updated. Use swap_remove to make
                // it easier.
                if let Reference { index, .. } = state.get_register(dest) {
                    // Remove Allocation and Reference
                    state.allocations.swap_remove(index);
                    let new = index;
                    let old = state.allocations.len();

                    // Replace all indices `swap` with `index`
                    // Any References to `index` become Unspecified
                    for val in state.registers.iter_mut() {
                        if let Reference { index, .. } = val {
                            if *index == new {
                                *val = Value::Unspecified
                            } else if *index == old {
                                *index = new;
                            }
                        }
                    }
                    for alloc in state.allocations.iter_mut() {
                        for val in alloc.0.iter_mut() {
                            if let Reference { index, .. } = val {
                                if *index == new {
                                    *val = Value::Unspecified
                                } else if *index == old {
                                    *index = new;
                                }
                            }
                        }
                    }
                } else {
                    panic!("Can only Drop a Reference.")
                }
            }
        }
    }
}

// Costs
impl Transition {
    pub(crate) fn cost(&self) -> usize {
        // TODO: In practice, we either want the absolute smallest or absolute
        // fastest code. The middle ground doesn't really exist anymore. The only
        // other trade-off is compile time, which we don't care about at the moment.

        // Optimize for size, with cycles as a potential tie-breaker
        self.size() * 10000 + self.cycles()
    }

    /// Code size in bytes
    pub(crate) fn size(&self) -> usize {
        let mut asm = OffsetAssembler::default();
        self.assemble(&mut asm);
        asm.offset().0
    }

    /// Run time in clock cycles ⨉ 12
    /// Note: It's impossible to be perfectly accurate in time, because it
    /// utimately depends on the non-disclosed internal details of the
    /// specific processor in use. Provided here is a very rough estimate.
    /// See <https://www.agner.org/optimize/instruction_tables.pdf>
    // TODO: Account for dependency chains
    // TODO: Measure and calibrate these numbers
    pub(crate) fn cycles(&self) -> usize {
        use Transition::*;
        // Timings are minimum (throughput) from Fog's Skylake table
        match *self {
            Set { .. } => 3,
            Copy { dest, source } if dest == source => 0,
            Copy { .. } => 3,
            // See https://stackoverflow.com/questions/26469196/swapping-2-registers-in-8086-assembly-language16-bits
            // See https://stackoverflow.com/questions/45766444/why-is-xchg-reg-reg-a-3-micro-op-instruction-on-modern-intel-architectures
            Swap { dest, source } if dest == source => 0,
            Swap { .. } => 6,
            Read { .. } => 6,
            Write { .. } => 12,
            Alloc { .. } => 24, // TODO: Better estimate
            Drop { .. } => 24,  // TODO: Better estimate
        }
    }
}

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
