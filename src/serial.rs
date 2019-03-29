use crate::sample::{Sample, SampleIterator};
use clap::{App, Arg, ArgMatches, SubCommand};
use std::collections::VecDeque;
use std::fmt;
use std::str::FromStr;

#[derive(Clone)]
pub enum SerialEvent {
    Rx(u8),
    Tx(u8),
    Cts(bool),
    Rts(bool),
    Error(SerialError),
}
impl fmt::Debug for SerialEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SerialEvent::Rx(v) => write!(f, "Rx({:?})", v as char),
            SerialEvent::Tx(v) => write!(f, "Tx({:?})", v as char),
            SerialEvent::Cts(b) => write!(f, "Cts({})", b),
            SerialEvent::Rts(b) => write!(f, "Rts({})", b),
            SerialEvent::Error(e) => write!(f, "Error({:?})", e),
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum SerialError {
    /// Generated when a framing error is detected
    Framing,
    /// Generated when a parity error is detected
    Parity,
    /// Generated when data have been transmitted while flow control is expected to prevent it.
    FlowControl,
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum Parity {
    Even,
    Odd,
    Set,
    Clear,
    None,
}
#[derive(Debug, Clone, Copy)]
enum ParityParseError {
    InvalidInput,
}
impl FromStr for Parity {
    type Err = ParityParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Parity::None),
            "even" => Ok(Parity::Even),
            "odd" => Ok(Parity::Odd),
            "set" => Ok(Parity::Set),
            "clear" => Ok(Parity::Clear),
            _ => Err(ParityParseError::InvalidInput),
        }
    }
}
#[derive(Debug, PartialEq)]
enum MonitorState {
    Idle,
    Start,
    Data(u8, u32),
    Parity(u8),
    Stop(u8),
}
struct Monitor {
    prefix: &'static str,
    state: MonitorState,
    last_ts: f64,
    last_v: bool,
    last_fc: bool,
    bit_duration: f64,
    parity: Parity,
    on_data: &'static Fn(u8) -> SerialEvent,
    on_fc: &'static Fn(bool) -> SerialEvent,
}
impl Monitor {
    fn new(
        prefix: &'static str,
        baud: f64,
        parity: Parity,
        on_data: &'static Fn(u8) -> SerialEvent,
        on_fc: &'static Fn(bool) -> SerialEvent,
    ) -> Self {
        Monitor {
            prefix,
            state: MonitorState::Idle,
            last_ts: -0.1,
            last_v: false,
            last_fc: false,
            bit_duration: 1. / baud,
            parity,
            on_data,
            on_fc,
        }
    }
    fn update(&mut self, ts: f64, data: bool, fc: bool) -> Vec<(f64, SerialEvent)> {
        let mut res = Vec::new();
        if self.last_fc != fc {
            self.last_fc = fc;
            res.push((ts, (self.on_fc)(fc)));
        }
        if self.last_v != data {
            //let symbol_len = 9 + if self.parity != Parity::None { 1 } else { 0 };
            let fbits = (ts - self.last_ts) / self.bit_duration;
            let mut bits = fbits.round() as u32;
            let mut new_ts = self.last_ts;
            while bits > 0 {
                let new_state = match self.state {
                    MonitorState::Idle if !self.last_v => MonitorState::Start,
                    MonitorState::Idle => {
                        // previous idle state was wrong
                        MonitorState::Idle
                    }
                    MonitorState::Start => {
                        MonitorState::Data(if self.last_v { 0x80 } else { 0 }, 1)
                    }
                    MonitorState::Data(mut reg, shift) => {
                        reg >>= 1;
                        if self.last_v {
                            reg |= 0x80;
                        }
                        if (shift + 1) == 8 {
                            if self.parity != Parity::None {
                                MonitorState::Parity(reg)
                            } else {
                                MonitorState::Stop(reg)
                            }
                        } else {
                            MonitorState::Data(reg, shift + 1)
                        }
                    }
                    MonitorState::Parity(reg) => unimplemented!(),
                    MonitorState::Stop(reg) => {
                        if !self.last_v {
                            res.push((new_ts, SerialEvent::Error(SerialError::Framing)));
                        } else {
                            res.push((new_ts, (self.on_data)(reg)));
                        }
                        MonitorState::Idle
                    }
                };
                bits -= 1;
                self.state = new_state;
                new_ts += self.bit_duration;
                if self.state == MonitorState::Idle {
                    break;
                }
            }
            self.last_ts = ts;
            self.last_v = data;
        }
        res
    }
}

pub struct Serial<T>
where
    T: Iterator<Item = Sample>,
{
    it: T,
    prev_sample: u8,
    pending_event: VecDeque<(f64, SerialEvent)>,
    inspect: bool,

    // Monitor Rx + RTS
    rx_mask: u8,
    rts_mask: u8,
    rx: Monitor,
    // Monitor Tx + CTS
    tx_mask: u8,
    cts_mask: u8,
    tx: Monitor,
}

