use crate::sample::SampleIterator;
use crate::spi::{self, SpiEvent};
use clap::{App, ArgMatches, SubCommand};
use std::fmt;

struct DebugVec<'a>(&'a Vec<u8>);
impl<'a> fmt::Debug for DebugVec<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

pub struct Read {
    addr: u32,
    data: Vec<u8>,
}
impl Read {
    fn new() -> Read {
        Read {
            addr: 0,
            data: Vec::new(),
        }
    }
}
impl fmt::Debug for Read {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Read {{ addr: {:06X}, data({:4}): {:?} }}",
            self.addr,
            self.data.len(),
            DebugVec(&self.data)
        )
    }
}

pub struct PageProgram {
    addr: u32,
    data: Vec<u8>,
}
impl PageProgram {
    fn new() -> PageProgram {
        PageProgram {
            addr: 0,
            data: Vec::new(),
        }
    }
}
impl fmt::Debug for PageProgram {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PageProgram {{ addr: {:06X}, data({:4}): {:?} }}",
            self.addr,
            self.data.len(),
            DebugVec(&self.data)
        )
    }
}

pub struct SFDP {
    addr: u32,
    data: Vec<u8>,
}
impl SFDP {
    fn new() -> Self {
        SFDP {
            addr: 0,
            data: Vec::new(),
        }
    }
}
impl fmt::Debug for SFDP {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "SFDP {{ addr: {:06X}, data({:4}): {:?} }}",
            self.addr,
            self.data.len(),
            DebugVec(&self.data)
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DeviceId {
    manufacturer: u8,
    device_id: u16,
}

#[derive(Debug)]
pub struct StatusRegister(u8);

pub enum Command {
    Read(Read),
    WriteEnable,
    ResetEnable,
    Reset,
    PageProgram(PageProgram),
    BlockErase(u32),
    BlockErase32(u32),
    SectorErase(u32),
    ReadSFDP(SFDP),
    ReadStatusRegister(StatusRegister),
    ReadDeviceId(DeviceId),
}
impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Command::Read(r) => r.fmt(f),
            Command::WriteEnable => write!(f, "WriteEnable"),
            Command::ResetEnable => write!(f, "ResetEnable"),
            Command::Reset => write!(f, "Reset"),
            Command::PageProgram(pp) => pp.fmt(f),
            Command::BlockErase(addr) => write!(f, "BlockErase({:x})", addr),
            Command::BlockErase32(addr) => write!(f, "BlockErase32({:x})", addr),
            Command::SectorErase(addr) => write!(f, "SectorErase({:x})", addr),
            Command::ReadSFDP(sfdp) => sfdp.fmt(f),
            Command::ReadStatusRegister(sr) => sr.fmt(f),
            Command::ReadDeviceId(did) => did.fmt(f),
        }
    }
}

enum PartialCommand {
    Read(f64, Read),
    ReadStatusRegister(f64),
    PageProgram(f64, PageProgram),
    BlockErase(f64, u32),
    BlockErase32(f64, u32),
    SectorErase(f64, u32),
    ReadSFDP(f64, SFDP),
    ReadDeviceId(f64, DeviceId),
    None,
}
pub struct Spif<T>
where
    T: Iterator<Item = (f64, SpiEvent)>,
{
    it: T,
    inspect: bool,

    cs: bool,
    idx: u32,
    partial: PartialCommand,
}

