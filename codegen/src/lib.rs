// Required for dynasm!
#![feature(proc_macro_hygiene)]

use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

// Calling convention:
// r0: current closure pointer
// r1..r15: arguments

/// Emit the print builtin
/// `print str ret`
fn println(ops: &mut dynasmrt::x64::Assembler) {
    // TODO: This builtin doesn't need a closure to be passed. It can have a
    // more optimized calling convention.
    dynasm!(ops
        // Back up ret to r15
        ; mov r15, r2
        // sys_write(fd, buffer, length)
        // Syscalls are in r0, r7, r6, r2, r10, r8, r9, returns in r0, r1 clobbers r11
        // See <https://github.com/hjl-tools/x86-psABI/wiki/X86-psABI> A.2.1
        // See <https://github.com/apple/darwin-xnu/blob/master/bsd/kern/syscalls.master>
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

fn zero_pad_to_boundary(vec: &mut Vec<u8>, block_size: usize) {
    let trailing = vec.len() % block_size;
    if trailing > 0 {
        let padding = block_size - trailing;
        vec.extend(std::iter::repeat(0_u8).take(padding));
    }
}

// See <https://pewpewthespells.com/re/Mach-O_File_Format.pdf>
// See <https://github.com/apple/darwin-xnu/blob/master/EXTERNAL_HEADERS/mach-o/loader.h>
// See
// <https://github.com/apple/darwin-xnu/blob/master/bsd/kern/mach_loader.c>
// See
// <https://censoredusername.github.io/dynasm-rs/language/langref_x64.html>
// TODO: Figure out how the stack is allocated (it seems that LC_UNIXTHREAD comes with a stack)
// TODO: Figure out how environment and command line arguments are passed.
// https://embeddedartistry.com/blog/2019/05/20/exploring-startup-implementations-os-x/
fn macho(code: &[u8], rom: &[u8], ram: &[u8]) -> Vec<u8> {
    // Page size
    const PAGE: usize = 4096;
    const RAM_PAGES: usize = 1024; // 4MB RAM

    let num_segments = 4;
    let header_size: usize = 32 + 72 * num_segments + 184;
    let code_pages = (code.len() + header_size + PAGE - 1) / PAGE;
    let rom_pages = (rom.len() + PAGE - 1) / PAGE;
    let ram_init_pages = (ram.len() + PAGE - 1) / PAGE;
    let ram_pages = std::cmp::max(RAM_PAGES, ram_init_pages);

    let mut ops = dynasmrt::x64::Assembler::new().unwrap();

    // All offsets and sizes are in pages
    fn segment(
        ops: &mut dynasmrt::x64::Assembler,
        vm_start: usize,
        vm_size: usize,
        file_start: usize,
        file_size: usize,
        protect: u32,
    ) {
        assert!(vm_size > 0);
        let file_start = if file_size > 0 { file_start } else { 0 };
        dynasm!(ops
            ; .dword 0x19       // Segment command
            ; .dword 72         // command size
            ; .qword 0          // segment name
            ; .qword 0          // segment name
            ; .qword (vm_start * PAGE) as i64   // VM Address
            ; .qword (vm_size * PAGE) as i64     // VM Size
            ; .qword (file_start * PAGE) as i64  // File Offset
            ; .qword (file_size * PAGE) as i64   // File Size
            ; .dword protect as i32    // max protect
            ; .dword protect as i32   // initial protect
            ; .dword 0          // Num sections
            ; .dword 0          // Flags
        );
    }
    let mut vm_offset = 0;
    let mut file_offset = 0;

    // Mach-O header (32 bytes)
    dynasm!(ops
        ; .dword 0xfeedfacf_u32 as i32 // Magic
        ; .dword 0x01000007_u32 as i32 // Cpu type x86_64
        ; .dword 0x80000003_u32 as i32 // Cpu subtype (i386)
        ; .dword 0x2        // Type: executable
        ; .dword (num_segments + 1) as i32         // num_commands
        ; .dword (num_segments * 72 + 184) as i32  // Size of commands
        ; .dword 0x1        // Noun definitions
        ; .dword 0          // Reserved
    );
    // Page zero (___)
    // This is required by XNU for the process to start.
    segment(&mut ops, vm_offset, 1, 0, 0, 0);
    vm_offset += 1;
    // Code (R_X)
    // XNU insists there is one R_X segment starting from the start of the file,
    // even tough this includes the non-executable the Mach-O headers.
    // See <https://github.com/apple/darwin-xnu/blob/a449c6a/bsd/kern/mach_loader.c#L985>
    segment(&mut ops, vm_offset, code_pages, 0, code_pages, 5);
    vm_offset += code_pages;
    file_offset += code_pages;
    // ROM (R__)
    segment(&mut ops, vm_offset, rom_pages, file_offset, rom_pages, 1);
    vm_offset += rom_pages;
    file_offset += rom_pages;
    // RAM (RW_)
    segment(
        &mut ops,
        vm_offset,
        ram_pages,
        file_offset,
        ram_init_pages,
        3,
    );
    vm_offset += rom_pages;
    file_offset += rom_pages;

    // Unix thread segment (184 bytes)
    dynasm!(ops
        ; .dword 0x5        // Segment command
        ; .dword 184        // Command size
        ; .dword 0x4        // Flavour
        ; .dword 42         // Thread state (needs to be 42)
        ; .qword 0, 0, 0, 0 // r0, r3, r1, r2 (rax, rbx, rcx, rdx)
        ; .qword 0, 0, 0, 0 // r7, r6, r5, r4 (rdi, rsi, rbp, rsp)
        ; .qword 0, 0, 0, 0, 0, 0, 0, 0 // r8..r15
        ; .qword (PAGE + header_size) as i64 // rip
        ; .qword 0, 0, 0, 0 // rflags, cs, fs, gs
    );
    // Because `rsp` is zero, a default stack will be allocated by the
    // kernel. This stack will be filled with command line arguments and
    // environment variables. On start the `rsp` register will contain the
    // address of this stack.
    // TODO: Use custom stack to merge overlap (non-functional) stack with
    // RW_ memory and

    let mut result = ops.finalize().unwrap()[..].to_owned();
    assert_eq!(result.len(), header_size);
    result.extend(code);
    zero_pad_to_boundary(&mut result, PAGE);
    assert_eq!(result.len(), code_pages * PAGE);
    result.extend(rom);
    zero_pad_to_boundary(&mut result, PAGE);
    assert_eq!(result.len(), (code_pages + rom_pages) * PAGE);
    result.extend(ram);
    zero_pad_to_boundary(&mut result, PAGE);
    assert_eq!(
        result.len(),
        (code_pages + rom_pages + ram_init_pages) * PAGE
    );
    result
}

pub fn codegen(destination: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut ops = dynasmrt::x64::Assembler::new()?;
    let string = "Hello, World!\n";

    // https://www.idryman.org/blog/2014/12/02/writing-64-bit-assembly-on-mac-os-x/
    // https://censoredusername.github.io/dynasm-rs/language/langref_x64.html#register
    dynasm!(ops
        // syscall number in the rax register
        // arguments are passed on the registers rdi, rsi, rdx, r10, r8 and r9
        ; mov eax, WORD 0x2000004 // sys_write(fd, buffer, length)
        ; mov edi, BYTE 1
        ; mov esi, 0x2000
        ; mov edx, BYTE string.len() as _
        ; syscall
        ; mov DWORD [0x3000], BYTE 42
        ; mov eax, WORD 0x2000001 // sys_exit(code)
        ; mov edi, 0
        ; syscall
    );
    ops.commit()?;
    let buf = ops.finalize().expect("Finalize after commit.");

    let exe = macho(&*buf, &b"Hello, world!\n"[..], &[]);
    {
        let mut file = File::create(destination)?;
        file.write_all(&exe)?;
        file.sync_all()?;
    }
    {
        let mut perms = fs::metadata(destination)?.permissions();
        perms.set_mode(0o755); // rwx r_x r_x
        fs::set_permissions(destination, perms)?;
    }

    /*
    println!("Running...");
    let hello_fn: extern "win64" fn() -> bool = unsafe { mem::transmute(buf.ptr(hello)) };
    hello_fn();
    println!("And back!");
    */
    Ok(())
}
