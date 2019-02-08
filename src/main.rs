#[macro_use]
extern crate nom;

use nom::float;
use std::fmt;
use std::io::BufRead;

fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(input, 16)
}

fn is_hex_digit(c: char) -> bool {
    c.is_digit(16)
}

named!(spi_trace<&str, (f32, u8, u8)>,
  do_parse!(
    ts:     float >>
            tag!(",SPI,MOSI: 0x") >>
    mosi:   byte_from_hex >>
            tag!(";  MISO: 0x") >>
    miso:   byte_from_hex >>
    ((ts, mosi, miso))
  )
);

named!(byte_from_hex<&str, u8>,
  map_res!(take_while_m_n!(2, 2, is_hex_digit), from_hex)
);

#[test]
fn parser_spi_trace() {
    assert_eq!(
        spi_trace("0.002285333333333,SPI,MOSI: 0x00;  MISO: 0x2E"),
        Ok(("", (0.002285333333333f32, 0x00, 0x2E)))
    );
}

struct DebugVec<'a>(&'a Vec<u8>);
impl<'a> fmt::Debug for DebugVec<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

struct Read {
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

struct PageProgram {
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

struct SFDP {
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
struct DeviceId {
    manufacturer: u8,
    device_id: u16,
}

#[derive(Debug)]
struct StatusRegister(u8);

enum Command {
    Read(f32, Read),
    WriteEnable(f32),
    ResetEnable(f32),
    Reset(f32),
    PageProgram(f32, PageProgram),
    BlockErase(f32, u32),
    BlockErase32(f32, u32),
    SectorErase(f32, u32),
    ReadSFDP(f32, SFDP),
    ReadStatusRegister(f32, StatusRegister),
    ReadDeviceId(f32, DeviceId),
}
impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Command::Read(ts, r) => write!(f, "{:.6} {:?}", ts, r),
            Command::WriteEnable(ts) => write!(f, "{:.6} WriteEnable", ts),
            Command::ResetEnable(ts) => write!(f, "{:.6} ResetEnable", ts),
            Command::Reset(ts) => write!(f, "{:.6} Reset", ts),
            Command::PageProgram(ts, pp) => write!(f, "{:.6} {:?}", ts, pp),
            Command::BlockErase(ts, addr) => write!(f, "{:.6} BlockErase({:x})", ts, addr),
            Command::BlockErase32(ts, addr) => write!(f, "{:.6} BlockErase32({:x})", ts, addr),
            Command::SectorErase(ts, addr) => write!(f, "{:.6} SectorErase({:x})", ts, addr),
            Command::ReadSFDP(ts, sfdp) => write!(f, "{:.6} {:?}", ts, sfdp),
            Command::ReadStatusRegister(ts, sr) => write!(f, "{:.6} {:?}", ts, sr),
            Command::ReadDeviceId(ts, did) => write!(f, "{:6} {:?}", ts, did),
        }
    }
}

