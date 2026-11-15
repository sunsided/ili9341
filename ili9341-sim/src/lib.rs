//! # Ilitek ILI9341 behavioral model
//!
//! A command-level model of the ILI9341 TFT LCD controller for host tests of the
//! `ili9341-ng` driver. It decodes the command stream of the 4-wire SPI interface
//! ([`FakeSpi`] plus a [`SimPin`] as `D/CX`) and of the 8-bit and 16-bit 8080-I parallel
//! interfaces ([`SimPin`]s as data lines, `D/CX` and `WRX`).
//!
//! The model implements the frame memory, the column and page address window, `MADCTL`
//! address mapping, `COLMOD`, sleep, display and inversion state, hardware and software
//! reset, and the command timing rules of the datasheet: commands sent too early after a
//! reset or sleep transition are ignored and counted, as the real chip ignores them.
//!
//! Only 16-bit color is modeled; pixel data in other formats is stored as if 16-bit.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod pins;

pub use pins::{Line, SimPin};

use fake_bus::{Clock, SpiTarget};
use ili9341_registers::{Command, HEIGHT, MemoryAccessControl, WIDTH};

/// An SPI device with an [`Ili9341Model`] attached; pair it with a [`SimPin`] on
/// [`Line::Dc`].
pub type FakeSpi<'a> = fake_bus::FakeSpi<'a, Ili9341Model>;

/// The bus the model listens on, as strapped by the `IM[3:0]` pins.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InterfaceMode {
    /// 4-wire SPI (`IM = 0110`).
    Spi,
    /// 8-bit 8080-I parallel (`IM = 0000`).
    Parallel8,
    /// 16-bit 8080-I parallel (`IM = 0001`).
    Parallel16,
}

/// How long the chip ignores commands after a reset, `SLPIN` or `SLPOUT`, in nanoseconds.
const BUSY_NS: u64 = 5_000_000;

/// The minimum time from a reset, `SLPIN` or `SLPOUT` to the next sleep transition, in
/// nanoseconds.
const SLEEP_TOGGLE_NS: u64 = 120_000_000;

const PIXELS: usize = WIDTH as usize * HEIGHT as usize;

/// The behavioral model of an ILI9341.
#[derive(Debug, Clone)]
pub struct Ili9341Model {
    mode: InterfaceMode,
    gram: [u16; PIXELS],
    now_ns: u64,
    busy_until_ns: u64,
    sleep_toggle_at_ns: u64,
    ignored_commands: u32,

    // Pin state, driven by `SimPin`s.
    dc: bool,
    wr: bool,
    in_reset: bool,
    data: u16,

    // Command decoding.
    command: Option<u8>,
    parameters: [u8; 4],
    parameter_count: usize,
    pixel_high_byte: Option<u8>,

    // Controller state.
    madctl: MemoryAccessControl,
    colmod: u8,
    sleeping: bool,
    display_on: bool,
    inverted: bool,
    columns: (u16, u16),
    pages: (u16, u16),
    pointer: (u16, u16),
}

impl Ili9341Model {
    /// Creates a model listening on the specified bus, in the reset state, with the
    /// supply settled: it accepts commands right away.
    pub fn new(mode: InterfaceMode) -> Self {
        let mut model = Self {
            mode,
            gram: [0; PIXELS],
            now_ns: 0,
            busy_until_ns: 0,
            sleep_toggle_at_ns: 0,
            ignored_commands: 0,
            dc: true,
            wr: true,
            in_reset: false,
            data: 0,
            command: None,
            parameters: [0; 4],
            parameter_count: 0,
            pixel_high_byte: None,
            madctl: MemoryAccessControl::new(),
            colmod: 0,
            sleeping: true,
            display_on: false,
            inverted: false,
            columns: (0, 0),
            pages: (0, 0),
            pointer: (0, 0),
        };
        model.reset();
        (model.busy_until_ns, model.sleep_toggle_at_ns) = (0, 0);
        model
    }

    /// The bus the model listens on.
    pub fn mode(&self) -> InterfaceMode {
        self.mode
    }

    /// The frame memory word at a memory column (`0..240`) and row (`0..320`), unaffected
    /// by `MADCTL`.
    pub fn gram(&self, column: u16, row: u16) -> u16 {
        self.gram[usize::from(row) * usize::from(WIDTH) + usize::from(column)]
    }

