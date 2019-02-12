#[macro_use]
extern crate nom;

#[macro_use]
extern crate clap;

use clap::{App, Arg};

mod logic_sample;
mod spi;
mod spif;

use logic_sample::SampleIterator;
use spi::{Phase, Polarity, SpiBuilder};
use spif::Spif;
use std::io::stdin;

fn main() {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .args(&[
            Arg::from_usage("-f, --freq [freq] 'Sample frequency'").default_value("1."),
            Arg::from_usage("--cs [cs] 'Channel used for the chip select.'").default_value("0"),
            Arg::from_usage("--miso [miso] 'Channel used for miso'").default_value("1"),
            Arg::from_usage("--mosi [mosi] 'Channel used for mosi'").default_value("2"),
            Arg::from_usage("--clk [clk] 'Channel used for the clock'").default_value("3"),
            Arg::from_usage("-l, --cs_active_level [cs_active_level] 'Chip select active level'")
                .possible_values(&["High", "Low"])
                .default_value("Low"),
            Arg::from_usage("-m --mode [mode] 'Spi mode'")
                .possible_values(&["0", "1", "2", "3"])
                .default_value("0"),
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        ])
        .get_matches();

    let print_samples = matches.occurrences_of("v") >= 2;
    let print_spi = matches.occurrences_of("v") >= 1;

    let mut freq = value_t_or_exit!(matches, "freq", f32);
    if freq == 0. {
        freq = 1.;
    }
    let stdin = stdin();
    let it = SampleIterator::new(stdin.lock(), freq);
    let (phase, polarity) = match value_t_or_exit!(matches, "mode", u8) {
        1 => (Phase::SecondEdge, Polarity::High),
        2 => (Phase::FirstEdge, Polarity::Low),
        3 => (Phase::SecondEdge, Polarity::Low),
        0 | _ => (Phase::FirstEdge, Polarity::High),
    };

    let mut spi = SpiBuilder::new()
        .cs(value_t_or_exit!(matches, "cs", u8))
        .miso(value_t_or_exit!(matches, "miso", u8))
        .mosi(value_t_or_exit!(matches, "mosi", u8))
        .clk(value_t_or_exit!(matches, "clk", u8))
        .mode(phase, polarity)
        .cs_active_level(value_t_or_exit!(matches, "cs_active_level", Polarity))
        .into_spi();
    let mut spif = Spif::new();

    for res in it {
        match res {
            Ok(smp) => {
                if print_samples {
                    println!("{:?}", smp);
                }
                spi.update(&smp, |ts, ev| {
                    if print_spi {
                        println!("{:.6} {:?}", ts, ev);
                    }
                    match spif.update(ts, ev) {
                        Ok(Some((ts, cmd))) => println!("{:.6} {:?}", ts, cmd),
                        Err(msg) => eprintln!("{}", msg),
                        _ => {}
                    }
                })
            }
            Err(msg) => eprintln!("{:?}", msg),
        };
    }
}
