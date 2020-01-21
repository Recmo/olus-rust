// Required for dynasm!
#![feature(proc_macro_hygiene)]

use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};

use std::fs::File;
use std::io::prelude::*;
use std::io::Write;
use std::{io, mem, slice};

fn macho(code: &[u8]) -> Vec<u8> {
    let mut ops = dynasmrt::x64::Assembler::new().unwrap();

    // See https://pewpewthespells.com/re/Mach-O_File_Format.pdf

    dynasm!(ops
        // Mach-O header (64 bytes)
        ; .dword 0xfeedfacf_u32 as i32 // Magic
        ; .dword 0x01000007_u32 as i32 // Cpu type x86_64
        ; .dword 0x80000003_u32 as i32 // Cpu subtype
        ; .dword 0x2        // Type: executable
        ; .dword 3          // num_commands: 3
        ; .dword 328        // Size of commands
        ; .dword 0x1        // Noun definitions
        ; .dword 0          // Reserved
        // Page zero segment (72 bytes)
        ; .dword 0x19       // Segment command
        ; .dword 72         // command size
        ; .qword 0          // segment name
        ; .qword 0          // segment name
        ; .qword 0          // VM Address
        ; .qword 4096       // VM Size
        ; .qword 0          // File Offset
        ; .qword 0          // File Size
        ; .dword 0          // max protect
        ; .dword 0          // initial protect
        ; .dword 0          // Num sections
        ; .dword 0          // Flags
        // Text segment (72 bytes)
        ; .dword 0x19       // Segment command
        ; .dword 72         // command size
        ; .qword 0          // segment name
        ; .qword 0          // segment name
        ; .qword 4096       // VM Address
        ; .qword 4096       // VM Size
        ; .qword 0          // File Offset
        ; .qword 4096       // File Size
        ; .dword 5          // max protect (R_X)
        ; .dword 5          // initial protect (R_X)
        ; .dword 0          // Num sections
        ; .dword 0          // Flags
        // Unix thread segment (184 bytes)
        ; .dword 0x5        // Segment command
        ; .dword 184        // Command size
        ; .dword 0x4        // Flavour
        ; .dword 42         // Thread state (needs to be 42)
        ; .qword 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,  0, 0, 0, 0 // r0..r15
        ; .qword 4096 + 360 // rip
        ; .qword 0, 0, 0, 0 // rflags, cs, fs, gs
        // Code
        ; .bytes code
        // Padding
        ; .bytes std::iter::repeat(0_u8).take(4096 - 360 - code.len())
    );
    ops.finalize().unwrap()[..].to_owned()
}

fn main() {
    let mut ops = dynasmrt::x64::Assembler::new().unwrap();
    let string = "Hello, World!\n";

    // https://www.idryman.org/blog/2014/12/02/writing-64-bit-assembly-on-mac-os-x/
    // https://censoredusername.github.io/dynasm-rs/language/langref_x64.html#register
    dynasm!(ops
        // syscall number in the rax register
        // arguments are passed on the registers rdi, rsi, rdx, r10, r8 and r9
        ; mov eax, WORD 0x2000004 // sys_write(fd, buffer, length)
        ; mov edi, BYTE 1
        ; lea rsi, [-> hello]
        ; mov edx, BYTE string.len() as _
        ; syscall
        ; mov edi, eax
        ; mov eax, WORD 0x2000001 // sys_exit(code)
        ; syscall
        ; ->hello:
        ; .bytes string.as_bytes()
    );
    println!("Compiling...");

    let buf = ops.finalize().unwrap();
    dbg!(&*buf);
    dbg!(buf.len());

    let mut file = File::create("foo").unwrap();
    file.write_all(&macho(&*buf)).unwrap();

    /*
    println!("Running...");
    let hello_fn: extern "win64" fn() -> bool = unsafe { mem::transmute(buf.ptr(hello)) };
    hello_fn();
    println!("And back!");
    */
}
