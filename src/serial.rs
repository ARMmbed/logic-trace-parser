use crate::sample::{Sample, SampleIterator};
use clap::{App, Arg, ArgMatches, SubCommand};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy)]
pub enum SerialEvent {
    Rx(u8),
    Tx(u8),
    Cts(bool),
    Rts(bool),
    TxError(SerialError),
    RxError(SerialError),
}
impl fmt::Debug for SerialEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SerialEvent::Rx(v) => write!(f, "Rx({:?})", v as char),
            SerialEvent::Tx(v) => write!(f, "Tx({:?})", v as char),
            SerialEvent::Cts(b) => write!(f, "Cts({})", b),
            SerialEvent::Rts(b) => write!(f, "Rts({})", b),
            SerialEvent::RxError(e) => write!(f, "RxError({:?})", e),
            SerialEvent::TxError(e) => write!(f, "TxError({:?})", e),
        }
    }
}
impl SerialEvent {
    pub fn is_error(&self) -> bool {
        match self {
            SerialEvent::RxError(_) | SerialEvent::TxError(_) => true,
            _ => false,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum SerialError {
    /// Generated when a framing error is detected
    Framing,
    /// Generated when a parity error is detected
    Parity,
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
#[derive(Debug, PartialEq, Clone, Copy)]
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
    ts: f64,
    data: bool,
    last_fc: bool,
    bit_duration: f64,
    parity: Parity,
    on_data: &'static Fn(u8) -> SerialEvent,
    on_err: &'static Fn(SerialError) -> SerialEvent,
    on_fc: &'static Fn(bool) -> SerialEvent,
}
impl Monitor {
    fn new(
        prefix: &'static str,
        baud: f64,
        parity: Parity,
        on_data: &'static Fn(u8) -> SerialEvent,
        on_err: &'static Fn(SerialError) -> SerialEvent,
        on_fc: &'static Fn(bool) -> SerialEvent,
    ) -> Self {
        Monitor {
            prefix,
            state: MonitorState::Idle,
            ts: -0.1,
            data: true,
            last_fc: false,
            bit_duration: 1. / baud,
            parity,
            on_data,
            on_err,
            on_fc,
        }
    }
    fn update(&mut self, ts: f64, data: bool, fc: bool) -> [Option<(f64, SerialEvent)>; 2] {
        let mut res = [None, None];
        if self.last_fc != fc {
            self.last_fc = fc;
            res[1] = Some((ts, (self.on_fc)(fc)));
        }

        while self.ts < ts {
            let (new_ts, new_state) = match self.state {
                MonitorState::Idle if !data => (ts, MonitorState::Start),
                MonitorState::Idle => (ts, MonitorState::Idle),
                MonitorState::Start if (self.ts + self.bit_duration * 1.5) < ts => (
                    self.ts + self.bit_duration * 1.5,
                    MonitorState::Data(if self.data { 0x80 } else { 0 }, 1),
                ),
                MonitorState::Data(mut reg, mut shift) if (self.ts + self.bit_duration) < ts => {
                    shift += 1;
                    reg >>= 1;
                    if self.data {
                        reg |= 0x80;
                    }
                    (
                        self.ts + self.bit_duration,
                        if shift == 8 {
                            if self.parity != Parity::None {
                                MonitorState::Parity(reg)
                            } else {
                                MonitorState::Stop(reg)
                            }
                        } else {
                            MonitorState::Data(reg, shift)
                        },
                    )
                }
                MonitorState::Parity(_) => unimplemented!(),
                MonitorState::Stop(reg) if (self.ts + self.bit_duration) < ts => {
                    if !self.data {
                        res[0] = Some((self.ts, (self.on_err)(SerialError::Framing)));
                    } else {
                        res[0] = Some((self.ts, (self.on_data)(reg)));
                    }
                    (self.ts + self.bit_duration, MonitorState::Idle)
                }
                _ => {
                    break;
                }
            };
            /*println!(
                "{}: {:.6} {:?} {}-> {:?} ({:.6})",
                self.prefix, self.ts, self.state, self.data, new_state, new_ts
            );*/
            self.state = new_state;
            self.ts = new_ts;
        }
        self.data = data;
        res
    }
    fn finalize(&mut self) -> Option<(f64, SerialEvent)> {
        let res = match self.state {
            MonitorState::Idle => None,
            MonitorState::Start | MonitorState::Data(_, _) | MonitorState::Parity(_) => {
                Some((self.ts, (self.on_err)(SerialError::Framing)))
            }
            MonitorState::Stop(byte) => Some((self.ts, (self.on_data)(byte))),
        };
        self.state = MonitorState::Idle;
        res
    }
}

pub struct Serial<T>
where
    T: Iterator<Item = Sample>,
{
    it: T,
    pending_event: Vec<(f64, SerialEvent)>,
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
            self.pending_event.pop()
        } else {
            while let Some(smp) = self.it.next() {
                let ts = smp.timestamp();
                let smp = smp.sample();

                self.pending_event.extend(
                    self.rx
                        .update(
                            ts,
                            (smp & self.rx_mask) == self.rx_mask,
                            (smp & self.rts_mask) == self.rts_mask,
                        )
                        .iter()
                        .flatten(),
                );
                self.pending_event.extend(
                    self.tx
                        .update(
                            ts,
                            (smp & self.tx_mask) == self.tx_mask,
                            (smp & self.cts_mask) == self.cts_mask,
                        )
                        .iter()
                        .flatten(),
                );
                if self.pending_event.len() > 0 {
                    break;
                }
            }
            if self.pending_event.len() == 0 {
                if let Some(tx) = self.tx.finalize() {
                    self.pending_event.push(tx);
                }
                if let Some(rx) = self.rx.finalize() {
                    self.pending_event.push(rx);
                }
            }
            self.pending_event
                .sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            self.pending_event.pop()
        };
        if self.inspect {
            if let Some((ref ts, ref ev)) = ret {
                println!("{:.6} {:?}", ts, ev);
            }
        }
        ret
    }
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
            pending_event: Vec::with_capacity(4),
            inspect,
            rx_mask,
            rts_mask,
            rx: Monitor::new(
                "rx",
                baud,
                parity,
                &SerialEvent::Rx,
                &SerialEvent::RxError,
                &SerialEvent::Rts,
            ),
            tx_mask,
            cts_mask,
            tx: Monitor::new(
                "tx",
                baud,
                parity,
                &SerialEvent::Tx,
                &SerialEvent::TxError,
                &SerialEvent::Cts,
            ),
        }
    }
    /*
    pub fn new_with_options<'a>(
        input: T,
        matches: &ArgMatches<'a>,
        depth: u64,
        rx: Option<u8>,
        rts: Option<u8>,
        rx: Option<u8>,
        cts: Option<u8>,
        baud: Option<f64>,
        parity: Option<Parity>,
    ) -> Serial<SampleIterator<T>> {
        let inspect = matches.occurrences_of("v") >= depth;

        let tx_mask = 1 << tx.unwrap_or_else(|_| value_t!(matches, "tx", u8).unwrap_or_else(|e| e.exit()));
        let rx_mask = 1 << rx.unwrap_or_else(|_| value_t!(matches, "rx", u8).unwrap_or_else(|e| e.exit()));
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
            pending_event: VecDeque::new(),
            inspect,
            rx_mask,
            rts_mask,
            rx: Monitor::new("rx", baud, parity, &SerialEvent::Rx, &SerialEvent::Rts),
            tx_mask,
            cts_mask,
            tx: Monitor::new("tx", baud, parity, &SerialEvent::Tx, &SerialEvent::Cts),
        }
    }*/
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

pub fn subcommand() -> App<'static, 'static> {
    SubCommand::with_name("serial").args(&args())
}
