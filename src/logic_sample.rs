use nom::{le_i64, le_u8};
use std::io::Read;

#[derive(Debug)]
pub struct Sample {
    sample: u8,
    timestamp: f32,
}
impl Sample {
    pub fn sample(&self) -> u8 {
        self.sample
    }
    pub fn timestamp(&self) -> f32 {
        self.timestamp
    }
}

named_args!(
    parse_sample(freq: f32)<&[u8], Sample>,
    do_parse!(
        ts: le_i64 >>
        smp: le_u8 >>
        (Sample { sample: smp, timestamp: (ts as f32)/freq })
    )
);
pub struct SampleIterator<T>
where
    T: Read,
{
    input: T,
    freq: f32,
}

impl<T> SampleIterator<T>
where
    T: Read,
{
    pub fn new(input: T, freq: f32) -> Self {
        Self { input, freq }
    }
}

impl<T> Iterator for SampleIterator<T>
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
