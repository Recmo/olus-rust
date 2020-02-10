use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi};

// TODO: NOP generator <https://stackoverflow.com/a/36361832/4696352>

pub(crate) fn assemble_read4(code: &mut Assembler, reg: usize, address: usize) {
    assert!(address <= (u32::max_value() as usize));
    dynasm!(code; mov Rd(reg as u8), DWORD [address as i32]);
}

pub(crate) fn assemble_literal(code: &mut Assembler, reg: usize, literal: u64) {
    // TODO: XOR for zero?
    if literal <= u32::max_value().into() {
        dynasm!(code; mov Rd(reg as u8), DWORD literal as i32);
    } else {
        dynasm!(code; mov Rq(reg as u8), QWORD literal as i64);
    }
}

pub(crate) fn assemble_mov(code: &mut Assembler, reg: usize, src: usize) {
    dynasm!(code; mov Rq(reg as u8), Rq(src as u8));
}

pub(crate) fn assemble_read(code: &mut Assembler, reg: usize, index: usize) {
    let offset = (8 + 8 * index) as i32;
    dynasm!(code; mov Rq(reg as u8), QWORD [r0 + offset]);
}

// TODO: Look into using PUSH instructions to write closures and POP to read
// them. While we are at it we could use `RET` instead of `JMP *r0`.

pub(crate) fn assemble_write_const(code: &mut Assembler, reg: usize, offset: usize, value: u64) {
    let offset = offset as i32;
    if value <= u32::max_value().into() {
        dynasm!(code; mov QWORD [Rq(reg as u8) + offset], DWORD value as i32);
    } else {
        // TODO: Avoid r15 clobber, could use two 32 bit writes
        dynasm!(code
            ; mov r15, QWORD value as i64
            ; mov QWORD [Rq(reg as u8) + offset], r15
        );
    }
}

pub(crate) fn assemble_write_reg(code: &mut Assembler, reg: usize, offset: usize, src: usize) {
    let offset = offset as i32;
    dynasm!(code; mov QWORD [Rq(reg as u8) + offset], Rq(src as u8));
}

pub(crate) fn assemble_write_read(code: &mut Assembler, reg: usize, offset: usize, index: usize) {
    // TODO: Don't clobber r15
    let read_offset = (8 + 8 * index) as i32;
    let write_offset = offset as i32;
    dynasm!(code
        ; mov r15, QWORD [r0 + read_offset]
        ; mov QWORD [Rq(reg as u8) + write_offset], r15
    );
}
