use parser::parse_file;
use std::error::Error;
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
}

fn main() -> Result<(), Box<Error>> {
    // Parse commandline options using structopt
    let options = Options::from_args();
    // TODO: Prinnt unicode version in version info

    // Initialize log output
    stderrlog::new()
        .verbosity(options.verbose)
        .quiet(options.quiet)
        .init()
        .unwrap();

    // Compile
    let olus = parse_file("../example.olus");
    println!("{:?}", olus);

    Ok(())
}
