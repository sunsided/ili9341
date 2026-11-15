//! Command codes.

/// Command codes, named after the datasheet mnemonics.
///
/// Level 1 commands (`00h`–`5Fh`) follow the MIPI DCS; Level 2 and extended commands
/// (`B0h`–`F7h`) are Ilitek-specific.
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Command {
    /// No operation (00h).
    NOP = 0x00,
    /// Software reset (01h). Wait 5 ms before the next command, and 120 ms before
    /// `SLPOUT` if the controller was in sleep-out mode.
    SWRESET = 0x01,
    /// Read display identification information (04h).
    RDDIDIF = 0x04,
    /// Read display status (09h).
    RDDST = 0x09,
    /// Read display power mode (0Ah).
    RDDPM = 0x0A,
    /// Read display `MADCTL` (0Bh).
    RDDMADCTL = 0x0B,
    /// Read display pixel format (0Ch).
    RDDCOLMOD = 0x0C,
    /// Enter sleep mode (10h). Wait 5 ms before the next command.
    SLPIN = 0x10,
    /// Sleep out (11h). Wait 5 ms before the next command, and 120 ms before `SLPIN`.
    SLPOUT = 0x11,
    /// Partial mode on (12h).
    PTLON = 0x12,
    /// Normal display mode on (13h).
    NORON = 0x13,
    /// Display inversion off (20h).
    INVOFF = 0x20,
    /// Display inversion on (21h).
    INVON = 0x21,
    /// Gamma set (26h). One parameter: the gamma curve.
    GAMSET = 0x26,
    /// Display off (28h). The frame memory is kept, the panel shows blank.
    DISPOFF = 0x28,
    /// Display on (29h).
    DISPON = 0x29,
    /// Column address set (2Ah). Four parameters, see [`address_range`](crate::address_range).
    CASET = 0x2A,
    /// Page (row) address set (2Bh). Four parameters, see
    /// [`address_range`](crate::address_range).
    PASET = 0x2B,
    /// Memory write (2Ch). Followed by pixel data; resets the write pointer to the start
    /// of the window.
    RAMWR = 0x2C,
    /// Color set (2Dh). Lookup table for 16-bit to 18-bit color expansion.
    RGBSET = 0x2D,
    /// Memory read (2Eh).
    RAMRD = 0x2E,
    /// Partial area (30h).
    PTLAR = 0x30,
    /// Vertical scrolling definition (33h).
    VSCRDEF = 0x33,
    /// Tearing effect line off (34h).
    TEOFF = 0x34,
    /// Tearing effect line on (35h).
    TEON = 0x35,
    /// Memory access control (36h). One parameter, see
    /// [`MemoryAccessControl`](crate::MemoryAccessControl).
    MADCTL = 0x36,
    /// Vertical scrolling start address (37h).
    VSCRSADD = 0x37,
    /// Idle mode off (38h).
    IDMOFF = 0x38,
    /// Idle mode on (39h): 8 colors.
    IDMON = 0x39,
    /// Pixel format set (3Ah). One parameter, see [`PixelFormat`](crate::PixelFormat).
    COLMOD = 0x3A,
    /// Write memory continue (3Ch): continues a `RAMWR` where it stopped.
    WRITE_MEMORY_CONTINUE = 0x3C,
    /// Read memory continue (3Eh).
    READ_MEMORY_CONTINUE = 0x3E,
    /// Set tear scanline (44h).
    SET_TEAR_SCANLINE = 0x44,
    /// Get scanline (45h).
    GET_SCANLINE = 0x45,
    /// Write display brightness (51h).
    WRDISBV = 0x51,
    /// Read display brightness (52h).
    RDDISBV = 0x52,
    /// Write CTRL display (53h).
    WRCTRLD = 0x53,
    /// Read CTRL display (54h).
    RDCTRLD = 0x54,
    /// Write content adaptive brightness control (55h).
    WRCABC = 0x55,
    /// Read content adaptive brightness control (56h).
    RDCABC = 0x56,
    /// Write CABC minimum brightness (5Eh).
    WRCABCMB = 0x5E,
    /// Read CABC minimum brightness (5Fh).
    RDCABCMB = 0x5F,
    /// RGB interface signal control (B0h).
    IFMODE = 0xB0,
    /// Frame rate control, normal mode (B1h).
    FRMCTR1 = 0xB1,
    /// Frame rate control, idle mode (B2h).
    FRMCTR2 = 0xB2,
    /// Frame rate control, partial mode (B3h).
    FRMCTR3 = 0xB3,
    /// Display inversion control (B4h).
    INVTR = 0xB4,
    /// Blanking porch control (B5h).
    PRCTR = 0xB5,
    /// Display function control (B6h).
    DISCTRL = 0xB6,
    /// Entry mode set (B7h).
    ETMOD = 0xB7,
    /// Power control 1 (C0h).
    PWCTRL1 = 0xC0,
    /// Power control 2 (C1h).
    PWCTRL2 = 0xC1,
    /// VCOM control 1 (C5h).
    VMCTRL1 = 0xC5,
    /// VCOM control 2 (C7h).
    VMCTRL2 = 0xC7,
    /// Power control A (CBh).
    PWCTRLA = 0xCB,
    /// Power control B (CFh).
    PWCTRLB = 0xCF,
    /// NV memory write (D0h).
    NVMWR = 0xD0,
    /// NV memory protection key (D1h).
    NVMPKEY = 0xD1,
    /// NV memory status read (D2h).
    RDNVM = 0xD2,
    /// Read ID4 (D3h).
    RDID4 = 0xD3,
    /// Read ID1 (DAh).
    RDID1 = 0xDA,
    /// Read ID2 (DBh).
    RDID2 = 0xDB,
    /// Read ID3 (DCh).
    RDID3 = 0xDC,
    /// Positive gamma correction (E0h).
    PGAMCTRL = 0xE0,
    /// Negative gamma correction (E1h).
    NGAMCTRL = 0xE1,
    /// Digital gamma control 1 (E2h).
    DGAMCTRL1 = 0xE2,
    /// Digital gamma control 2 (E3h).
    DGAMCTRL2 = 0xE3,
    /// Driver timing control A (E8h).
    DTCTRLA = 0xE8,
    /// Driver timing control B (EAh).
    DTCTRLB = 0xEA,
    /// Power on sequence control (EDh).
    PWRONSEQ = 0xED,
    /// Enable 3 gamma (F2h).
    ENABLE_3G = 0xF2,
    /// Interface control (F6h).
    IFCTL = 0xF6,
    /// Pump ratio control (F7h).
    PUMPRATIO = 0xF7,
}

impl Command {
    /// The command code.
    pub const fn code(self) -> u8 {
        self as u8
    }
}

impl From<Command> for u8 {
    fn from(command: Command) -> Self {
        command.code()
    }
}
