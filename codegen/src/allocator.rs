use crate::utils::assemble_read4;
use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi};

pub(crate) fn initial_ram(ram_start: usize) -> Vec<u8> {
    let mut ram = Assembler::new().unwrap();
    dynasm!(ram
        // First 8 bytes are free memory pointer
        ; .qword (ram_start + 8) as i64
    );
    let ram = ram.finalize().expect("Finalize after commit.");
    ram.to_vec()
}

pub(crate) trait Allocator {
    fn alloc(code: &mut Assembler, ram_start: usize, reg: usize, size: usize);
    fn drop(code: &mut Assembler, reg: usize);
}

pub(crate) struct Bump();

impl Allocator for Bump {
    /// Allocate `size` bytes and store the pointer in register `reg`
    fn alloc(code: &mut Assembler, ram_start: usize, reg: usize, size: usize) {
        // Read current free memory pointer
        assemble_read4(code, reg, ram_start);

        // Add size to free memory pointer
        if size <= 127 {
            dynasm!(code
                ; add DWORD [ram_start as i32], BYTE size as i32 // TODO
            );
        } else if size <= (u32::max_value() as usize) {
            dynasm!(code
                ; add DWORD [ram_start as i32], DWORD size as i32
            );
        } else {
            panic!("Can not allocate more than 4GB.");
        }
    }

    /// Deallocate bytes pointed to by register `reg`
    fn drop(_code: &mut Assembler, _reg: usize) {
        // Do nothing
    }
}
