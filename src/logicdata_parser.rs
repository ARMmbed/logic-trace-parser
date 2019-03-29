use crate::sample::Sample;
use clap::ArgMatches;
use nom::{le_i64, le_u8};
use std::io::Read;

named_args!(
    parse_sample(freq: f64)<&[u8], Sample>,
    do_parse!(
        ts: le_i64 >>
        smp: le_u8 >>
        (Sample::new(smp, (ts as f64)/freq))
    )
);
pub struct LogicDataParser<T>
where
    T: Read,
{
    input: T,
    freq: f64,
}

impl<T> LogicDataParser<T>
where
    T: Read,
{
    pub fn new<'a>(input: T, matches: &ArgMatches<'a>) -> Self {
        let mut freq = value_t!(matches, "freq", f64).unwrap_or_else(|e| e.exit());
        if freq == 0. {
            freq = 1.;
        }
        Self { input, freq }
    }
}

impl<T> Iterator for LogicDataParser<T>
where
    T: Read,
{
    type Item = Result<Sample, String>;
    fn next(&mut self) -> Option<Result<Sample, String>> {
        let mut buffer = [0; 9];

        if self.input.read_exact(&mut buffer).is_ok() {
            match parse_sample(&buffer, self.freq) {
                Ok((_, sample)) => Some(Ok(sample)),
                Err(msg) => Some(Err(format!("{:?}", msg))),
            }
        } else {
            None
        }
    }
}
