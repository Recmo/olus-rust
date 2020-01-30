use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi};

// Syscalls are in r0, r7, r6, r2, r10, r8, r9, returns in r0, r1 clobbers r11
// See <https://github.com/hjl-tools/x86-psABI/wiki/X86-psABI> A.2.1
// See <https://github.com/apple/darwin-xnu/blob/master/bsd/kern/syscalls.master>

// TODO: These intrinsics don't need a closure to be passed. They can have a
// more optimized calling convention.

pub(crate) fn intrinsic(ops: &mut Assembler, name: &str) {
    match name {
        "exit" => sys_exit(ops),
        "print" => sys_print(ops),
        "add" => add(ops),
        "sub" => sub(ops),
        "mul" => mul(ops),
        "isZero" => is_zero(ops),
        // TODO:
        "input" => is_zero(ops),
        "parseInt" => is_zero(ops),
        _ => panic!("Unknown intrinsic {}", name),
    }
}

/// Emit the exit builtin
/// `exit code`
fn sys_exit(ops: &mut Assembler) {
    dynasm!(ops
        // sys_exit(code)
        ; mov r0d, WORD 0x0200_0001
        ; mov r7, r1
        ; syscall
    );
}

/// Emit the print builtin
/// `print str ret`
fn sys_print(ops: &mut Assembler) {
    dynasm!(ops
        // Back up ret to r15
        ; mov r15, r2
        // sys_write(fd, buffer, length)
        ; mov r0d, WORD 0x0200_0004
        ; mov r7d, BYTE 1
        ; lea r6, [r1 + 4]
        ; mov r2d, [r1]
        ; syscall
        // call ret from r15
        ; mov r0, r15
        ; jmp QWORD [r0]
    );
}

/// Emit the add builtin
/// `add a b ret`
fn add(ops: &mut Assembler) {
    dynasm!(ops
        ; add r1, r2
        ; mov r0, r3
        ; jmp QWORD [r0]
    );
}

/// Emit the add builtin
/// `sub a b ret`
fn sub(ops: &mut Assembler) {
    dynasm!(ops
        ; sub r1, r2
        ; mov r0, r3
        ; jmp QWORD [r0]
    );
}

/// Emit the mul builtin
/// `mul a b ret`
fn mul(ops: &mut Assembler) {
    dynasm!(ops
        ; mulx r0, r1, r1 // r0:r1 = r1 * r2
        ; mov r0, r3
        ; jmp QWORD [r0]
    );
}

/// Emit the isZero builtin
/// `isZero n true false`
fn is_zero(ops: &mut Assembler) {
    dynasm!(ops
        ; test r1, r1
        ; mov r0, r2
        ; cmovnz r0, r3
        ; jmp QWORD [r0]
    );
}
