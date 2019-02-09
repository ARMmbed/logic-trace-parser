#[macro_use]
extern crate nom;

mod spi;
mod spif;

use nom::{le_i64, le_u8};
use spi::Spi;
use spif::Spif;
use std::io::{stdin, Read};

#[derive(Debug)]
struct Sample {
    sample: u8,
    timestamp: i64,
}

named!(
    parse_sample<&[u8], Sample>,
    do_parse!(
        ts: le_i64 >>
        smp: le_u8 >>
        (Sample { sample: smp, timestamp: ts })
    )
);

fn main() {
    let stdin = stdin();
    let mut buffer = [0; 9];
    let mut spif = Spif::new(|ts, cmd| {
        println!("{:.6} {:?}", ts, cmd);
    });
    let mut spi = Spi::new(|ts, ev| {
        //println!("{:.6} {:?}", ts, ev);
        if let Err(msg) = spif.update(ts, ev) {
            eprintln!("{}", msg);
        }
    });

    while stdin.lock().read_exact(&mut buffer).is_ok() {
        let smp = parse_sample(&buffer).unwrap().1;
        spi.update(smp.timestamp, smp.sample);
        //println!("{:?}", smp);
    }
}
