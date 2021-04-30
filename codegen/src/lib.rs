#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]
// Required for dynasm!
#![feature(proc_macro_hygiene)]

mod allocator;
mod code;
mod intrinsics;
mod machine;
mod macho;
mod offset_assembler;
mod rom;
mod utils;

use crate::{
    intrinsics::intrinsic,
    macho::{ram_start, rom_start, Assembly},
};
use bitvec;
use parser::mir::Module;
use std::{collections::HashSet, error::Error, path::PathBuf};

type Set<T> = HashSet<T>;
type BitVec = bitvec::vec::BitVec<bitvec::order::Lsb0, u64>;

pub use offset_assembler::OffsetAssembler;

// For Dynasm syntax see
// <https://censoredusername.github.io/dynasm-rs/language/langref_x64.html#register>

// Oluś default calling convention:
// r0: current closure pointer
// r1..r15: arguments

pub fn codegen(module: &Module, destination: &PathBuf) -> Result<(), Box<dyn Error>> {
    let dummy_code_layout = code::Layout::dummy(module);
    let dummy_rom_layout = rom::Layout::dummy(module);
    // TODO: ram_start and ram_layout

    // First pass with dummy layout
    let (code, code_layout) = code::compile(module, &dummy_code_layout, &dummy_rom_layout, 0);

    // Compile final rom
    let rom_start = rom_start(code.len());
    println!("ROM start: {:08x}", rom_start);
    let (rom, rom_layout) = rom::compile(module, &code_layout, rom_start);
    assert!(rom.len() < 4096);

    // Second pass compile
    let ram_start = ram_start(rom_start, rom.len());
    println!("RAM start: {:08x}", ram_start);
    let (code, code_layout_final) = code::compile(module, &code_layout, &rom_layout, ram_start);
    // Layout should not change between passes
    assert_eq!(code_layout, code_layout_final);

    let ram = allocator::initial_ram(ram_start);
    let assembly = Assembly { code, rom, ram };
    assembly.save(destination)
}
