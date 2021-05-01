#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]

mod lexer;
mod parser;
mod ast;
mod desugar;
pub mod mir;

use std::{fs::File, io, io::prelude::*, path::PathBuf};

pub fn parse_file(name: &PathBuf) -> io::Result<mir::Module> {
    // Read file contents
    let mut file = File::open(name)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let contents = contents;

    // Parse
    let mut ast = parser::parse(&contents);
    desugar::desugar(&mut ast);
    let module = mir::Module::from(&ast);
    Ok(module)
}

#[cfg(test)]
mod tests {
    // TODO
}
