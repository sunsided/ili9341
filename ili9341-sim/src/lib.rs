//! # Ilitek ILI9341 behavioral model
//!
//! A command-level model of the ILI9341 TFT LCD controller, used in two places:
//!
//! - in host tests of the `ili9341-ng` driver, where it decodes the command stream of the
//!   4-wire SPI interface ([`FakeSpi`] plus a [`SimPin`] as `D/CX`) and of the 8-bit and
//!   16-bit 8080-I parallel interfaces ([`SimPin`]s as data lines, `D/CX` and `WRX`), and
//! - wrapped into a [Wokwi](https://wokwi.com) custom chip (`sim/wokwi-chip`), which
//!   shows [`Ili9341Model::panel_pixel`] on its display.
//!
//! The model implements the 18-bit frame memory, the column and page address window,
//! `MADCTL` address mapping, 16-bit and 18-bit pixel data, sleep, display, inversion,
//! partial and scroll state, hardware and software reset, and the command timing rules
//! of the datasheet: commands sent too early after a reset or sleep transition are
//! ignored and counted, as the real chip ignores them.
//!
//! Over SPI, it answers the read commands `RDDIDIF`, `RDDPM`, `RDDMADCTL`, `RDDCOLMOD`,
//! `RDDSDR` and `RAMRD` with the datasheet's dummy cycles.
//!
//! ## Assumptions
//!
//! - 16-bit pixels are expanded to the 18-bit frame memory by copying the most
//!   significant bit of red and blue into their least significant bit. The real chip
//!   uses its color lookup table (`RGBSET`), whose content the datasheet leaves undefined
//!   after a hardware reset.
//! - The 16-bit bus uses the default 18-bit transfer mode (`MDT = 00`): two bytes of the
//!   8-bit pixel stream per word.
//! - `MADCTL` `ML` (vertical refresh order) is ignored for scrolling and partial mode.
//! - The panel shows frame memory as Wokwi's built-in ILI9341 part does: memory column 0
//!   is the leftmost panel column, memory line 0 the bottom panel line. With the
//!   driver's `MADCTL` values, the four orientations then look the same on both parts.
//! - Areas the panel does not show (display off, sleep, outside the partial area) are
//!   black.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod pins;

pub use pins::{Line, SimPin};

use fake_bus::{Clock, SpiTarget};
use ili9341_registers::{Command, HEIGHT, MemoryAccessControl, PixelFormat, PowerMode, WIDTH};

/// An SPI device with an [`Ili9341Model`] attached; pair it with a [`SimPin`] on
/// [`Line::Dc`].
pub type FakeSpi<'a> = fake_bus::FakeSpi<'a, Ili9341Model>;

/// The display identification the model returns for `RDDIDIF`. Real modules return
/// module-specific values.
pub const MODEL_DISPLAY_ID: [u8; 3] = [0x8A, 0x93, 0x41];

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

/// The most parameters any command takes (`PGAMCTRL`, `NGAMCTRL`).
const MAX_PARAMETERS: usize = 15;

const PIXELS: usize = WIDTH as usize * HEIGHT as usize;

/// A read in progress on the SPI bus; it ends when chip select deasserts.
#[derive(Debug, Copy, Clone)]
enum Read {
    /// A fixed response, sent byte by byte.
    Bytes { data: [u8; 5], len: u8, pos: u8 },
    /// `RAMRD`: one dummy byte, then three bytes per pixel from the write pointer on.
    Memory {
        dummy: bool,
        pixel: [u8; 3],
        pos: u8,
    },
}

/// The behavioral model of an ILI9341.
#[derive(Debug, Clone)]
pub struct Ili9341Model {
    mode: InterfaceMode,
    /// 6-bit `[r, g, b]` per pixel.
    gram: [[u8; 3]; PIXELS],
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
    parameters: [u8; MAX_PARAMETERS],
    parameter_count: usize,
    last_parameters: [([u8; MAX_PARAMETERS], u8); 256],
    pixel_bytes: [u8; 3],
    pixel_byte_count: usize,
    read: Option<Read>,

    // Controller state.
    madctl: MemoryAccessControl,
    colmod: u8,
    sleeping: bool,
    display_on: bool,
    inverted: bool,
    columns: (u16, u16),
    pages: (u16, u16),
    pointer: (u16, u16),
    partial_mode: bool,
    partial_area: (u16, u16),
    scrolling: bool,
    /// Top fixed, scrolling and bottom fixed lines.
    scroll_area: (u16, u16, u16),
    scroll_start: u16,
    changes: u64,
}

