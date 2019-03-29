use crate::sample::Sample;
use std::collections::BTreeMap;
use std::io::Read;
use vcd::{Command, IdCode, Parser, TimescaleUnit, Value, VarType};

pub struct VcdParser<T>
where
    T: Read,
{
    input: Parser<T>,
    factor: f64,
    first_ts: f64,
    current_ts: f64,
    vars: BTreeMap<IdCode, usize>,
    state: u8,
    stopped: bool,
}

impl<T> VcdParser<T>
where
    T: Read,
{
    pub fn new(input: T) -> Self {
        Self {
            input: Parser::new(input),
            factor: 1.,
            first_ts: -0.1, // pre-trigger buffer size
            current_ts: -0.1,
            vars: BTreeMap::new(),
            state: 0,
            stopped: false,
        }
    }
}

impl<T> Iterator for VcdParser<T>
where
    T: Read,
{
    type Item = Result<Sample, String>;
    fn next(&mut self) -> Option<Result<Sample, String>> {
        if self.stopped {
            return None;
        }
        while let Some(res) = self.input.next() {
            match res {
                Ok(cmd) => match cmd {
                    Command::Timescale(n, unit) => {
                        self.factor = (n as f64)
                            * match unit {
                                TimescaleUnit::S => 1.,
                                TimescaleUnit::MS => 0.001,
                                TimescaleUnit::US => 0.000001,
                                TimescaleUnit::NS => 0.000000001,
                                TimescaleUnit::PS => 0.000000000001,
                                TimescaleUnit::FS => 0.000000000000001,
                            };
                    }
                    Command::Timestamp(ts) => {
                        let new_ts = (ts as f64) * self.factor;
                        if self.first_ts == -0.1 {
                            self.first_ts = new_ts;
                        }
                        let new_ts = new_ts - self.first_ts - 0.1;
                        assert!(self.current_ts <= new_ts, "Timestamp must be monotonic");
                        self.current_ts = new_ts;
                    }
                    Command::ChangeScalar(id, v) => {
                        let v = match v {
                            Value::V0 => 0,
                            Value::V1 => 1,
                            _ => {
                                self.stopped = true;
                                return Some(Err(format!("Unsupported value : {:?}", v)));
                            }
                        };
                        let shift = self.vars[&id];
                        self.state &= !(1 << shift);
                        self.state |= v << shift;
                        return Some(Ok(Sample::new(self.state, self.current_ts)));
                    }
                    Command::VarDef(ty, _sz, id, name) => {
                        if ty == VarType::Wire {
                            self.vars.insert(
                                id,
                                name.split('_').nth(1).unwrap().parse::<usize>().unwrap(),
                            );
                        } else {
                            return Some(Err(format!("Unsupported VarType: {:?}", ty)));
                        }
                    }
                    _v => {
                        //eprintln!("ignoring: {:?}", v);
                    }
                },
                Err(err) => {
                    return Some(Err(format!("{:?}", err)));
                }
            }
        }
        None
    }
}