impl<T> Iterator for Serial<T>
where
    T: Iterator<Item = Sample>,
{
    type Item = (f64, SerialEvent);
    fn next(&mut self) -> Option<Self::Item> {
        let ret = if self.pending_event.len() > 0 {
            self.pending_event.pop_front()
        } else {
            while let Some(smp) = self.it.next() {
                let ts = smp.timestamp();
                let change = smp.sample() ^ self.prev_sample;
                self.prev_sample = smp.sample();
                if ((change & self.rx_mask) == self.rx_mask)
                    || (((change & self.rts_mask) == self.rts_mask) && self.rts_mask != 0)
                {
                    self.pending_event.extend(self.rx.update(
                        ts,
                        (smp.sample() & self.rx_mask) == self.rx_mask,
                        (smp.sample() & self.rts_mask) == self.rts_mask,
                    ));
                }
                if ((change & self.tx_mask) == self.tx_mask)
                    || (((change & self.cts_mask) == self.cts_mask) && self.cts_mask != 0)
                {
                    self.pending_event.extend(self.tx.update(
                        ts,
                        (smp.sample() & self.tx_mask) == self.tx_mask,
                        (smp.sample() & self.cts_mask) == self.cts_mask,
                    ));
                }
                if self.pending_event.len() > 0 {
                    break;
                }
            }
            self.pending_event.pop_front()
        };
        if self.inspect {
            if let Some((ref ts, ref ev)) = ret {
                println!("{:.6} {:?}", ts, ev);
            }
        }
        ret
    }
}

pub fn args() -> [Arg<'static, 'static>; 7] {
    [
        Arg::from_usage("--tx [tx] 'Channel used for the tx pin'").default_value("0"),
        Arg::from_usage("--rx [rx] 'Channel used for the rx pin'").default_value("1"),
        Arg::from_usage("--rts [rts] 'Channel used for the rts pin'"),
        Arg::from_usage("--cts [cts] 'Channel used for the cts pin'"),
        Arg::from_usage("-b --baud [baudrate] 'Serial line baudrate'").default_value("auto"),
        Arg::from_usage("-p --parity [parity] 'Serial line parity'")
            .possible_values(&["even", "odd", "clear", "set", "none"])
            .default_value("none"),
        Arg::from_usage("-s --stop [stop] 'Serial line stop bit length'").default_value("1"),
    ]
}

impl<T> Serial<SampleIterator<T>>
where
    T: 'static + std::io::Read,
{
    pub fn new<'a>(input: T, matches: &ArgMatches<'a>, depth: u64) -> Serial<SampleIterator<T>> {
        let inspect = matches.occurrences_of("v") >= depth;

        let tx_mask = 1 << value_t!(matches, "tx", u8).unwrap_or_else(|e| e.exit());
        let rx_mask = 1 << value_t!(matches, "rx", u8).unwrap_or_else(|e| e.exit());
        let rts_mask = if let Some(v) = matches.value_of("rts") {
            match v.parse::<u8>() {
                Ok(val) => 1 << val,
                Err(_) => ::clap::Error::value_validation_auto(
                    "the argument 'rts' isn't a valid value".to_string(),
                )
                .exit(),
            }
        } else {
            0
        };
        let cts_mask = if let Some(v) = matches.value_of("cts") {
            match v.parse::<u8>() {
                Ok(val) => 1 << val,
                Err(_) => ::clap::Error::value_validation_auto(
                    "the argument 'cts' isn't a valid value".to_string(),
                )
                .exit(),
            }
        } else {
            0
        };
        let baud = if let Some(baud) = matches.value_of("baud") {
            if baud == "auto" {
                ::clap::Error::with_description(
                    "Auto baudrate detection not yet implemented",
                    ::clap::ErrorKind::ValueValidation,
                )
                .exit();
            } else {
                match baud.parse::<u32>() {
                    Ok(val) => val as f64,
                    Err(_) => ::clap::Error::value_validation_auto(
                        "the argument 'baud' isn't a valid value".to_string(),
                    )
                    .exit(),
                }
            }
        } else {
            unreachable!();
        };
        let parity = value_t!(matches, "parity", Parity).unwrap_or_else(|e| e.exit());

        Serial {
            it: SampleIterator::new(input, matches, depth + 1),
            prev_sample: 0,
            pending_event: VecDeque::new(),
            inspect,
            rx_mask,
            rts_mask,
            rx: Monitor::new("rx", baud, parity, &SerialEvent::Rx, &SerialEvent::Rts),
            tx_mask,
            cts_mask,
            tx: Monitor::new("tx", baud, parity, &SerialEvent::Tx, &SerialEvent::Cts),
        }
    }
}
pub fn subcommand() -> App<'static, 'static> {
    SubCommand::with_name("serial").args(&args())
}