impl Ili9341Model {
    /// Creates a model listening on the specified bus, in the reset state, with the
    /// supply settled: it accepts commands right away.
    pub fn new(mode: InterfaceMode) -> Self {
        let mut model = Self {
            mode,
            gram: [[0; 3]; PIXELS],
            now_ns: 0,
            busy_until_ns: 0,
            sleep_toggle_at_ns: 0,
            ignored_commands: 0,
            dc: true,
            wr: true,
            in_reset: false,
            data: 0,
            command: None,
            parameters: [0; MAX_PARAMETERS],
            parameter_count: 0,
            last_parameters: [([0; MAX_PARAMETERS], 0); 256],
            pixel_bytes: [0; 3],
            pixel_byte_count: 0,
            read: None,
            madctl: MemoryAccessControl::new(),
            colmod: 0,
            sleeping: true,
            display_on: false,
            inverted: false,
            columns: (0, 0),
            pages: (0, 0),
            pointer: (0, 0),
            partial_mode: false,
            partial_area: (0, 0),
            scrolling: false,
            scroll_area: (0, 0, 0),
            scroll_start: 0,
            changes: 0,
        };
        model.reset();
        (model.busy_until_ns, model.sleep_toggle_at_ns) = (0, 0);
        model
    }

    /// The frame memory pixel at a memory column (`0..240`) and row (`0..320`) as RGB565,
    /// unaffected by `MADCTL`. Red and blue lose their least significant bit.
    pub fn gram(&self, column: u16, row: u16) -> u16 {
        let [r, g, b] = self.gram_rgb666(column, row);
        (u16::from(r >> 1) << 11) | (u16::from(g) << 5) | u16::from(b >> 1)
    }

