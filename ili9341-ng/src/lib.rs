//! # Ilitek ILI9341 TFT LCD controller driver
//!
//! > A platform-agnostic driver for the Ilitek ILI9341 240×320 TFT LCD controller.
//!
//! This driver was built using [`embedded-hal`] 1.0 and [`ili9341-registers`]. Enable the
//! `async` feature for an equivalent driver on [`embedded-hal-async`] in the `asynch`
//! module, and the `embedded-graphics` feature for a [`DrawTarget`] implementation on the
//! blocking driver.
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
//! The controller is write-only in this driver: reading needs the `SDO`/`RDX` lines,
//! which many modules do not connect.
//!
//! ## Pixels
//!
//! The driver configures 16-bit color (RGB565). Pixels are `u16` values in RGB565.
//!
//! [`embedded-hal`]: https://crates.io/crates/embedded-hal
//! [`embedded-hal-async`]: https://crates.io/crates/embedded-hal-async
//! [`ili9341-registers`]: https://crates.io/crates/ili9341-registers
//! [`SpiDevice`]: embedded_hal::spi::SpiDevice
//! [`DrawTarget`]: https://docs.rs/embedded-graphics-core/latest/embedded_graphics_core/draw_target/trait.DrawTarget.html

#![deny(missing_docs)]
#![deny(warnings)]
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

/// A driver error.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<E> {
    /// The interface failed.
    Interface(E),
    /// The area does not lie within the screen.
    OutOfBounds,
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
    delay = embedded_hal::delay::DelayNs,
    async = [],
    await = [],
);