enum PartialCommand {
    Read(f32, Read),
    ReadStatusRegister(f32),
    PageProgram(f32, PageProgram),
    BlockErase(f32, u32),
    BlockErase32(f32, u32),
    SectorErase(f32, u32),
    ReadSFDP(f32, SFDP),
    ReadDeviceId(f32, DeviceId),
    None,
}
enum ParseResult {
    None,
    Single(Command),
    Pair(Command, Command),
}
struct CommandParser {
    partial: PartialCommand,
    idx: u32,
    prev_ts: f32,
}
impl CommandParser {
    fn new() -> Self {
        CommandParser {
            partial: PartialCommand::None,
            idx: 0,
            prev_ts: 0.,
        }
    }
    fn new_cmd(&mut self, ts: f32, mosi: u8, miso: u8) -> Result<Option<Command>, String> {
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
            0x06 => Ok(Some(Command::WriteEnable(ts))),
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
            0x66 => Ok(Some(Command::ResetEnable(ts))),
            0x99 => Ok(Some(Command::Reset(ts))),
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

            _ => Err(format!("{:.6}: Unsupported cmd {:x}-{:x}", ts, mosi, miso)),
        }
    }

    fn parse(&mut self, ts: f32, mosi: u8, miso: u8) -> Result<ParseResult, String> {
        let res = match self.partial {
            PartialCommand::None => match self.new_cmd(ts, mosi, miso) {
                Ok(Some(cmd)) => Ok(ParseResult::Single(cmd)),
                Ok(None) => Ok(ParseResult::None),
                Err(msg) => Err(msg),
            },
            PartialCommand::Read(ref sts, ref mut r) => {
                let mut res = Ok(ParseResult::None);
                if self.idx < 3 {
                    r.addr = (r.addr << 8) | (mosi as u32);
                    self.idx += 1;
                } else {
                    if mosi != 0 {
                        let sts = *sts;
                        let mut a = Read::new();
                        std::mem::swap(&mut a, r);
                        self.partial = PartialCommand::None;
                        res = match self.new_cmd(ts, mosi, miso) {
                            Ok(Some(b)) => Ok(ParseResult::Pair(Command::Read(sts, a), b)),
                            Ok(None) => Ok(ParseResult::Single(Command::Read(sts, a))),
                            Err(msg) => Err(msg),
                        };
                    } else {
                        r.data.push(miso);
                        self.idx += 1;
                    }
                }
                res
            }
            PartialCommand::ReadStatusRegister(sts) => {
                self.partial = PartialCommand::None;
                Ok(ParseResult::Single(Command::ReadStatusRegister(
                    sts,
                    StatusRegister(miso),
                )))
            }
            PartialCommand::BlockErase(ref sts, ref mut addr) => {
                if self.idx < 2 {
                    *addr = (*addr << 8) | (mosi as u32);
                    self.idx += 1;
                    Ok(ParseResult::None)
                } else {
                    let res = Ok(ParseResult::Single(Command::BlockErase(
                        *sts,
                        (*addr << 8) | (mosi as u32),
                    )));
                    self.partial = PartialCommand::None;
                    res
                }
            }
            PartialCommand::BlockErase32(ref sts, ref mut addr) => {
                if self.idx < 2 {
                    *addr = (*addr << 8) | (mosi as u32);
                    self.idx += 1;
                    Ok(ParseResult::None)
                } else {
                    let res = Ok(ParseResult::Single(Command::BlockErase32(
                        *sts,
                        (*addr << 8) | (mosi as u32),
                    )));
                    self.partial = PartialCommand::None;
                    res
                }
            }
            PartialCommand::SectorErase(ref sts, ref mut addr) => {
                if self.idx < 2 {
                    *addr = (*addr << 8) | (mosi as u32);
                    self.idx += 1;
                    Ok(ParseResult::None)
                } else {
                    let res = Ok(ParseResult::Single(Command::SectorErase(
                        *sts,
                        (*addr << 8) | (mosi as u32),
                    )));
                    self.partial = PartialCommand::None;
                    res
                }
            }
            PartialCommand::PageProgram(ref sts, ref mut pp) => {
                let mut res = Ok(ParseResult::None);
                if self.idx < 3 {
                    pp.addr = (pp.addr << 8) | (mosi as u32);
                    self.idx += 1;
                } else {
                    if (self.prev_ts + 0.0005) < ts {
                        let sts = *sts;
                        let mut a = PageProgram::new();
                        std::mem::swap(&mut a, pp);
                        self.partial = PartialCommand::None;
                        res = match self.new_cmd(ts, mosi, miso) {
                            Ok(Some(b)) => Ok(ParseResult::Pair(Command::PageProgram(sts, a), b)),
                            Ok(None) => Ok(ParseResult::Single(Command::PageProgram(sts, a))),
                            Err(msg) => {
                                println!("dropping: {:?}", a);
                                Err(msg)
                            }
                        };
                    } else {
                        pp.data.push(mosi);
                        self.idx += 1;
                    }
                }
                res
            }
            PartialCommand::ReadSFDP(ref sts, ref mut sfdp) => {
                let mut res = Ok(ParseResult::None);
                if self.idx < 3 {
                    sfdp.addr = (sfdp.addr << 8) | (mosi as u32);
                    self.idx += 1;
                } else {
                    if mosi != 0 {
                        let sts = *sts;
                        let mut a = SFDP::new();
                        std::mem::swap(&mut a, sfdp);
                        self.partial = PartialCommand::None;
                        res = match self.new_cmd(ts, mosi, miso) {
                            Ok(Some(b)) => Ok(ParseResult::Pair(Command::ReadSFDP(sts, a), b)),
                            Ok(None) => Ok(ParseResult::Single(Command::ReadSFDP(sts, a))),
                            Err(msg) => Err(msg),
                        };
                    } else {
                        sfdp.data.push(miso);
                        self.idx += 1;
                    }
                }
                res
            }
            PartialCommand::ReadDeviceId(ref sts, ref mut rdid) => {
                let mut res = Ok(ParseResult::None);
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
                        res = Ok(ParseResult::Single(Command::ReadDeviceId(*sts, *rdid)));
                        self.partial = PartialCommand::None;
                    }
                    _ => unreachable!(),
                }
                res
            }
        };
        self.prev_ts = ts;
        res
    }
}

fn main() {
    let mut trace = Vec::new();
    let mut parser = CommandParser::new();
    let stdin = std::io::stdin();

    for (ts, mosi, miso) in stdin
        .lock()
        .lines()
        .skip(1)
        .map(|l| spi_trace(&l.unwrap()).unwrap().1)
    {
        match parser.parse(ts, mosi, miso) {
            Ok(ParseResult::Single(a)) => {
                println!("{:?}", a);
                trace.push(a);
            }
            Ok(ParseResult::Pair(a, b)) => {
                println!("{:?}", a);
                println!("{:?}", b);
                trace.push(a);
                trace.push(b);
            }
            Ok(ParseResult::None) => {}
            Err(msg) => println!("{}", msg),
        }
        /*
         * read:        0x03 + 3*addr + n
         * fast read:   0x0B + 3*addr + dummy + n
         * 2read:       0xBB + 3*addr + dummy + n
         * dread:       0x3B + 3*addr + dummy + n
         * 4read:       0xEB + 3*addr + dummy + n
         * qread:       0x6B + 3*addr + dummy + n
         * pp:          0x02 + 3*addr
         * 4pp:         0x38 + 3*addr
         * sector erase:0x20 + 3*addr
         * block err32k:0x52 + 3*addr
         * block erase: 0xD8 + 3*addr
         * chip erase:  0x60
         * chip erase:  0xC7
         * rdsfdp:      0x5A + 3*addr + dummy + 1
         * wren:        0x06
         * wrdi:        0x04
         * rdsr:        0x05 + 1
         * rdcr:        0x15 + 2
         * wrsr:        0x01 + 3
         * pgm suspend  0x75
         * erase suspen 0xB0
         * pgm resume   0x7A
         * erase resume 0x30
         * deep power d 0xB9
         * Set Burst Le 0xC0 + 1
         * RDID         0x9F + 2
         * RES          0xAB + 3 + 1
         * REMS         0x90 + 2 + 1 + 3
         * ENSO         0xB1
         * EXSO         0xC1
         * RDSCUR       0x2B + 1
         * WRSCUR       0x2F
         * NOP          0x00
         * RSTEN        0x66
         * RST          0x99
         */
        //        println!("{}: {}->{}", ts, mosi, miso);
    }
}
