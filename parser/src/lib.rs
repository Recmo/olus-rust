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

    #[allow(unsafe_code)]
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
