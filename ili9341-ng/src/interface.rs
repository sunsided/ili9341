//! The SPI and parallel interfaces, written once for both the blocking and the async API.

use embedded_hal::digital::{OutputPin, PinState};

/// How many bytes [`SpiInterface`] buffers per SPI write when streaming pixel data.
///
/// Each chunk is one SPI transaction; a full screen in 16-bit color takes 600.
pub(crate) const SPI_CHUNK: usize = 256;

/// An interface error.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum InterfaceError<B, P> {
    /// The SPI or parallel data bus failed.
    Bus(B),
    /// A control pin (`D/CX`, `WRX`) failed.
    Pin(P),
}

/// 4-wire SPI: an [`SpiDevice`](embedded_hal::spi::SpiDevice) (with `CSX` as its chip
/// select) and the `D/CX` pin.
///
/// Implements the blocking [`Interface`](crate::Interface) and
/// [`ReadInterface`](crate::ReadInterface) and, with the `async` feature, their `asynch`
/// counterparts when `SPI` implements the respective `SpiDevice`. Reading needs `SDO` (or
/// a bidirectional `SDA`) connected to MISO, and an SPI clock of at most 6.6 MHz: the
/// datasheet's read clock cycle is 150 ns, against 100 ns for writes.
#[derive(Debug)]
pub struct SpiInterface<SPI, DC> {
    pub(crate) spi: SPI,
    pub(crate) dc: DC,
}

impl<SPI, DC> SpiInterface<SPI, DC> {
    /// Creates the interface. Performs no bus access.
    pub fn new(spi: SPI, dc: DC) -> Self {
        Self { spi, dc }
    }

    /// Consumes the interface and returns the SPI device and the `D/CX` pin.
    pub fn release(self) -> (SPI, DC) {
        (self.spi, self.dc)
    }
}

/// A word on the parallel data bus: `u8` for 8-bit, `u16` for 16-bit.
pub trait Word: Copy + From<u8> {
    /// Takes the next bus word of pixel data from a byte stream, or `None` at its end.
    ///
    /// The 16-bit bus carries two bytes per word, first byte on `D[15:8]`; this is the
    /// datasheet's 16-bit pixel format, and its default (`MDT = 00`) 18-bit format. A
    /// trailing odd byte is padded with zeros.
    fn next_from<I: Iterator<Item = u8>>(bytes: &mut I) -> Option<Self>;
}

impl Word for u8 {
    fn next_from<I: Iterator<Item = u8>>(bytes: &mut I) -> Option<Self> {
        bytes.next()
    }
}

impl Word for u16 {
    fn next_from<I: Iterator<Item = u8>>(bytes: &mut I) -> Option<Self> {
        let high = bytes.next()?;
        Some(u16::from_be_bytes([high, bytes.next().unwrap_or(0)]))
    }
}

/// The data lines `D[7:0]` or `D[15:0]` of the parallel bus.
pub trait OutputBus {
    /// `u8` for an 8-bit bus, `u16` for a 16-bit bus.
    type Word: Word;
    /// The error type.
    type Error;

    /// Drives all data lines to `value`; bit `n` goes to `Dn`.
    fn set_value(&mut self, value: Self::Word) -> Result<(), Self::Error>;
}

/// An [`OutputBus`] on individual output pins: `[P; 8]` for an 8-bit bus, `[P; 16]` for
/// a 16-bit bus, with `pins[n]` on `Dn`.
///
/// Sets each pin separately. For speed, implement [`OutputBus`] on a port register.
#[derive(Debug)]
pub struct GpioBus<P, const N: usize> {
    pins: [P; N],
}

impl<P, const N: usize> GpioBus<P, N> {
    /// Creates the bus. Performs no pin access.
    pub fn new(pins: [P; N]) -> Self {
        Self { pins }
    }

    /// Consumes the bus and returns the pins.
    pub fn release(self) -> [P; N] {
        self.pins
    }
}

impl<P: OutputPin, const N: usize> GpioBus<P, N> {
    fn set_bits(&mut self, value: u16) -> Result<(), P::Error> {
        for (n, pin) in self.pins.iter_mut().enumerate() {
            pin.set_state(PinState::from(value >> n & 1 != 0))?;
        }
        Ok(())
    }
}

