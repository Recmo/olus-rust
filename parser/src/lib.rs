#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
mod ast;
mod desugar;
mod parser;
mod tokens;
use memmap::Mmap;
use std::{fs::File, io, path::PathBuf};
pub use unic::UNICODE_VERSION;
pub mod mir;

pub fn parse_file(name: &PathBuf) -> io::Result<mir::Module> {
    let file = File::open(name)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let text = std::str::from_utf8(mmap.as_ref()).expect("Not UTF-8"); // TODO: Convert error
    let mut ast = parser::parse_olus(text);
    desugar::desugar(&mut ast);
    let module = mir::Module::from(&ast);
    Ok(module)
}

#[cfg(test)]
mod tests {
    // TODO
}
