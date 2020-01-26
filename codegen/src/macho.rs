use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};
use std::{
    error::Error,
    fs,
    fs::File,
    io::{prelude::*, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
};

// TODO: These are not constant
pub const CODE_START: usize = 0x11f8;
pub const ROM_START: usize = 0x2000;
pub const RAM_START: usize = 0x3000;

const PAGE: usize = 4096;
const RAM_PAGES: usize = 1024; // 4MB RAM

/// The `code`, `rom` and `ram` segments will be extended to 4k page boundaries,
/// concatenated and loaded at address 0x1000. Ram will be extended to 4MB.
pub struct Assembly {
    pub code: Vec<u8>,
    pub rom:  Vec<u8>,
    pub ram:  Vec<u8>,
}

impl Assembly {
    pub fn save(&self, destination: &PathBuf) -> Result<(), Box<dyn Error>> {
        let exe = self.to_macho();
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
        Ok(())
    }

    // NOTE: The documentation on Mach-O is incomplete compared to the source. XNU
    // is substantially stricter than the documentation may appear.
    // See <https://pewpewthespells.com/re/Mach-O_File_Format.pdf>
    // See <https://github.com/apple/darwin-xnu/blob/master/EXTERNAL_HEADERS/mach-o/loader.h>
    // See <https://github.com/apple/darwin-xnu/blob/master/bsd/kern/mach_loader.c>
    pub fn to_macho(&self) -> Vec<u8> {
        let num_segments = 4;
        let header_size: usize = 32 + 72 * num_segments + 184;
        let code_pages = (self.code.len() + header_size + PAGE - 1) / PAGE;
        let rom_pages = (self.rom.len() + PAGE - 1) / PAGE;
        let ram_init_pages = (self.ram.len() + PAGE - 1) / PAGE;
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
        let end_of_ram = code_pages + rom_pages + ram_pages;
        let mut vm_offset = 0;
        let mut file_offset = 0;

        // Mach-O header (32 bytes)
        dynasm!(ops
            ; .dword 0xfeed_facf_u32 as i32 // Magic
            ; .dword 0x0100_0007_u32 as i32 // Cpu type x86_64
            ; .dword 0x8000_0003_u32 as i32 // Cpu subtype (i386)
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

        // Unix thread segment (184 bytes)
        // rip need to be initialized to the start of the program.
        // If rsp is zero, XNU will allocate a stack for the program. XNU requires
        // programs to have a stack and uses it to pass command line and environment
        // arguments. On start rsp will point to the top of the stack. To prevent
        // XNU from allocating an otherwise unecessary stack, but still keep the
        // variables, we set rsp to the top of the RAM. On start, variables will be
        // in [rsp ... end of ram - 8]. The last eight bytes are reserved to store
        // rsp in on start.
        // This initial 'stack' looks like:
        // See <https://github.com/apple/darwin-xnu/blob/master/bsd/kern/kern_exec.c#L3821>
        dynasm!(ops
            ; .dword 0x5        // Segment command
            ; .dword 184        // Command size
            ; .dword 0x4        // Flavour
            ; .dword 42         // Thread state (needs to be 42)
            ; .qword 0, 0, 0, 0 // r0, r3, r1, r2 (rax, rbx, rcx, rdx)
            ; .qword 0, 0, 0    // r7, r6, r5 (rdi, rsi, rbp)
            ; .qword (end_of_ram * PAGE - 8) as i64     // r4 (rsp)
            ; .qword 0, 0, 0, 0, 0, 0, 0, 0 // r8..r15
            ; .qword (PAGE + header_size) as i64 // rip
            ; .qword 0, 0, 0, 0 // rflags, cs, fs, gs
        );

        // Concatenate all the pages
        let mut result = ops.finalize().unwrap()[..].to_owned();
        assert_eq!(result.len(), header_size);
        assert_eq!(result.len(), CODE_START - PAGE);
        result.extend(&self.code);
        zero_pad_to_boundary(&mut result, PAGE);
        assert_eq!(result.len(), code_pages * PAGE);
        result.extend(&self.rom);
        zero_pad_to_boundary(&mut result, PAGE);
        assert_eq!(result.len(), (code_pages + rom_pages) * PAGE);
        result.extend(&self.ram);
        zero_pad_to_boundary(&mut result, PAGE);
        assert_eq!(
            result.len(),
            (code_pages + rom_pages + ram_init_pages) * PAGE
        );
        result
    }
}

fn zero_pad_to_boundary(vec: &mut Vec<u8>, block_size: usize) {
    let trailing = vec.len() % block_size;
    if trailing > 0 {
        let padding = block_size - trailing;
        vec.extend(std::iter::repeat(0_u8).take(padding));
    }
}
