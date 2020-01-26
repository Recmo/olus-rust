use crate::{macho::RAM_START, utils::assemble_read4};
use dynasm::dynasm;
use dynasmrt::{x64::Assembler, DynasmApi, DynasmLabelApi};

pub trait Allocator {
    fn alloc(code: &mut Assembler, reg: usize, size: usize);
    fn drop(code: &mut Assembler, reg: usize);
}

pub struct Bump();

impl Allocator for Bump {
    /// Allocate `size` bytes and store the pointer in register `reg`
    fn alloc(code: &mut Assembler, reg: usize, size: usize) {
        // Read current free memory pointer
        assemble_read4(code, reg, RAM_START);

        // Add size to free memory pointer
        if size <= 127 {
            dynasm!(code
                ; add DWORD [RAM_START as i32], BYTE (size as i32) // TODO
            );
        } else if size <= (u32::max_value() as usize) {
            dynasm!(code
                ; add DWORD [RAM_START as i32], DWORD size as i32
            );
        } else {
            panic!("Can not allocate more than 4GB.");
        }
    }

    /// Deallocate bytes pointed to by register `reg`
    fn drop(code: &mut Assembler, reg: usize) {
        // Do nothing
    }
}
