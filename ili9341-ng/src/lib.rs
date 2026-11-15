//! # Ilitek ILI9341 TFT LCD controller driver
//!
//! > A platform-agnostic driver for the Ilitek ILI9341 240×320 TFT LCD controller.
//!
//! This driver was built using [`embedded-hal`] 1.0 and [`ili9341-registers`]. Enable the
//! `async` feature for an equivalent driver on [`embedded-hal-async`] in the `asynch`
//! module, and the `embedded-graphics` feature for a [`DrawTarget`] implementation on the
//! blocking driver: `Rgb565` on the driver, `Rgb666` through `Ili9341::rgb666`.
//!
//! ## Interfaces
//!
//! The driver talks to the controller through an [`Interface`]:
//!
//! - [`SpiInterface`]: 4-wire SPI ([`SpiDevice`] plus the `D/CX` pin).
//! - [`ParallelInterface`]: the 8080-I 8-bit or 16-bit parallel bus, driven through an
//!   [`OutputBus`]. [`GpioBus`] implements it on individual pins; implement [`OutputBus`]
//!   yourself for a faster port write.
//!
//! Reading works over SPI only ([`ReadInterface`]), and needs `SDO` connected, which
//! many modules leave open.
//!
//! ## Pixels
//!
//! The controller runs in 16-bit (RGB565) or 18-bit (RGB666) color, see [`ColorDepth`].
//! Either way, the driver takes RGB565 pixels as `u16` and RGB666 pixels as `[r, g, b]`
//! with 6-bit components, and converts them to the configured depth.
//!
//! ## Scrolling and partial mode
//!
//! Both work on frame memory lines: the 320 lines along the long side of the panel,
//! independent of the [`Orientation`]. In landscape, they scroll or limit columns.
//!
//! [`embedded-hal`]: https://crates.io/crates/embedded-hal
//! [`embedded-hal-async`]: https://crates.io/crates/embedded-hal-async
//! [`ili9341-registers`]: https://crates.io/crates/ili9341-registers
//! [`SpiDevice`]: embedded_hal::spi::SpiDevice
//! [`DrawTarget`]: https://docs.rs/embedded-graphics-core/latest/embedded_graphics_core/draw_target/trait.DrawTarget.html

#![deny(missing_docs)]
#![no_std]
#![forbid(unsafe_code)]
// Enables the `doc_cfg` feature when the `docsrs` configuration attribute is defined.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
mod driver;
#[macro_use]
mod interface;

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub mod asynch;
#[cfg(feature = "embedded-graphics")]
mod graphics;

#[cfg(feature = "embedded-graphics")]
pub use graphics::Rgb666Target;
pub use ili9341_registers as registers;
pub use ili9341_registers::{Command, HEIGHT, WIDTH};
pub use interface::{GpioBus, InterfaceError, OutputBus, ParallelInterface, SpiInterface, Word};

/// How long `RESX` is held low during [`reset`], in microseconds.
///
/// The datasheet requires at least 10 µs; shorter pulses may not reset the chip.
pub const RESET_PULSE_US: u32 = 20;

/// How long to wait after a hardware or software reset before sending commands, in
/// milliseconds.
///
/// The datasheet requires 5 ms, or 120 ms if the controller was in sleep-out mode.
pub const RESET_DELAY_MS: u32 = 120;

/// How long to wait after `SLPOUT` before sending `SLPIN` or showing content, in
/// milliseconds.
pub const SLEEP_OUT_DELAY_MS: u32 = 120;

/// How long to wait after `SLPIN`, in milliseconds.
///
/// The datasheet requires 5 ms before the next command, and 120 ms before `SLPOUT`.
/// Waiting the full 120 ms lets a wake follow at any time.
pub const SLEEP_IN_DELAY_MS: u32 = 120;

