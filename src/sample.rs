use crate::logicdata_parser::LogicDataParser;
use crate::vcd_parser::VcdParser;
use clap::ArgMatches;
use std::fmt;

pub struct Sample {
    sample: u8,
    timestamp: f64,
}
impl fmt::Debug for Sample {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:.6} {:08b}", self.timestamp, self.sample)
    }
}
impl Sample {
    pub fn new(sample: u8, timestamp: f64) -> Self {
        Sample { sample, timestamp }
    }
    pub fn sample(&self) -> u8 {
        self.sample
    }
    pub fn timestamp(&self) -> f64 {
        self.timestamp
    }
}

pub enum SampleIterator<T: 'static + std::io::Read> {
    Vcd(bool, VcdParser<T>),
    LogicData(bool, LogicDataParser<T>),
}
impl<T> Iterator for SampleIterator<T>
where
    T: 'static + std::io::Read,
{
    type Item = Sample;
    fn next(&mut self) -> Option<Sample> {
        let (inspect, res) = match self {
            SampleIterator::Vcd(ref inspect, parser) => (inspect, parser.next()),
            SampleIterator::LogicData(ref inspect, parser) => (inspect, parser.next()),
        };
        match res {
            Some(Ok(smp)) => {
                if *inspect {
                    println!("{:?}", smp);
                }
                Some(smp)
            }
            Some(Err(msg)) => {
                eprintln!("{}", msg);
                None
            }
            None => None,
        }
    }
}
impl<T> SampleIterator<T>
where
    T: 'static + std::io::Read,
{
    pub fn new<'a>(reader: T, matches: &ArgMatches<'a>, depth: u64) -> Self {
        let inspect = matches.occurrences_of("v") >= depth;
        if matches.is_present("vcd") {
            SampleIterator::Vcd(inspect, VcdParser::new(reader))
        } else {
            SampleIterator::LogicData(inspect, LogicDataParser::new(reader, matches))
        }
    }
}
