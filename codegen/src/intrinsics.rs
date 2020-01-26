use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi, DynasmLabelApi};

// Syscalls are in r0, r7, r6, r2, r10, r8, r9, returns in r0, r1 clobbers r11
// See <https://github.com/hjl-tools/x86-psABI/wiki/X86-psABI> A.2.1
// See <https://github.com/apple/darwin-xnu/blob/master/bsd/kern/syscalls.master>

// TODO: These intrinsics don't need a closure to be passed. They can have a
// more optimized calling convention.

/// Emit the exit builtin
/// `exit code`
pub fn sys_exit(ops: &mut dynasmrt::x64::Assembler) {
    dynasm!(ops
        // sys_exit(code)
        ; mov r0d, WORD 0x2000001
        ; mov r7, r1
        ; syscall
    );
}

/// Emit the print builtin
/// `print str ret`
pub fn sys_print(ops: &mut dynasmrt::x64::Assembler) {
    dynasm!(ops
        // Back up ret to r15
        ; mov r15, r2
        // sys_write(fd, buffer, length)
        ; mov r0d, WORD 0x2000004
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
pub fn add(ops: &mut dynasmrt::x64::Assembler) {
    dynasm!(ops
        ; add r1, r2
        ; mov r0, r3
        ; jmp QWORD [r0]
    );
}

/// Emit the mul builtin
/// `mul a b ret`
pub fn add(ops: &mut dynasmrt::x64::Assembler) {
    dynasm!(ops
        ; mul r1, r2
        ; mov r0, r3
        ; jmp QWORD [r0]
    );
}