/// Power, VCOM, frame rate and gamma settings sent by [`Ili9341::init`] after the
/// software reset.
///
/// The values come from the `begin()` sequence of [Adafruit_ILI9341], present since its
/// oldest file history ([`a82528f`], 2014-01-02) without a note on their origin.
/// [TFT_eSPI] copied them; its maintainer wrote: "The init sequence was pulled from the
/// Adafruit ILI9341 library a few years ago. [...] You may find you can delete those lines
/// and it will not make a difference!" ([Arduino forum, 2018][forum]).
///
/// [Adafruit_ILI9341]: https://github.com/adafruit/Adafruit_ILI9341
/// [`a82528f`]: https://github.com/adafruit/Adafruit_ILI9341/blob/a82528fb6a5c6ba39b3bc411123df68ab5cdce0d/Adafruit_ILI9341.cpp
/// [TFT_eSPI]: https://github.com/Bodmer/TFT_eSPI/blob/master/TFT_Drivers/ILI9341_Init.h
/// [forum]: https://forum.arduino.cc/t/bodmer-ili9341-master-library-vs-datasheet/518777
const PANEL_INIT: &[(u8, &[u8])] = &[
    // Undocumented: not in the datasheet, effect unknown. An Adafruit forum thread
    // reportedly attributes it to Ilitek and the panel maker (unverified, page not
    // readable): https://forums.adafruit.com/viewtopic.php?f=47&t=63229
    (0xEF, &[0x03, 0x80, 0x02]),
    (Command::PWCTRLB.code(), &[0x00, 0xC1, 0x30]),
    (Command::PWRONSEQ.code(), &[0x64, 0x03, 0x12, 0x81]),
    (Command::DTCTRLA.code(), &[0x85, 0x00, 0x78]),
    (Command::PWCTRLA.code(), &[0x39, 0x2C, 0x00, 0x34, 0x02]),
    (Command::PUMPRATIO.code(), &[0x20]),
    (Command::DTCTRLB.code(), &[0x00, 0x00]),
    // GVDD = 4.60 V.
    (Command::PWCTRL1.code(), &[0x23]),
    (Command::PWCTRL2.code(), &[0x10]),
    // VCOMH = 4.250 V, VCOML = -1.500 V.
    (Command::VMCTRL1.code(), &[0x3E, 0x28]),
    (Command::VMCTRL2.code(), &[0x86]),
    // 79 Hz frame rate.
    (Command::FRMCTR1.code(), &[0x00, 0x18]),
    (Command::DISCTRL.code(), &[0x08, 0x82, 0x27]),
    // 3-gamma off, gamma curve 1.
    (Command::ENABLE_3G.code(), &[0x00]),
    (Command::GAMSET.code(), &[0x01]),
    (
        Command::PGAMCTRL.code(),
        &[
            0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09,
            0x00,
        ],
    ),
    (
        Command::NGAMCTRL.code(),
        &[
            0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36,
            0x0F,
        ],
    ),
];

/// The color depth of the controller's pixel interface (`COLMOD`).
///
/// The frame memory always holds 18 bits per pixel; in 16-bit mode, the controller
/// expands red and blue from 5 to 6 bits.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ColorDepth {
    /// RGB565: two bytes per pixel.
    #[default]
    Bits16,
    /// RGB666: three bytes per pixel.
    Bits18,
}

impl ColorDepth {
    /// The `COLMOD` parameter.
    pub const fn pixel_format(self) -> registers::PixelFormat {
        let bpp = match self {
            Self::Bits16 => registers::PixelFormat::BPP_16,
            Self::Bits18 => registers::PixelFormat::BPP_18,
        };
        registers::PixelFormat::new().with_dpi(bpp).with_dbi(bpp)
    }
}

/// Converts an RGB565 pixel into 6-bit `[r, g, b]` components, copying the most
/// significant bit of red and blue into their new least significant bit.
pub const fn rgb565_to_rgb666(pixel: u16) -> [u8; 3] {
    let r = (pixel >> 11) as u8;
    let g = (pixel >> 5) as u8 & 0x3F;
    let b = pixel as u8 & 0x1F;
    [r << 1 | r >> 4, g, b << 1 | b >> 4]
}

