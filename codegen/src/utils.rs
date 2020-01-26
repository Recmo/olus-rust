use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi};

pub fn assemble_read4(code: &mut Assembler, reg: usize, address: usize) {
    assert!(address <= (u32::max_value() as usize));
    match reg {
        0 => dynasm!(code; mov r0d, DWORD [address as i32]),
        1 => dynasm!(code; mov r1d, DWORD [address as i32]),
        2 => dynasm!(code; mov r2d, DWORD [address as i32]),
        3 => dynasm!(code; mov r3d, DWORD [address as i32]),
        4 => dynasm!(code; mov r4d, DWORD [address as i32]),
        5 => dynasm!(code; mov r5d, DWORD [address as i32]),
        6 => dynasm!(code; mov r6d, DWORD [address as i32]),
        7 => dynasm!(code; mov r7d, DWORD [address as i32]),
        8 => dynasm!(code; mov r8d, DWORD [address as i32]),
        9 => dynasm!(code; mov r9d, DWORD [address as i32]),
        10 => dynasm!(code; mov r10d, DWORD [address as i32]),
        11 => dynasm!(code; mov r11d, DWORD [address as i32]),
        12 => dynasm!(code; mov r12d, DWORD [address as i32]),
        13 => dynasm!(code; mov r13d, DWORD [address as i32]),
        14 => dynasm!(code; mov r14d, DWORD [address as i32]),
        15 => dynasm!(code; mov r15d, DWORD [address as i32]),
        _ => panic!("Unknown register"),
    }
}

pub fn assemble_literal(code: &mut Assembler, reg: usize, literal: u64) {
    if literal <= u32::max_value().into() {
        let literal = literal as i32;
        match reg {
            0 => dynasm!(code; mov r0d, DWORD literal),
            1 => dynasm!(code; mov r1d, DWORD literal),
            2 => dynasm!(code; mov r2d, DWORD literal),
            3 => dynasm!(code; mov r3d, DWORD literal),
            4 => dynasm!(code; mov r4d, DWORD literal),
            5 => dynasm!(code; mov r5d, DWORD literal),
            6 => dynasm!(code; mov r6d, DWORD literal),
            7 => dynasm!(code; mov r7d, DWORD literal),
            8 => dynasm!(code; mov r8d, DWORD literal),
            9 => dynasm!(code; mov r9d, DWORD literal),
            10 => dynasm!(code; mov r10d, DWORD literal),
            11 => dynasm!(code; mov r11d, DWORD literal),
            12 => dynasm!(code; mov r12d, DWORD literal),
            13 => dynasm!(code; mov r13d, DWORD literal),
            14 => dynasm!(code; mov r14d, DWORD literal),
            15 => dynasm!(code; mov r15d, DWORD literal),
            _ => panic!("Unknown register"),
        }
    } else {
        let literal = literal as i64;
        match reg {
            0 => dynasm!(code; mov r0, QWORD literal),
            1 => dynasm!(code; mov r1, QWORD literal),
            2 => dynasm!(code; mov r2, QWORD literal),
            3 => dynasm!(code; mov r3, QWORD literal),
            4 => dynasm!(code; mov r4, QWORD literal),
            5 => dynasm!(code; mov r5, QWORD literal),
            6 => dynasm!(code; mov r6, QWORD literal),
            7 => dynasm!(code; mov r7, QWORD literal),
            8 => dynasm!(code; mov r8, QWORD literal),
            9 => dynasm!(code; mov r9, QWORD literal),
            10 => dynasm!(code; mov r10, QWORD literal),
            11 => dynasm!(code; mov r11, QWORD literal),
            12 => dynasm!(code; mov r12, QWORD literal),
            13 => dynasm!(code; mov r13, QWORD literal),
            14 => dynasm!(code; mov r14, QWORD literal),
            15 => dynasm!(code; mov r15, QWORD literal),
            _ => panic!("Unknown register"),
        }
    }
}