    /// The frame memory pixel at a memory column and row as 6-bit `[r, g, b]`.
    pub fn gram_rgb666(&self, column: u16, row: u16) -> [u8; 3] {
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

    /// Whether partial mode is on.
    pub fn is_partial_mode(&self) -> bool {
        self.partial_mode
    }

    /// Whether vertical scroll mode is on.
    pub fn is_scrolling(&self) -> bool {
        self.scrolling
    }

    /// The frame memory line the panel shows at display line `line` (`0..320`), after
    /// scrolling; `None` outside the partial area in partial mode.
    pub fn panel_line(&self, line: u16) -> Option<u16> {
        if line >= HEIGHT {
            return None;
        }
        if self.partial_mode {
            let (start, end) = self.partial_area;
            let inside = if start <= end {
                (start..=end).contains(&line)
            } else {
                line >= start || line <= end
            };
            if !inside {
                return None;
            }
        }
        let (top, scrolling, _) = self.scroll_area;
        if self.scrolling && scrolling > 0 && (top..top + scrolling).contains(&line) {
            let offset = i32::from(self.scroll_start) - i32::from(top) + i32::from(line - top);
            return Some(top + offset.rem_euclid(i32::from(scrolling)) as u16);
        }
        Some(line)
    }

    /// The pixel the panel shows at column `x` (`0..240`) and row `y` (`0..320`, top
    /// down) as 6-bit `[r, g, b]`, after scrolling, partial mode, display on/off, sleep
    /// and inversion. `None` outside the panel.
    pub fn panel_pixel(&self, x: u16, y: u16) -> Option<[u8; 3]> {
        if x >= WIDTH || y >= HEIGHT {
            return None;
        }
        if !self.display_on || self.sleeping {
            return Some([0; 3]);
        }
        // Display line 0 is the bottom panel row.
        let Some(line) = self.panel_line(HEIGHT - 1 - y) else {
            return Some([0; 3]);
        };
        let pixel = self.gram_rgb666(x, line);
        Some(if self.inverted {
            pixel.map(|c| 63 - c)
        } else {
            pixel
        })
    }

    /// A counter that increases with every word received and every reset; unchanged
    /// means the panel shows the same picture.
    pub fn changes(&self) -> u64 {
        self.changes
    }

    /// Drives the active-low `RESX` line. Releasing it resets the controller; while it is
    /// low, the controller ignores the bus.
    pub fn set_reset_line(&mut self, high: bool) {
        let released = high && self.in_reset;
        self.in_reset = !high;
        if released {
            self.reset();
        }
    }

    /// Whether an SPI read is in progress: the controller ignores `SDI` and shifts out
    /// [`spi_read_byte`](Self::spi_read_byte).
    pub fn spi_reading(&self) -> bool {
        self.read.is_some()
    }

    /// Ends an SPI read; chip select deasserting does this.
    pub fn spi_end_read(&mut self) {
        self.read = None;
    }

    /// The parameters last sent with a command, also for commands the model does not
    /// otherwise implement. Pixel data is not recorded.
    pub fn parameters(&self, command: u8) -> &[u8] {
        let (parameters, count) = &self.last_parameters[usize::from(command)];
        &parameters[..usize::from(*count)]
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
        self.partial_mode = false;
        self.partial_area = (0, HEIGHT - 1);
        self.scrolling = false;
        self.scroll_area = (0, HEIGHT, 0);
        self.scroll_start = 0;
        self.command = None;
        self.pixel_byte_count = 0;
        self.read = None;
        self.changes += 1;
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
    /// commands and parameters use the low byte, and pixel data both bytes, high first.
    pub fn receive(&mut self, word: u16, dc: bool) {
        if self.in_reset {
            return;
        }
        self.changes += 1;
        if dc {
            self.receive_data(word);
        } else {
            self.receive_command(word as u8);
        }
    }

    fn receive_command(&mut self, code: u8) {
        self.command = None;
        self.parameter_count = 0;
        self.pixel_byte_count = 0;

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
            c if c == Command::PTLON.code() => (self.partial_mode, self.scrolling) = (true, false),
            c if c == Command::NORON.code() => (self.partial_mode, self.scrolling) = (false, false),
            c if c == Command::RAMWR.code() => {
                self.pointer = (self.columns.0, self.pages.0);
                self.command = Some(code);
            }
            c if c == Command::RAMRD.code() => {
                self.pointer = (self.columns.0, self.pages.0);
                self.read = Some(Read::Memory {
                    dummy: true,
                    pixel: [0; 3],
                    pos: 3,
                });
            }
            c if c == Command::RDDIDIF.code() => {
                // One dummy clock, then 24 bits.
                let [a, b, c] = MODEL_DISPLAY_ID;
                let bits = u32::from_be_bytes([0, a, b, c]) << 7;
                let [d0, d1, d2, d3] = bits.to_be_bytes();
                self.respond(&[d0, d1, d2, d3]);
            }
            c if c == Command::RDDPM.code() => {
                let mode = PowerMode::new()
                    .with_booster_on(!self.sleeping)
                    .with_partial_mode(self.partial_mode)
                    .with_sleep_out(!self.sleeping)
                    .with_normal_mode(!self.partial_mode)
                    .with_display_on(self.display_on);
                self.respond(&[mode.into_bits()]);
            }
            c if c == Command::RDDMADCTL.code() => self.respond(&[self.madctl.into_bits()]),
            c if c == Command::RDDCOLMOD.code() => self.respond(&[self.colmod]),
            c if c == Command::RDDSDR.code() => {
                self.respond(&[if self.sleeping { 0x00 } else { 0xC0 }])
            }
            _ => self.command = Some(code),
        }
    }

    /// Starts sending a fixed response on the next SPI transfers.
    fn respond(&mut self, response: &[u8]) {
        let mut data = [0; 5];
        data[..response.len()].copy_from_slice(response);
        let len = response.len() as u8;
        self.read = Some(Read::Bytes { data, len, pos: 0 });
    }

    /// The next byte of the SPI read in progress, or `0` without one.
    pub fn spi_read_byte(&mut self) -> u8 {
        match self.read {
            Some(Read::Bytes { data, len, pos }) if pos < len => {
                self.read = Some(Read::Bytes {
                    data,
                    len,
                    pos: pos + 1,
                });
                data[usize::from(pos)]
            }
            Some(Read::Memory {
                dummy: true,
                pixel,
                pos,
            }) => {
                self.read = Some(Read::Memory {
                    dummy: false,
                    pixel,
                    pos,
                });
                0
            }
            Some(Read::Memory {
                dummy: false,
                mut pixel,
                mut pos,
            }) => {
                if pos == 3 {
                    let (column, page) = self.pointer;
                    pixel = self
                        .memory_index(column, page)
                        .map_or([0; 3], |index| self.gram[index].map(|c| c << 2));
                    self.advance_pointer();
                    pos = 0;
                }
                self.read = Some(Read::Memory {
                    dummy: false,
                    pixel,
                    pos: pos + 1,
                });
                pixel[usize::from(pos)]
            }
            _ => 0,
        }
    }

    fn receive_data(&mut self, word: u16) {
        let Some(command) = self.command else {
            return;
        };

        if command == Command::RAMWR.code() || command == Command::WRITE_MEMORY_CONTINUE.code() {
            match self.mode {
                InterfaceMode::Parallel16 => {
                    let [high, low] = word.to_be_bytes();
                    self.receive_pixel_byte(high);
                    self.receive_pixel_byte(low);
                }
                InterfaceMode::Spi | InterfaceMode::Parallel8 => {
                    self.receive_pixel_byte(word as u8)
                }
            }
            return;
        }

        if self.parameter_count < self.parameters.len() {
            self.parameters[self.parameter_count] = word as u8;
            self.parameter_count += 1;
            self.last_parameters[usize::from(command)] =
                (self.parameters, self.parameter_count as u8);
        }
        let p = self.parameters;
        let word_at = |i: usize| u16::from_be_bytes([p[i], p[i + 1]]);
        match (command, self.parameter_count) {
            (c, 1) if c == Command::MADCTL.code() => {
                self.madctl = MemoryAccessControl::from_bits(p[0])
            }
            (c, 1) if c == Command::COLMOD.code() => self.colmod = p[0],
            (c, 4) if c == Command::CASET.code() => self.columns = (word_at(0), word_at(2)),
            (c, 4) if c == Command::PASET.code() => self.pages = (word_at(0), word_at(2)),
            (c, 4) if c == Command::PTLAR.code() => self.partial_area = (word_at(0), word_at(2)),
            (c, 6) if c == Command::VSCRDEF.code() => {
                self.scroll_area = (word_at(0), word_at(2), word_at(4))
            }
            (c, 2) if c == Command::VSCRSADD.code() && !self.partial_mode => {
                (self.scroll_start, self.scrolling) = (word_at(0), true)
            }
            _ => {}
        }
    }

    /// Collects the bytes of one pixel: two in 16-bit color, three in 18-bit color.
    fn receive_pixel_byte(&mut self, byte: u8) {
        self.pixel_bytes[self.pixel_byte_count] = byte;
        self.pixel_byte_count += 1;
        let sixteen_bit = PixelFormat::from_bits(self.colmod).dbi() == PixelFormat::BPP_16;
        let [b0, b1, b2] = self.pixel_bytes;
        let pixel = match (sixteen_bit, self.pixel_byte_count) {
            (true, 2) => {
                let pixel = u16::from_be_bytes([b0, b1]);
                let (r, g, b) = (
                    (pixel >> 11) as u8,
                    (pixel >> 5) as u8 & 0x3F,
                    pixel as u8 & 0x1F,
                );
                [r << 1 | r >> 4, g, b << 1 | b >> 4]
            }
            (false, 3) => [b0 >> 2, b1 >> 2, b2 >> 2],
            _ => return,
        };
        self.pixel_byte_count = 0;
        self.write_pixel(pixel);
    }

    /// Stores a pixel at the write pointer and advances the pointer.
    fn write_pixel(&mut self, pixel: [u8; 3]) {
        let (column, page) = self.pointer;
        if let Some(index) = self.memory_index(column, page) {
            self.gram[index] = pixel;
        }
        self.advance_pointer();
    }

    /// Advances the pointer row by row within the window, wrapping at its end.
    fn advance_pointer(&mut self) {
        let (column, page) = self.pointer;
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

    fn deselect(&mut self) {
        self.spi_end_read();
    }

    fn transfer(&mut self, mosi: u8) -> u8 {
        if self.mode != InterfaceMode::Spi {
            return 0;
        }
        // The chip ignores SDI while it shifts out a response.
        if self.spi_reading() {
            return self.spi_read_byte();
        }
        self.receive(mosi.into(), self.dc);
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

    /// A model in 16-bit color.
    fn ready(mode: InterfaceMode) -> Box<Ili9341Model> {
        let mut model = Box::new(Ili9341Model::new(mode));
        model.receive(Command::COLMOD.code().into(), false);
        model.receive(0x55, true);
        model
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
    fn panel_shows_memory_bottom_up_and_display_state() {
        let mut model = ready(InterfaceMode::Spi);
        model.receive(Command::RAMWR.code().into(), false);
        model.receive(0xF8, true); // Red at memory column 0.
        model.receive(0x00, true);
        let bottom_left = (0, HEIGHT - 1);
        assert_eq!(
            model.panel_pixel(bottom_left.0, bottom_left.1),
            Some([0; 3]),
            "display off"
        );

        model.receive(Command::DISPON.code().into(), false);
        model.receive(Command::SLPOUT.code().into(), false);
        assert_eq!(
            model.panel_pixel(bottom_left.0, bottom_left.1),
            Some([63, 0, 0])
        );
        assert_eq!(model.panel_pixel(0, 0), Some([0; 3]));
        model.advance_ns(BUSY_NS);
        model.receive(Command::INVON.code().into(), false);
        assert_eq!(
            model.panel_pixel(bottom_left.0, bottom_left.1),
            Some([0, 63, 63])
        );
        assert_eq!(model.panel_pixel(WIDTH, 0), None);
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
