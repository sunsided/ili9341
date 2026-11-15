//! Fake control and data pins that drive an [`Ili9341Model`].

use crate::Ili9341Model;
use core::cell::RefCell;
use core::convert::Infallible;
use fake_bus::embedded_hal::digital::{ErrorType, OutputPin};

/// The controller input a [`SimPin`] drives.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Line {
    /// `D/CX`: low for commands, high for parameters and pixel data.
    Dc,
    /// `WRX`: the parallel bus latches the data lines on its rising edge.
    Wr,
    /// `RESX`: active-low hardware reset.
    Reset,
    /// `D0`–`D15`: a parallel data line.
    Data(u8),
}

/// An output pin connected to an [`Ili9341Model`].
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
