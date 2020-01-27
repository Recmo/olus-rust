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

use codegen::codegen;
use parser::parse_file;
use std::{error::Error, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Oluś")]
struct Options {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: usize,

    /// Silence all log output (-q)
    #[structopt(short, long)]
    quiet: bool,

    /// Source file
    #[structopt(parse(from_os_str))]
    input: PathBuf,

    /// Output file, defaults to 'a.out'
    #[structopt(parse(from_os_str))]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse commandline options using structopt
    let options = Options::from_args();
    // TODO: Print unicode version in version info

    // Initialize log output
    stderrlog::new()
        .verbosity(options.verbose)
        .quiet(options.quiet)
        .init()
        .unwrap();

    // Compile
    let olus = parse_file(&options.input)?;
    dbg!(&olus);

    // Codegen
    codegen(&olus, &options.output.unwrap_or("a.out".into()))?;

    Ok(())
}