impl<P: OutputPin> OutputBus for GpioBus<P, 8> {
    type Word = u8;
    type Error = P::Error;

    fn set_value(&mut self, value: u8) -> Result<(), P::Error> {
        self.set_bits(value.into())
    }
}

impl<P: OutputPin> OutputBus for GpioBus<P, 16> {
    type Word = u16;
    type Error = P::Error;

    fn set_value(&mut self, value: u16) -> Result<(), P::Error> {
        self.set_bits(value)
    }
}

/// The 8080-I parallel bus: data lines, `D/CX` and `WRX`. `CSX` must be held low and
/// `RDX` high.
///
/// The bus width follows the [`OutputBus`]. With a 16-bit bus, commands and parameters
/// use `D[7:0]`.
#[derive(Debug)]
pub struct ParallelInterface<BUS, DC, WR> {
    bus: BUS,
    dc: DC,
    wr: WR,
}

impl<BUS, DC, WR> ParallelInterface<BUS, DC, WR> {
    /// Creates the interface. Performs no pin access.
    pub fn new(bus: BUS, dc: DC, wr: WR) -> Self {
        Self { bus, dc, wr }
    }

    /// Consumes the interface and returns the data bus, `D/CX` and `WRX`.
    pub fn release(self) -> (BUS, DC, WR) {
        (self.bus, self.dc, self.wr)
    }
}

impl<BUS, DC, WR> ParallelInterface<BUS, DC, WR>
where
    BUS: OutputBus,
    DC: OutputPin,
    WR: OutputPin<Error = DC::Error>,
{
    /// Clocks one word; the controller latches it on the rising edge of `WRX`.
    fn write_word(&mut self, word: BUS::Word) -> Result<(), InterfaceError<BUS::Error, DC::Error>> {
        self.wr.set_low().map_err(InterfaceError::Pin)?;
        self.bus.set_value(word).map_err(InterfaceError::Bus)?;
        self.wr.set_high().map_err(InterfaceError::Pin)
    }

    pub(crate) fn command(
        &mut self,
        command: u8,
        parameters: &[u8],
    ) -> Result<(), InterfaceError<BUS::Error, DC::Error>> {
        self.dc.set_low().map_err(InterfaceError::Pin)?;
        self.write_word(command.into())?;
        self.dc.set_high().map_err(InterfaceError::Pin)?;
        parameters
            .iter()
            .try_for_each(|&parameter| self.write_word(parameter.into()))
    }

    pub(crate) fn pixel_data<I>(
        &mut self,
        bytes: I,
    ) -> Result<(), InterfaceError<BUS::Error, DC::Error>>
    where
        I: IntoIterator<Item = u8>,
    {
        self.dc.set_high().map_err(InterfaceError::Pin)?;
        let mut bytes = bytes.into_iter();
        while let Some(word) = BUS::Word::next_from(&mut bytes) {
            self.write_word(word)?;
        }
        Ok(())
    }
}

