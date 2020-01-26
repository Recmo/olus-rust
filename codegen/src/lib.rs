// Required for dynasm!
#![feature(proc_macro_hygiene)]

mod intrinsics;
mod macho;

use crate::intrinsics::{sys_exit, sys_print};
use crate::macho::Assembly;
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

// For Dynasm syntax see
// <https://censoredusername.github.io/dynasm-rs/language/langref_x64.html#register>

// Oluś default calling convention:
// r0: current closure pointer
// r1..r15: arguments

pub fn codegen(destination: &PathBuf) -> Result<(), Box<dyn Error>> {
    let string = b"Hello, World!\n";

    let mut code = dynasmrt::x64::Assembler::new()?;
    dynasm!(code
        // Prelude, write rsp to RAM[END-8]. End of ram is initialized with with
        // the OS provided stack frame.
        // TODO: Replace constant with expression
        ; mov QWORD[0x401ff8], rsp

        // Jump to closure at rom zero
        ; mov r0, 0x2000
        ; jmp QWORD [r0]
    );
    dbg!(code.offset());

    // TODO: Don't hardcode rom offset
    dynasm!(code
        ; mov r1, 0x2000 + 8
        ; mov r2, 0x2000 + 26
        ; jmp DWORD (58 - 36) // Jumps are relative to end of instruction
    );
    dbg!(code.offset());

    dynasm!(code
        ; mov r1, 42
        ; jmp DWORD (48 - 48) // Jumps are relative to end of instruction
    );
    dbg!(code.offset());

    // Add intrinsic functions
    dbg!(code.offset());
    sys_exit(&mut code);
    dbg!(code.offset());
    sys_print(&mut code);

    code.commit()?;
    let code = code.finalize().expect("Finalize after commit.");

    // Assemble rom
    let mut rom = dynasmrt::x64::Assembler::new()?;
    dynasm!(rom
        // Closure 0
        ; .qword 0x11f8 + 17
    );
    dbg!(rom.offset());
    dynasm!(rom
        // String
        ; .dword string.len() as i32
        ; .bytes string
    );
    dbg!(rom.offset());
    dynasm!(rom
        // Closure 1
        ; .qword 0x11f8 + 36
    );
    rom.commit()?;
    let rom = rom.finalize().expect("Finalize after commit.");

    let assembly = Assembly {
        code: code.to_vec(),
        rom: rom.to_vec(),
        ram: vec![],
    };
    assembly.save(destination)
}
