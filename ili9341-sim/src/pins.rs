//! Fake control and data pins that drive an [`Ili9341Model`].

use crate::Ili9341Model;
use core::cell::RefCell;
use core::convert::Infallible;
use fake_bus::Clock;
use fake_bus::embedded_hal::digital::{ErrorType, InputPin, OutputPin};
use fake_bus::embedded_hal_async::digital::Wait;

/// How far reading a [`Line::Te`] pin advances the model clock, in nanoseconds.
pub const TE_POLL_NS: u64 = 1_000;

/// The controller input a [`SimPin`] drives.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Line {
    /// `D/CX`: low for commands, high for parameters and pixel data.
    Dc,
    /// `WRX`: the parallel bus latches the data lines on its rising edge.
    Wr,
    /// `RDX`: on the parallel bus, the controller drives the next word of a read onto
    /// the data lines when it falls; see [`Ili9341Model::data_out`].
    Rd,
    /// `TE`: the tearing effect output, read through [`InputPin`] or [`Wait`]. Each
    /// read advances the model clock by [`TE_POLL_NS`], so polling loops make progress.
    Te,
    /// `RESX`: active-low hardware reset.
    Reset,
    /// `D0`–`D15`: a parallel data line.
    Data(u8),
}

/// A pin connected to an [`Ili9341Model`]: an output for the controller inputs, an input
/// for [`Line::Te`].
///
/// All lines share one type, so data lines fit into an array such as `[SimPin; 8]`.
#[derive(Debug, Copy, Clone)]
pub struct SimPin<'a> {
    model: &'a RefCell<Ili9341Model>,
    line: Line,
}

impl<'a> SimPin<'a> {
    /// Creates a pin that drives `line` of `model`.
    pub fn new(model: &'a RefCell<Ili9341Model>, line: Line) -> Self {
        Self { model, line }
    }

    /// Eight data pins `D0`–`D7`.
    pub fn data8(model: &'a RefCell<Ili9341Model>) -> [Self; 8] {
        core::array::from_fn(|n| Self::new(model, Line::Data(n as u8)))
    }

    /// Sixteen data pins `D0`–`D15`.
    pub fn data16(model: &'a RefCell<Ili9341Model>) -> [Self; 16] {
        core::array::from_fn(|n| Self::new(model, Line::Data(n as u8)))
    }

    fn set(&mut self, high: bool) {
        let mut model = self.model.borrow_mut();
        match self.line {
            Line::Dc => model.dc = high,
            Line::Wr => {
                let rising = high && !model.wr;
                model.wr = high;
                let parallel = model.mode != crate::InterfaceMode::Spi;
                if rising && parallel {
                    let (data, dc) = (model.data, model.dc);
                    model.receive(data, dc);
                }
            }
            Line::Rd => model.set_read_line(high),
            // An input of the host.
            Line::Te => {}
            Line::Reset => model.set_reset_line(high),
            Line::Data(n) => {
                let bit = 1 << n;
                model.data = if high {
                    model.data | bit
                } else {
                    model.data & !bit
                };
            }
        }
    }
}

impl ErrorType for SimPin<'_> {
    type Error = Infallible;
}

impl OutputPin for SimPin<'_> {
    fn set_low(&mut self) -> Result<(), Infallible> {
        self.set(false);
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Infallible> {
        self.set(true);
        Ok(())
    }
}

impl InputPin for SimPin<'_> {
    /// The level of the line: `TE` from the model, otherwise what the pin last drove.
    fn is_high(&mut self) -> Result<bool, Infallible> {
        let mut model = self.model.borrow_mut();
        Ok(match self.line {
            Line::Te => {
                model.advance_ns(TE_POLL_NS);
                model.te_level()
            }
            Line::Dc => model.dc,
            Line::Wr => model.wr,
            Line::Rd => model.rd,
            Line::Reset => !model.in_reset,
            Line::Data(n) => model.data >> n & 1 != 0,
        })
    }

    fn is_low(&mut self) -> Result<bool, Infallible> {
        Ok(!self.is_high()?)
    }
}

impl Wait for SimPin<'_> {
    async fn wait_for_high(&mut self) -> Result<(), Infallible> {
        while self.is_low()? {}
        Ok(())
    }

    async fn wait_for_low(&mut self) -> Result<(), Infallible> {
        while self.is_high()? {}
        Ok(())
    }

    async fn wait_for_rising_edge(&mut self) -> Result<(), Infallible> {
        self.wait_for_low().await?;
        self.wait_for_high().await
    }

    async fn wait_for_falling_edge(&mut self) -> Result<(), Infallible> {
        self.wait_for_high().await?;
        self.wait_for_low().await
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Infallible> {
        if self.is_high()? {
            self.wait_for_low().await
        } else {
            self.wait_for_high().await
        }
    }
}
