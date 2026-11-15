//! Parameter layouts.

use bitfield_struct::bitfield;

/// Encodes the four parameters of [`CASET`](crate::Command::CASET) and
/// [`PASET`](crate::Command::PASET): the inclusive start and end address, big-endian.
pub const fn address_range(start: u16, end: u16) -> [u8; 4] {
    let [sh, sl] = start.to_be_bytes();
    let [eh, el] = end.to_be_bytes();
    [sh, sl, eh, el]
}

/// Parameter of [`MADCTL`](crate::Command::MADCTL) (36h): how frame memory addresses map
/// to the panel. Default after reset: `0x00`.
#[bitfield(u8, order = Msb, defmt = cfg(feature = "defmt"))]
#[derive(PartialEq, Eq)]
pub struct MemoryAccessControl {
    /// `MY`: row address order; mirrors rows.
    pub row_address_order: bool,
    /// `MX`: column address order; mirrors columns.
    pub column_address_order: bool,
    /// `MV`: row/column exchange; swaps rows and columns.
    pub row_column_exchange: bool,
    /// `ML`: vertical refresh order (LCD scan direction only, not the memory mapping).
    pub vertical_refresh_order: bool,
    /// `BGR`: the panel's color filter order is BGR rather than RGB.
    pub bgr: bool,
    /// `MH`: horizontal refresh order (LCD scan direction only).
    pub horizontal_refresh_order: bool,
    #[bits(2)]
    __: u8,
}

/// Parameter of [`COLMOD`](crate::Command::COLMOD) (3Ah): pixel formats of the RGB
/// interface (`DPI`) and the MCU interface (`DBI`). Default after reset: `0x66` (18 bit).
#[bitfield(u8, order = Msb, defmt = cfg(feature = "defmt"))]
#[derive(PartialEq, Eq)]
pub struct PixelFormat {
    #[bits(1)]
    __: u8,
    /// `DPI`: RGB interface format; one of the `BPP_*` values.
    #[bits(3)]
    pub dpi: u8,
    #[bits(1)]
    __: u8,
    /// `DBI`: MCU (SPI and parallel) interface format; one of the `BPP_*` values.
    #[bits(3)]
    pub dbi: u8,
}

impl PixelFormat {
    /// 16 bits per pixel (RGB565).
    pub const BPP_16: u8 = 0b101;
    /// 18 bits per pixel (RGB666).
    pub const BPP_18: u8 = 0b110;

    /// 16 bits per pixel on both interfaces: `0x55`.
    pub const fn rgb565() -> Self {
        Self::new().with_dpi(Self::BPP_16).with_dbi(Self::BPP_16)
    }
}

/// Response of [`RDDPM`](crate::Command::RDDPM) (0Ah): the display power mode. Default
/// after reset: `0x08`.
#[bitfield(u8, order = Msb, defmt = cfg(feature = "defmt"))]
#[derive(PartialEq, Eq)]
pub struct PowerMode {
    /// `D7`: the booster is on and working.
    pub booster_on: bool,
    /// `D6`: idle mode (8 colors) is on.
    pub idle_mode: bool,
    /// `D5`: partial mode is on.
    pub partial_mode: bool,
    /// `D4`: the controller is out of sleep mode.
    pub sleep_out: bool,
    /// `D3`: normal display mode is on.
    pub normal_mode: bool,
    /// `D2`: the display output is on.
    pub display_on: bool,
    #[bits(2)]
    __: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn madctl_bits() {
        let madctl = MemoryAccessControl::new()
            .with_column_address_order(true)
            .with_bgr(true);
        assert_eq!(madctl.into_bits(), 0x48);
        assert!(MemoryAccessControl::from_bits(0x80).row_address_order());
        assert!(MemoryAccessControl::from_bits(0x20).row_column_exchange());
    }

    #[test]
    fn pixel_format_bits() {
        assert_eq!(PixelFormat::rgb565().into_bits(), 0x55);
        assert_eq!(PixelFormat::from_bits(0x66).dbi(), PixelFormat::BPP_18);
    }

    #[test]
    fn power_mode_bits() {
        let mode = PowerMode::from_bits(0x9C);
        assert!(mode.booster_on() && mode.sleep_out() && mode.normal_mode() && mode.display_on());
        assert!(!mode.idle_mode() && !mode.partial_mode());
    }

    #[test]
    fn address_range_is_big_endian() {
        assert_eq!(address_range(0x0012, 0x013F), [0x00, 0x12, 0x01, 0x3F]);
    }
}
