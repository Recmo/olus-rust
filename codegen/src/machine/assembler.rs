use super::Transition;
use crate::allocator::{Allocator, Bump};
use dynasm::dynasm;
use dynasmrt::DynasmApi;
use std::convert::TryInto;

impl Transition {
    pub(crate) fn assemble<A: DynasmApi>(&self, asm: &mut A) {
        use Transition::*;
        match *self {
            Set { dest, value } => {
                // TODO: MOVABS?
                // TODO: Load from ROM?
                if value == 0 {
                    // See <https://stackoverflow.com/questions/33666617/what-is-the-best-way-to-set-a-register-to-zero-in-x86-assembly-xor-mov-or-and/33668295#33668295>
                    match dest.as_u8() {
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
                        d => dynasm!(asm; xor Rd(d), Rd(d)),
                    }
                } else if value <= u32::max_value() as u64 {
                    // asm.push(0xb8 | (dest.as_u8() & 7));
                    match dest.as_u8() {
                        // For registers < 8 REX.W is not required
                        0 => dynasm!(asm; mov r0d, DWORD value as i32),
                        1 => dynasm!(asm; mov r1d, DWORD value as i32),
                        2 => dynasm!(asm; mov r2d, DWORD value as i32),
                        3 => dynasm!(asm; mov r3d, DWORD value as i32),
                        4 => dynasm!(asm; mov r4d, DWORD value as i32),
                        5 => dynasm!(asm; mov r5d, DWORD value as i32),
                        6 => dynasm!(asm; mov r6d, DWORD value as i32),
                        7 => dynasm!(asm; mov r7d, DWORD value as i32),
                        d => dynasm!(asm; mov Rd(d), DWORD value as i32),
                    }
                } else {
                    dynasm!(asm; mov Rq(dest.as_u8()), QWORD value as i64);
                }
            }
            Copy { dest, source } => {
                if dest == source {
                    return;
                }
                // TODO: Could use Rd without REX if we know source is 32 bit
                dynasm!(asm; mov Rq(dest.as_u8()), Rq(source.as_u8()));
            }
            Swap { dest, source } => {
                if dest == source {
                    return;
                }
                // TODO: Swap order of arguments?
                if dest.as_u8() == 0 {
                    dynasm!(asm; xchg rax, Rq(source.as_u8()));
                } else if source.as_u8() == 0 {
                    dynasm!(asm; xchg Rq(dest.as_u8()), rax);
                } else {
                    dynasm!(asm; xchg Rq(dest.as_u8()), Rq(source.as_u8()));
                }
            }
            Read {
                dest,
                source,
                offset,
            } => {
                let offset = (8 * offset) as i32;
                if let Ok(offset) = offset.try_into() {
                    dynasm!(asm; mov Rq(dest.as_u8()), QWORD [BYTE Rq(source.as_u8()) + offset]);
                } else {
                    dynasm!(asm; mov Rq(dest.as_u8()), QWORD [DWORD Rq(source.as_u8()) + offset]);
                }
            }
            Write {
                dest,
                offset,
                source,
            } => {
                let offset = (8 * offset) as i32;
                if let Ok(offset) = offset.try_into() {
                    dynasm!(asm; mov QWORD [BYTE Rq(dest.as_u8()) + offset], Rq(source.as_u8()));
                } else {
                    dynasm!(asm; mov QWORD [DWORD Rq(dest.as_u8()) + offset], Rq(source.as_u8()));
                }
            }
            Alloc { dest, size } => {
                // TODO: ram_start as allocator member
                // TODO: Take a generic Allocator as argument
                Bump::alloc(asm, 0x3000, dest.as_u8() as usize, size);
            }
            Drop { dest } => {
                Bump::drop(asm, dest.as_u8() as usize);
            }
        }
    }
}