/// Converts 6-bit `[r, g, b]` components into an RGB565 pixel, dropping the least
/// significant bit of red and blue.
pub const fn rgb666_to_rgb565([r, g, b]: [u8; 3]) -> u16 {
    ((r as u16 & 0x3E) << 10) | ((g as u16 & 0x3F) << 5) | ((b as u16 & 0x3E) >> 1)
}

/// A pixel the driver can send in either color depth.
trait PixelData: Copy {
    /// The bytes of the pixel in `depth`; the first `len` of the array are used.
    fn encode(self, depth: ColorDepth) -> ([u8; 3], usize);
}

impl PixelData for u16 {
    fn encode(self, depth: ColorDepth) -> ([u8; 3], usize) {
        match depth {
            ColorDepth::Bits16 => {
                let [high, low] = self.to_be_bytes();
                ([high, low, 0], 2)
            }
            ColorDepth::Bits18 => rgb565_to_rgb666(self).encode(depth),
        }
    }
}

impl PixelData for [u8; 3] {
    fn encode(self, depth: ColorDepth) -> ([u8; 3], usize) {
        // 8-bit components would wrap silently, e.g. 128 into black.
        debug_assert!(self.iter().all(|&c| c < 64), "RGB666 components are 0..=63");
        match depth {
            ColorDepth::Bits16 => rgb666_to_rgb565(self).encode(depth),
            // Components sit in the upper six bits of each byte.
            ColorDepth::Bits18 => (self.map(|c| c << 2), 3),
        }
    }
}

/// The pixel data bytes of `pixels` in `depth`.
fn pixel_bytes<P: PixelData>(
    pixels: impl IntoIterator<Item = P>,
    depth: ColorDepth,
) -> impl Iterator<Item = u8> {
    pixels.into_iter().flat_map(move |pixel| {
        let (bytes, len) = pixel.encode(depth);
        bytes.into_iter().take(len)
    })
}

/// A driver error.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<E> {
    /// The interface failed.
    Interface(E),
    /// The area, line or scroll region does not lie within the screen.
    OutOfBounds,
    /// The buffer length does not match the area.
    BufferLength,
}

/// The display orientation, as a rotation of the portrait panel.
///
/// The `MADCTL` values follow the common ILI9341 module layout, whose glass mirrors the
/// columns of the frame memory: [`Portrait`](Self::Portrait) sets `MX`.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Orientation {
    /// 240×320, connector at the bottom.
    #[default]
    Portrait,
    /// 320×240, rotated 90° clockwise.
    Landscape,
    /// 240×320, rotated 180°.
    PortraitFlipped,
    /// 320×240, rotated 270° clockwise.
    LandscapeFlipped,
}

impl Orientation {
    /// Whether rows and columns are exchanged (`MV`).
    pub const fn is_landscape(self) -> bool {
        matches!(self, Self::Landscape | Self::LandscapeFlipped)
    }

    /// The width and height in this orientation.
    pub const fn size(self) -> (u16, u16) {
        if self.is_landscape() {
            (HEIGHT, WIDTH)
        } else {
            (WIDTH, HEIGHT)
        }
    }

    /// Applies the orientation to `madctl`, keeping its other bits.
    pub const fn apply(
        self,
        madctl: registers::MemoryAccessControl,
    ) -> registers::MemoryAccessControl {
        let (my, mx, mv) = match self {
            Self::Portrait => (false, true, false),
            Self::Landscape => (false, false, true),
            Self::PortraitFlipped => (true, false, false),
            Self::LandscapeFlipped => (true, true, true),
        };
        madctl
            .with_row_address_order(my)
            .with_column_address_order(mx)
            .with_row_column_exchange(mv)
    }
}

/// The color filter order of the panel.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ColorOrder {
    /// Red-green-blue.
    Rgb,
    /// Blue-green-red. Most ILI9341 modules use this order.
    #[default]
    Bgr,
}

interface!(
    doc = "The bus between the host and the controller.",
    spi = embedded_hal::spi::SpiDevice,
    async = [],
    await = [],
);

driver!(
    doc = "ILI9341 driver.",
    interface = Interface,
    read_interface = ReadInterface,
    delay = embedded_hal::delay::DelayNs,
    async = [],
    await = [],
);
