#[macro_use]
extern crate nom;

#[macro_use]
extern crate clap;

use clap::{App, AppSettings, Arg};

mod logicdata_parser;
mod sample;
mod serial;
mod spi;
mod spif;
mod vcd_parser;
mod wizfi310;

use std::io::{stdin, Read};

fn main() {
    let matches = App::new(crate_name!())
        .setting(AppSettings::UnifiedHelpMessage)
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(Arg::from_usage("--vcd 'Input is a vcd file'").global(true))
        .subcommand(spi::subcommand())
        .subcommand(spif::subcommand())
        .subcommand(serial::subcommand())
        .subcommand(wizfi310::subcommand())
        .args(&[
            Arg::from_usage("-f, --freq [freq] 'Sample frequency (only used on binary input)'")
                .default_value("1.")
                .global(true),
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity")
                .global(true),
            Arg::with_name("file")
                .help("Input file. If not provided, stdin will be used.")
                .global(true),
        ])
        .get_matches();

    let input: Box<Read> = if let Some(path) = matches.value_of("file") {
        Box::new(std::fs::File::open(path).unwrap_or_else(|e| {
            clap::Error::with_description(&format!("{:?}", e), clap::ErrorKind::ValueValidation)
                .exit()
        }))
    } else {
        Box::new(stdin())
    };

    match matches.subcommand() {
        ("spif", Some(matches)) => spif::Spif::new(input, &matches, 0).for_each(|_| {}),
        ("spi", Some(matches)) => spi::Spi::new(input, &matches, 0).for_each(|_| {}),
        ("serial", Some(matches)) => serial::Serial::new(input, &matches, 0).for_each(|_| {}),
        ("wizfi310", Some(matches)) => wizfi310::Wizfi310::new(input, &matches, 0).for_each(|_| {}),
        _ => sample::SampleIterator::new(input, &matches, 0).for_each(|_| {}),
    }
}
