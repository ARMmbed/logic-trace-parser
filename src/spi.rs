use crate::logic_sample::Sample;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Polarity {
    High,
    Low,
}
impl FromStr for Polarity {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "High" => Ok(Polarity::High),
            "Low" => Ok(Polarity::Low),
            _ => Err("no match"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Phase {
    FirstEdge,
    SecondEdge,
}

#[derive(Debug)]
pub enum SpiEvent {
    ChipSelect(bool),
    Data { mosi: u8, miso: u8 },
}

#[derive(Debug)]
pub struct SpiBuilder {
    cs: u8,
    mosi: u8,
    miso: u8,
    clk: u8,
    phase: Phase,
    polarity: Polarity,
    cs_active_level: Polarity,
}
impl SpiBuilder {
    pub fn new() -> Self {
        Self {
            cs: 0,
            mosi: 1,
            miso: 2,
            clk: 3,
            phase: Phase::FirstEdge,
            polarity: Polarity::High,
            cs_active_level: Polarity::Low,
        }
    }
    pub fn cs(mut self, cs: u8) -> Self {
        self.cs = cs;
        self
    }
    pub fn clk(mut self, clk: u8) -> Self {
        self.clk = clk;
        self
    }
    pub fn miso(mut self, miso: u8) -> Self {
        self.miso = miso;
        self
    }
    pub fn mosi(mut self, mosi: u8) -> Self {
        self.mosi = mosi;
        self
    }
    pub fn mode(mut self, phase: Phase, polarity: Polarity) -> Self {
        self.phase = phase;
        self.polarity = polarity;
        self
    }
    pub fn cs_active_level(mut self, cs_active_level: Polarity) -> Self {
        self.cs_active_level = cs_active_level;
        self
    }
    pub fn into_spi(self) -> Spi {
        Spi {
            ccs: self.cs,
            cmiso: self.miso,
            cmosi: self.mosi,
            cclk: self.clk,

            clk_phase: self.phase == Phase::SecondEdge,
            clk_polarity: self.polarity == Polarity::Low,
            cs_active_level: self.cs_active_level == Polarity::High,

            shift_cnt: 0,
            shift_reg_mosi: 0,
            shift_reg_miso: 0,
            clk: false,
            cs: false,
        }
    }
}

pub struct Spi {
    ccs: u8,
    cmiso: u8,
    cmosi: u8,
    cclk: u8,

    cs_active_level: bool,
    clk_phase: bool,
    clk_polarity: bool,

    shift_reg_mosi: u8,
    shift_reg_miso: u8,
    shift_cnt: u8,
    clk: bool,
    cs: bool,
}
impl fmt::Debug for Spi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Spi {{ mosi: {:02x}, miso: {:02x}, cnt: {} }}",
            self.shift_reg_mosi, self.shift_reg_miso, self.shift_cnt
        )
    }
}
impl Spi {
    pub fn update<F: FnMut(f32, SpiEvent)>(&mut self, smp: &Sample, mut cbk: F) {
        let ts = smp.timestamp();
        let sample = smp.sample();
        let clk = ((sample >> self.cclk) & 1) == 1;
        let cs = ((sample >> self.ccs) & 1) == 1;

        if cs != self.cs {
            self.cs = cs;

            (cbk)(ts, SpiEvent::ChipSelect(cs));
            if cs {
                self.shift_cnt = 0;
            }
        }
        if clk != self.clk {
            self.clk = clk;
            if cs == self.cs_active_level && clk != (self.clk_phase ^ self.clk_polarity) {
                self.shift_reg_mosi =
                    self.shift_reg_mosi.wrapping_shl(1) | ((sample >> self.cmosi) & 1);
                self.shift_reg_miso =
                    self.shift_reg_miso.wrapping_shl(1) | ((sample >> self.cmiso) & 1);
                self.shift_cnt += 1;

                if self.shift_cnt == 8 {
                    self.shift_cnt = 0;

                    (cbk)(
                        ts,
                        SpiEvent::Data {
                            mosi: self.shift_reg_mosi,
                            miso: self.shift_reg_miso,
                        },
                    );
                }
            }
        }
    }
}