/// Generates the `Interface` and `ReadInterface` traits and their implementations for
/// [`SpiInterface`] and [`ParallelInterface`].
///
/// - `spi`: the `SpiDevice` trait to build on.
/// - `async`: empty, or `async` to make the methods async.
/// - `await`: empty, or `.await` to await SPI calls.
macro_rules! interface {
    (
        doc = $doc:literal,
        spi = $spi:path,
        async = [$($asy:tt)*],
        await = [$($aw:tt)*] $(,)?
    ) => {
        #[doc = $doc]
        ///
        /// A command is the command byte with `D/CX` low, followed by its parameter bytes
        /// with `D/CX` high. Pixel data follows a `RAMWR` command with `D/CX` high.
        #[allow(async_fn_in_trait)]
        pub trait Interface {
            /// The error type.
            type Error;

            /// Sends a command and its parameters.
            $($asy)* fn send_command(&mut self, command: u8, parameters: &[u8]) -> Result<(), Self::Error>;

            /// Sends pixel data as the byte stream of the 8-bit formats: two bytes per
            /// RGB565 pixel, three per RGB666 pixel. A 16-bit bus packs two bytes per word.
            $($asy)* fn send_pixel_data<I>(&mut self, bytes: I) -> Result<(), Self::Error>
            where
                I: IntoIterator<Item = u8>;
        }

        /// An [`Interface`] that can read from the controller.
        #[allow(async_fn_in_trait)]
        pub trait ReadInterface: Interface {
            /// Sends a read command with `D/CX` low and reads its response into `buffer`
            /// in the same transaction, after skipping one dummy byte if `dummy_byte`.
            $($asy)* fn read(&mut self, command: u8, dummy_byte: bool, buffer: &mut [u8]) -> Result<(), Self::Error>;
        }

        impl<SPI, DC> Interface for $crate::SpiInterface<SPI, DC>
        where
            SPI: $spi,
            DC: embedded_hal::digital::OutputPin,
        {
            type Error = $crate::InterfaceError<SPI::Error, DC::Error>;

            $($asy)* fn send_command(&mut self, command: u8, parameters: &[u8]) -> Result<(), Self::Error> {
                self.dc.set_low().map_err($crate::InterfaceError::Pin)?;
                self.spi.write(&[command]) $($aw)* .map_err($crate::InterfaceError::Bus)?;
                if !parameters.is_empty() {
                    self.dc.set_high().map_err($crate::InterfaceError::Pin)?;
                    self.spi.write(parameters) $($aw)* .map_err($crate::InterfaceError::Bus)?;
                }
                Ok(())
            }

            $($asy)* fn send_pixel_data<I>(&mut self, bytes: I) -> Result<(), Self::Error>
            where
                I: IntoIterator<Item = u8>,
            {
                self.dc.set_high().map_err($crate::InterfaceError::Pin)?;

                // Each chunk is its own SPI transaction; the controller keeps writing
                // memory across chip-select cycles until the next command.
                let mut buffer = [0; $crate::interface::SPI_CHUNK];
                let mut len = 0;
                for byte in bytes {
                    buffer[len] = byte;
                    len += 1;
                    if len == buffer.len() {
                        self.spi.write(&buffer) $($aw)* .map_err($crate::InterfaceError::Bus)?;
                        len = 0;
                    }
                }
                if len > 0 {
                    self.spi.write(&buffer[..len]) $($aw)* .map_err($crate::InterfaceError::Bus)?;
                }
                Ok(())
            }
        }

        impl<SPI, DC> ReadInterface for $crate::SpiInterface<SPI, DC>
        where
            SPI: $spi,
            DC: embedded_hal::digital::OutputPin,
        {
            $($asy)* fn read(&mut self, command: u8, dummy_byte: bool, buffer: &mut [u8]) -> Result<(), Self::Error> {
                use embedded_hal::spi::Operation;

                // D/CX cannot change within one SPI transaction; the datasheet's read
                // timing only requires it low for the command byte.
                self.dc.set_low().map_err($crate::InterfaceError::Pin)?;
                let mut dummy = [0];
                let dummy = &mut dummy[..usize::from(dummy_byte)];
                self.spi
                    .transaction(&mut [
                        Operation::Write(&[command]),
                        Operation::Read(dummy),
                        Operation::Read(buffer),
                    ])
                    $($aw)*
                    .map_err($crate::InterfaceError::Bus)
            }
        }

        impl<BUS, DC, WR> Interface for $crate::ParallelInterface<BUS, DC, WR>
        where
            BUS: $crate::OutputBus,
            DC: embedded_hal::digital::OutputPin,
            WR: embedded_hal::digital::OutputPin<Error = DC::Error>,
        {
            type Error = $crate::InterfaceError<BUS::Error, DC::Error>;

            $($asy)* fn send_command(&mut self, command: u8, parameters: &[u8]) -> Result<(), Self::Error> {
                self.command(command, parameters)
            }

            $($asy)* fn send_pixel_data<I>(&mut self, bytes: I) -> Result<(), Self::Error>
            where
                I: IntoIterator<Item = u8>,
            {
                self.pixel_data(bytes)
            }
        }
    };
}