    /// The `MADCTL` value.
    pub fn madctl(&self) -> MemoryAccessControl {
        self.madctl
    }

    /// The `COLMOD` value.
    pub fn colmod(&self) -> u8 {
        self.colmod
    }

    /// Whether the controller is in sleep mode.
    pub fn is_sleeping(&self) -> bool {
        self.sleeping
    }

    /// Whether the display output is on.
    pub fn is_display_on(&self) -> bool {
        self.display_on
    }

    /// Whether display inversion is on.
    pub fn is_inverted(&self) -> bool {
        self.inverted
    }

    /// How many commands were ignored because they came too early after a reset or a
    /// sleep transition.
    pub fn ignored_commands(&self) -> u32 {
        self.ignored_commands
    }

    /// Resets the controller to its default state. The frame memory is kept, as its
    /// content is undefined after a reset.
    pub fn reset(&mut self) {
        self.madctl = MemoryAccessControl::new();
        self.colmod = 0x66;
        self.sleeping = true;
        self.display_on = false;
        self.inverted = false;
        self.columns = (0, WIDTH - 1);
        self.pages = (0, HEIGHT - 1);
        self.command = None;
        self.pixel_high_byte = None;
        self.start_timing(true);
    }

    /// Starts the busy period after a reset or sleep transition.
    fn start_timing(&mut self, sleep_toggle: bool) {
        self.busy_until_ns = self.now_ns + BUSY_NS;
        if sleep_toggle {
            self.sleep_toggle_at_ns = self.now_ns + SLEEP_TOGGLE_NS;
        }
    }

    /// Receives one word from the bus: a command byte with `D/CX` low, otherwise a
    /// parameter byte or pixel data.
    ///
    /// SPI and the 8-bit bus deliver bytes; the 16-bit bus delivers words, of which
    /// commands and parameters use the low byte.
    pub fn receive(&mut self, word: u16, dc: bool) {
        if self.in_reset {
            return;
        }
        if dc {
            self.receive_data(word);
        } else {
            self.receive_command(word as u8);
        }
    }

    fn receive_command(&mut self, code: u8) {
        self.command = None;
        self.parameter_count = 0;
        self.pixel_high_byte = None;

        if self.now_ns < self.busy_until_ns {
            self.ignored_commands += 1;
            return;
        }

        let sleep_toggle_ok = self.now_ns >= self.sleep_toggle_at_ns;
        match code {
            c if c == Command::SWRESET.code() => self.reset(),
            c if c == Command::SLPOUT.code() || c == Command::SLPIN.code() => {
                if !sleep_toggle_ok {
                    self.ignored_commands += 1;
                    return;
                }
                self.sleeping = c == Command::SLPIN.code();
                self.start_timing(true);
            }
            c if c == Command::DISPON.code() => self.display_on = true,
            c if c == Command::DISPOFF.code() => self.display_on = false,
            c if c == Command::INVON.code() => self.inverted = true,
            c if c == Command::INVOFF.code() => self.inverted = false,
            c if c == Command::RAMWR.code() => {
                self.pointer = (self.columns.0, self.pages.0);
                self.command = Some(code);
            }
            _ => self.command = Some(code),
        }
    }

    fn receive_data(&mut self, word: u16) {
        let Some(command) = self.command else {
            return;
        };

        if command == Command::RAMWR.code() || command == Command::WRITE_MEMORY_CONTINUE.code() {
            match self.mode {
                InterfaceMode::Parallel16 => self.write_pixel(word),
                InterfaceMode::Spi | InterfaceMode::Parallel8 => {
                    match self.pixel_high_byte.take() {
                        None => self.pixel_high_byte = Some(word as u8),
                        Some(high) => self.write_pixel(u16::from_be_bytes([high, word as u8])),
                    }
                }
            }
            return;
        }

        if self.parameter_count < self.parameters.len() {
            self.parameters[self.parameter_count] = word as u8;
            self.parameter_count += 1;
        }
        let p = self.parameters;
        let range = (
            u16::from_be_bytes([p[0], p[1]]),
            u16::from_be_bytes([p[2], p[3]]),
        );
        match (command, self.parameter_count) {
            (c, 1) if c == Command::MADCTL.code() => {
                self.madctl = MemoryAccessControl::from_bits(p[0])
            }
            (c, 1) if c == Command::COLMOD.code() => self.colmod = p[0],
            (c, 4) if c == Command::CASET.code() => self.columns = range,
            (c, 4) if c == Command::PASET.code() => self.pages = range,
            _ => {}
        }
    }