impl<T> Spif<T>
where
    T: Iterator<Item = (f64, SpiEvent)>,
{
    fn new_cmd(&mut self, ts: f64, mosi: u8, miso: u8) -> Result<Option<Command>, String> {
        self.idx = 0;
        match mosi {
            0x02 => {
                self.partial = PartialCommand::PageProgram(ts, PageProgram::new());
                Ok(None)
            }
            0x03 => {
                self.partial = PartialCommand::Read(ts, Read::new());
                Ok(None)
            }
            0x05 => {
                self.partial = PartialCommand::ReadStatusRegister(ts);
                Ok(None)
            }
            0x06 => Ok(Some(Command::WriteEnable)),
            0x20 => {
                self.partial = PartialCommand::SectorErase(ts, 0);
                Ok(None)
            }
            0x52 => {
                self.partial = PartialCommand::BlockErase32(ts, 0);
                Ok(None)
            }
            0x5A => {
                self.partial = PartialCommand::ReadSFDP(ts, SFDP::new());
                Ok(None)
            }
            0x66 => Ok(Some(Command::ResetEnable)),
            0x99 => Ok(Some(Command::Reset)),
            0x9F => {
                self.partial = PartialCommand::ReadDeviceId(
                    ts,
                    DeviceId {
                        manufacturer: 0,
                        device_id: 0,
                    },
                );
                Ok(None)
            }
            0xD8 => {
                self.partial = PartialCommand::BlockErase(ts, 0);
                Ok(None)
            }

            _ => Err(format!("{:.6} Unsupported cmd {:x}-{:x}", ts, mosi, miso)),
        }
    }

    fn update(&mut self, ts: f64, ev: SpiEvent) -> Result<Option<(f64, Command)>, String> {
        match ev {
            SpiEvent::ChipSelect(false) => {
                self.cs = false;
                Ok(None)
            }
            SpiEvent::ChipSelect(true) => {
                self.cs = true;
                // finalize current command
                let mut partial = PartialCommand::None;
                std::mem::swap(&mut partial, &mut self.partial);
                match partial {
                    PartialCommand::Read(sts, r) => Ok(Some((sts, Command::Read(r)))),
                    PartialCommand::PageProgram(sts, pp) => {
                        Ok(Some((sts, Command::PageProgram(pp))))
                    }
                    PartialCommand::ReadSFDP(sts, sfdp) => Ok(Some((sts, Command::ReadSFDP(sfdp)))),
                    _ => Ok(None),
                }
            }
            SpiEvent::Data { mosi, miso } if !self.cs => match self.partial {
                PartialCommand::None => match self.new_cmd(ts, mosi, miso) {
                    Ok(Some(cmd)) => Ok(Some((ts, cmd))),
                    Ok(None) => Ok(None),
                    Err(msg) => Err(msg),
                },
                PartialCommand::Read(_, ref mut r) => {
                    if self.idx < 3 {
                        r.addr = (r.addr << 8) | (mosi as u32);
                        self.idx += 1;
                    } else {
                        r.data.push(miso);
                    }
                    Ok(None)
                }
                PartialCommand::ReadStatusRegister(sts) => {
                    self.partial = PartialCommand::None;
                    Ok(Some((
                        sts,
                        Command::ReadStatusRegister(StatusRegister(miso)),
                    )))
                }
                PartialCommand::BlockErase(ref sts, ref mut addr) => {
                    let mut res = None;
                    if self.idx < 2 {
                        *addr = (*addr << 8) | (mosi as u32);
                        self.idx += 1;
                    } else {
                        res = Some((*sts, Command::BlockErase((*addr << 8) | (mosi as u32))));
                        self.partial = PartialCommand::None;
                    }
                    Ok(res)
                }
                PartialCommand::BlockErase32(ref sts, ref mut addr) => {
                    let mut res = None;
                    if self.idx < 2 {
                        *addr = (*addr << 8) | (mosi as u32);
                        self.idx += 1;
                    } else {
                        res = Some((*sts, Command::BlockErase32((*addr << 8) | (mosi as u32))));
                        self.partial = PartialCommand::None;
                    }
                    Ok(res)
                }
                PartialCommand::SectorErase(ref sts, ref mut addr) => {
                    let mut res = None;
                    if self.idx < 2 {
                        *addr = (*addr << 8) | (mosi as u32);
                        self.idx += 1;
                    } else {
                        res = Some((*sts, Command::SectorErase((*addr << 8) | (mosi as u32))));
                        self.partial = PartialCommand::None;
                    }
                    Ok(res)
                }
                PartialCommand::PageProgram(_, ref mut pp) => {
                    if self.idx < 3 {
                        pp.addr = (pp.addr << 8) | (mosi as u32);
                        self.idx += 1;
                    } else {
                        pp.data.push(mosi);
                    }
                    Ok(None)
                }
                PartialCommand::ReadSFDP(_, ref mut sfdp) => {
                    if self.idx < 3 {
                        sfdp.addr = (sfdp.addr << 8) | (mosi as u32);
                        self.idx += 1;
                    } else {
                        sfdp.data.push(miso);
                    }
                    Ok(None)
                }
                PartialCommand::ReadDeviceId(ref sts, ref mut rdid) => {
                    let mut res = None;
                    match self.idx {
                        0 => {
                            rdid.manufacturer = miso;
                            self.idx += 1
                        }
                        1 => {
                            rdid.device_id = (miso as u16) << 8;
                            self.idx += 1
                        }
                        2 => {
                            rdid.device_id |= miso as u16;
                            res = Some((*sts, Command::ReadDeviceId(*rdid)));
                            self.partial = PartialCommand::None;
                        }
                        _ => unreachable!(),
                    }
                    Ok(res)
                }
            },
            _ => Err(format!("Ignoring event: {:?} at {:.6}", ev, ts)),
        }
    }
}

impl<T> Iterator for Spif<T>
where
    T: Iterator<Item = (f64, SpiEvent)>,
{
    type Item = Result<(f64, Command), String>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((ts, ev)) = self.it.next() {
            match self.update(ts, ev) {
                Ok(None) => {}
                Ok(Some((ts, cmd))) => {
                    if self.inspect {
                        println!("{:.6} {:?}", ts, cmd);
                    }
                    return Some(Ok((ts, cmd)));
                }
                Err(msg) => {
                    return Some(Err(msg));
                }
            }
        }
        None
    }
}

impl<T> Spif<spi::Spi<SampleIterator<T>>>
where
    T: 'static + std::io::Read,
{
    pub fn new<'a>(
        input: T,
        matches: &ArgMatches<'a>,
        depth: u64,
    ) -> Spif<spi::Spi<SampleIterator<T>>> {
        let inspect = matches.occurrences_of("v") >= depth;
        let it = spi::Spi::new(input, &matches, depth + 1);
        Self {
            it,
            inspect,
            cs: false,
            idx: 0,
            partial: PartialCommand::None,
        }
    }
}
pub fn subcommand() -> App<'static, 'static> {
    SubCommand::with_name("spif").args(&spi::args())
}
