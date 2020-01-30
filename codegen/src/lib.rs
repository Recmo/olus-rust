#![forbid(unsafe_code)]
#![warn(
    // Enable sets of warnings
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    rust_2018_idioms,
    future_incompatible,
    unused,

    // Additional unused warnings (not included in `unused`)
    unused_lifetimes,
    unused_qualifications,
    unused_results,

    // Additional misc. warnings
    anonymous_parameters,
    deprecated_in_future,
    elided_lifetimes_in_paths,
    explicit_outlives_requirements,
    keyword_idents,
    macro_use_extern_crate,
    // TODO: missing_docs,
    missing_doc_code_examples,
    private_doc_tests,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    variant_size_differences
)]
// Required for dynasm!
#![feature(proc_macro_hygiene)]
#![feature(const_in_array_repeat_expressions)]

mod allocator;
mod code;
mod intrinsics;
mod macho;
mod rom;
mod utils;

use crate::{intrinsics::intrinsic, macho::Assembly};
use parser::mir::Module;
use std::{error::Error, path::PathBuf};

// For Dynasm syntax see
// <https://censoredusername.github.io/dynasm-rs/language/langref_x64.html#register>

// Oluś default calling convention:
// r0: current closure pointer
// r1..r15: arguments

// TODO: Two phase: first lay out code, then

pub fn codegen(module: &Module, destination: &PathBuf) -> Result<(), Box<dyn Error>> {
    let rom_layout = rom::layout(module);
    dbg!(&rom_layout);
    let (code, code_layout) = code::compile(module, &rom_layout);
    dbg!(&code_layout);
    let rom = rom::compile(module, &code_layout);
    let ram = vec![];
    let assembly = Assembly { code, rom, ram };
    assembly.save(destination)
}