    /// Stores a pixel at the write pointer and advances it row by row within the window.
    fn write_pixel(&mut self, pixel: u16) {
        let (column, page) = self.pointer;
        if let Some(index) = self.memory_index(column, page) {
            self.gram[index] = pixel;
        }

        self.pointer = if column < self.columns.1 {
            (column + 1, page)
        } else if page < self.pages.1 {
            (self.columns.0, page + 1)
        } else {
            (self.columns.0, self.pages.0)
        };
    }

    /// Maps a column and page address through `MADCTL` to a frame memory index.
    fn memory_index(&self, column: u16, page: u16) -> Option<usize> {
        let (mut x, mut y) = if self.madctl.row_column_exchange() {
            (page, column)
        } else {
            (column, page)
        };
        if x >= WIDTH || y >= HEIGHT {
            return None;
        }
        if self.madctl.column_address_order() {
            x = WIDTH - 1 - x;
        }
        if self.madctl.row_address_order() {
            y = HEIGHT - 1 - y;
        }
        Some(usize::from(y) * usize::from(WIDTH) + usize::from(x))
    }
}

impl SpiTarget for Ili9341Model {
    fn select(&mut self) {}

    fn transfer(&mut self, mosi: u8) -> u8 {
        if self.mode == InterfaceMode::Spi {
            self.receive(mosi.into(), self.dc);
        }
        0
    }
}

impl Clock for Ili9341Model {
    fn advance_ns(&mut self, ns: u64) {
        self.now_ns += ns;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::boxed::Box;

    fn ready(mode: InterfaceMode) -> Box<Ili9341Model> {
        Box::new(Ili9341Model::new(mode))
    }

    #[test]
    fn ignores_commands_while_busy() {
        let mut model = Box::new(Ili9341Model::new(InterfaceMode::Spi));
        model.receive(Command::SWRESET.code().into(), false);
        model.receive(Command::DISPON.code().into(), false);
        assert!(!model.is_display_on());
        assert_eq!(model.ignored_commands(), 1);

        model.advance_ns(BUSY_NS);
        model.receive(Command::SLPOUT.code().into(), false);
        assert!(model.is_sleeping(), "SLPOUT needs 120 ms after reset");
        model.receive(Command::DISPON.code().into(), false);
        assert!(model.is_display_on());
        assert_eq!(model.ignored_commands(), 2);
    }

    #[test]
    fn writes_window_row_by_row_and_wraps() {
        let mut model = ready(InterfaceMode::Parallel16);
        for (command, parameters) in [
            (Command::CASET, [0u8, 1, 0, 2]),
            (Command::PASET, [0, 5, 0, 5]),
        ] {
            model.receive(command.code().into(), false);
            parameters
                .iter()
                .for_each(|&p| model.receive(p.into(), true));
        }
        model.receive(Command::RAMWR.code().into(), false);
        for pixel in [0xA, 0xB, 0xC] {
            model.receive(pixel, true);
        }
        assert_eq!((model.gram(1, 5), model.gram(2, 5)), (0xC, 0xB));
    }

    #[test]
    fn spi_pixels_are_big_endian() {
        let mut model = ready(InterfaceMode::Spi);
        model.receive(Command::RAMWR.code().into(), false);
        model.receive(0x12, true);
        model.receive(0x34, true);
        assert_eq!(model.gram(0, 0), 0x1234);
    }

    #[test]
    fn madctl_mirrors_and_exchanges() {
        let mut model = ready(InterfaceMode::Spi);
        model.receive(Command::MADCTL.code().into(), false);
        model.receive(0xE0, true); // MY | MX | MV
        model.receive(Command::CASET.code().into(), false);
        [0u8, 10, 0, 10]
            .iter()
            .for_each(|&p| model.receive(p.into(), true));
        model.receive(Command::PASET.code().into(), false);
        [0u8, 3, 0, 3]
            .iter()
            .for_each(|&p| model.receive(p.into(), true));
        model.receive(Command::RAMWR.code().into(), false);
        model.receive(0xFF, true);
        model.receive(0xFF, true);
        // Exchange: memory x = page 3, y = column 10; then mirrored.
        assert_eq!(model.gram(WIDTH - 1 - 3, HEIGHT - 1 - 10), 0xFFFF);
    }
}
