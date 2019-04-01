use crate::sample::SampleIterator;
use crate::serial::{self, SerialError, SerialEvent};
use clap::{App, Arg, ArgMatches, SubCommand};
use std::net::Ipv4Addr;

#[derive(Debug)]
pub enum WizFi310Event {
    Greeting(String),
    Command(String),
    Sent(String),
    Recv(RecvHeader, String),
    Resp(String),
}
#[derive(Debug)]
pub struct RecvHeader {
    socket_id: u8,
    ip: Ipv4Addr,
    port: u16,
}

pub struct Wizfi310<T>
where
    T: Iterator<Item = (f64, SerialEvent)>,
{
    it: T,
    inspect: bool,
    data_to_send: usize,
    data_to_receive: usize,
    recv_header: Option<RecvHeader>,
    // sockets ?
    tx: String,
    rx: String,
}

impl<T> Iterator for Wizfi310<T>
where
    T: Iterator<Item = (f64, SerialEvent)>,
{
    type Item = (f64, WizFi310Event);

    fn next(&mut self) -> Option<Self::Item> {
        let mut res = None;
        while let Some((ts, ev)) = self.it.next() {
            match ev {
                SerialEvent::Tx(c) => {
                    self.tx.push(c as char);

                    if self.data_to_send != 0 {
                        if self.data_to_send == self.tx.chars().count() {
                            self.data_to_send = 0;

                            let mut v = String::new();
                            std::mem::swap(&mut v, &mut self.tx);
                            res = Some((ts, WizFi310Event::Sent(v)));
                        }
                    } else if (c as char) == '\r' {
                        let mut v = String::new();
                        std::mem::swap(&mut v, &mut self.tx);
                        res = Some((ts, WizFi310Event::Command(v)));
                    }
                }
                SerialEvent::Rx(c) => {
                    self.rx.push(c as char);

                    if self.data_to_receive != 0 {
                        if self.data_to_receive == self.rx.chars().count() {
                            self.data_to_receive = 0;

                            let mut v = String::new();
                            std::mem::swap(&mut v, &mut self.rx);
                            res = Some((
                                ts,
                                WizFi310Event::Recv(self.recv_header.take().unwrap(), v),
                            ));
                        }
                    } else if (c as char) == '\n' {
                        if self.rx.starts_with("[") && self.rx.ends_with("]\r\n") {
                            if self.rx.contains(",") {
                                let line: String =
                                    self.rx.chars().skip(1).take(self.rx.len() - 4).collect();
                                self.data_to_send =
                                    line.split(',').last().and_then(|v| v.parse().ok()).unwrap()
                            }
                        }
                        let mut v = String::new();
                        std::mem::swap(&mut v, &mut self.rx);
                        res = Some((ts, WizFi310Event::Resp(v)));
                    } else if (c as char) == '}' {
                        let header = self
                            .rx
                            .chars()
                            .skip(1)
                            .take(self.rx.len() - 2)
                            .collect::<String>();
                        let mut hsplit = header.split(',');
                        let event = RecvHeader {
                            socket_id: hsplit.next().and_then(|v| v.parse().ok()).unwrap(),
                            ip: hsplit.next().and_then(|v| v.parse().ok()).unwrap(),
                            port: hsplit.next().and_then(|v| v.parse().ok()).unwrap(),
                        };
                        self.data_to_receive = hsplit.next().and_then(|v| v.parse().ok()).unwrap();
                        self.recv_header = Some(event);
                        self.rx.clear();
                    }
                }
                _ => {}
            }
            // push byte to appropriate buffer
            // check buffer for completion
            // if rx in data mode:
            //      has buf len reached expected length ?
            //

            if self.inspect {
                if let Some((ref ts, ref s)) = res {
                    println!("{:.6} {:?}", ts, s);
                };
            }
            if let Some((_, ref res)) = res {
                match res {
                    WizFi310Event::Command(s) => {}
                    WizFi310Event::Resp(s) => {}
                    _ => {}
                }
                break;
            }
        }
        res
    }
}

impl<T> Wizfi310<serial::Serial<SampleIterator<T>>>
where
    T: 'static + std::io::Read,
{
    pub fn new<'a>(
        input: T,
        matches: &ArgMatches<'a>,
        depth: u64,
    ) -> Wizfi310<serial::Serial<SampleIterator<T>>> {
        let inspect = matches.occurrences_of("v") >= depth;
        let it = serial::Serial::new(input, &matches, depth + 1);
        Self {
            it,
            inspect,
            data_to_send: 0,
            data_to_receive: 0,
            recv_header: None,
            tx: String::new(),
            rx: String::new(),
        }
    }
}

pub fn subcommand() -> App<'static, 'static> {
    SubCommand::with_name("wizfi310").args(&serial::args())
}
