use clap::{crate_authors, crate_description, crate_version, App, Arg};
use parser::parse_file;

fn main() {
    let args = App::new("Oluś")
        .about(crate_description!())
        .version(crate_version!())
        .author(crate_authors!(",\n"))
        .arg(
            Arg::with_name("verbosity")
                .short("v")
                .multiple(true)
                .help("Increase message verbosity"),
        )
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .help("Silence all output"),
        )
        .get_matches();

    let olus = parse_file("../example.olus");
    println!("{:?}", olus);

    stderrlog::new()
        .verbosity(args.occurrences_of("verbosity") as usize)
        .quiet(args.is_present("quiet"))
        .init()
        .unwrap();

    println!("Unicode version: {}", parser::UNICODE_VERSION);
}
