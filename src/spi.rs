use std::fmt;

#[derive(Debug)]
pub enum SpiEvent {
    ChipSelect(bool),
    Data { mosi: u8, miso: u8 },
}

pub struct Spi<F>
where
    F: FnMut(f32, SpiEvent),
{
    shift_reg_mosi: u8,
    shift_reg_miso: u8,
    shift_cnt: u8,
    clk: bool,
    cs: bool,

    cbk: F,
}
impl<F> fmt::Debug for Spi<F>
where
    F: FnMut(f32, SpiEvent),
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Spi {{ mosi: {:02x}, miso: {:02x}, cnt: {} }}",
            self.shift_reg_mosi, self.shift_reg_miso, self.shift_cnt
        )
    }
}
impl<F> Spi<F>
where
    F: FnMut(f32, SpiEvent),
{
    pub fn new(func: F) -> Self {
        Spi {
            shift_cnt: 0,
            shift_reg_mosi: 0,
            shift_reg_miso: 0,
            clk: false,
            cs: false,
            cbk: func,
        }
    }

    pub fn update(&mut self, ts: i64, sample: u8) {
        let clk = ((sample >> 0) & 1) == 1;
        let cs = ((sample >> 3) & 1) == 1;

        if cs != self.cs {
            self.cs = cs;

            (self.cbk)((ts as f32) / 3_000_000f32, SpiEvent::ChipSelect(cs));
            if cs {
                self.shift_cnt = 0;
            }
        }
        if clk != self.clk {
            self.clk = clk;
            if !cs && clk {
                self.shift_reg_mosi = self.shift_reg_mosi.wrapping_shl(1) | ((sample >> 1) & 1);
                self.shift_reg_miso = self.shift_reg_miso.wrapping_shl(1) | ((sample >> 2) & 1);
                self.shift_cnt += 1;

                if self.shift_cnt == 8 {
                    self.shift_cnt = 0;

                    (self.cbk)(
                        (ts as f32) / 3_000_000f32,
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
