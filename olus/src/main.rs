#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]

mod interpreter;

use codegen::codegen;
use interpreter::Interpeter;
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
    let module = parse_file(&options.input)?;

    // Interpret
    let interpreter = Interpeter::new(&module);
    interpreter.eval_by_name("main", &[]);

    // Codegen
    // codegen(&olus, &options.output.unwrap_or("a.out".into()))?;

    Ok(())
}
