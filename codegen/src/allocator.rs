use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi};

pub(crate) fn initial_ram(ram_start: usize) -> Vec<u8> {
    let mut ram = Assembler::new().unwrap();
    dynasm!(ram
        // First 4 bytes are free memory pointer
        ; .qword (ram_start + 4) as i64
    );
    let ram = ram.finalize().expect("Finalize after commit.");
    ram.to_vec()
}

pub(crate) trait Allocator {
    fn alloc<A: DynasmApi>(code: &mut A, ram_start: usize, reg: usize, size: usize);
    fn drop<A: DynasmApi>(code: &mut A, reg: usize);
}

pub(crate) struct Bump();

impl Allocator for Bump {
    /// Allocate `size` bytes and store the pointer in register `reg`
    fn alloc<A: DynasmApi>(asm: &mut A, ram_start: usize, reg: usize, size: usize) {
        // Read current free memory pointer
        // Add size to free memory pointer
        if size <= 127 {
            dynasm!(asm
                ; mov Rd(reg as u8), DWORD [ram_start as i32]
                ; add DWORD [ram_start as i32], BYTE size as i32); // ?
        } else if size <= (u32::max_value() as usize) {
            dynasm!(asm
                ; mov Rd(reg as u8), DWORD [ram_start as i32]
                ; add DWORD [ram_start as i32], DWORD size as i32);
        } else {
            panic!("Can not allocate more than 4GB.");
        }
    }

    /// Deallocate bytes pointed to by register `reg`
    fn drop<A: DynasmApi>(_code: &mut A, _reg: usize) {
        // Do nothing
    }
}
