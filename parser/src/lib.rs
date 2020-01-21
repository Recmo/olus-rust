#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
mod AST;
mod desugar;
mod parser;
mod tokens;
use memmap::Mmap;
use std::fs::File;
use std::io;
pub use unic::UNICODE_VERSION;

// Returns a single block containing the contents.
// TODO: Error handling.
pub fn parse_olus(input: &str) -> AST::Statement {
    match parser::block(input) {
        Ok(("", result)) => result,
        _ => panic!("Could not parse Syntax."),
    }
}

pub fn parse_file(name: &str) -> io::Result<AST::Statement> {
    let file = File::open(name)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let text = std::str::from_utf8(mmap.as_ref()).expect("Not UTF-8"); // TODO: Convert error
    let mut ast = parse_olus(text);
    desugar::desugar(&mut ast);
    Ok(ast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};
}
