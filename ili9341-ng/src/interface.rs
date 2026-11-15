//! The SPI and parallel interfaces, written once for both the blocking and the async API.

use embedded_hal::digital::{OutputPin, PinState};

/// How many bytes [`SpiInterface`] buffers per SPI write when streaming pixel data from an
/// iterator or repeating one color.
///
/// Each chunk is one SPI transaction; a full screen in 16-bit color takes 600. Slices
/// ([`Interface::send_pixel_slice`](crate::Interface::send_pixel_slice)) go out in one.
pub(crate) const SPI_CHUNK: usize = 256;

/// An interface error.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum InterfaceError<B, P> {
    /// The SPI or parallel data bus failed.
    Bus(B),
    /// A control pin (`D/CX`, `WRX`, `RDX`) failed.
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

/// The data lines of a parallel bus that can also read: the host switches them to inputs
/// while the controller drives them.
///
/// Implement it on the port registers of your host; [`GpioBus`] cannot switch the
/// direction of an [`OutputPin`].
pub trait ReadBus: OutputBus {
    /// Switches the data lines to inputs.
    fn set_input(&mut self) -> Result<(), Self::Error>;

    /// Switches the data lines back to outputs.
    fn set_output(&mut self) -> Result<(), Self::Error>;

    /// Samples the data lines; bit `n` comes from `Dn`.
    ///
    /// Called with `RDX` low. The controller needs up to 340 ns after the falling edge of
    /// `RDX` to drive frame memory data (`tRDLFM`), and 40 ns for other reads (`tRDLID`);
    /// wait that long here if the host is faster.
    fn read_value(&mut self) -> Result<Self::Word, Self::Error>;
}

/// The 8080-I parallel bus: data lines, `D/CX`, `WRX` and optionally `RDX`. `CSX` must be
/// held low, and `RDX` high when it is not connected through
/// [`with_read`](Self::with_read).
///
/// The bus width follows the [`OutputBus`]. With a 16-bit bus, commands and parameters
/// use `D[7:0]`.
///
/// With `RDX` and a [`ReadBus`], it implements [`ReadInterface`](crate::ReadInterface).
/// Every parallel read starts with a dummy word. Frame memory reads on the 16-bit bus are
/// assumed to pack the 8-bit pixel byte stream two bytes per word, first byte on
/// `D[15:8]`, as pixel writes do.
#[derive(Debug)]
pub struct ParallelInterface<BUS, DC, WR, RD = ()> {
    bus: BUS,
    dc: DC,
    wr: WR,
    rd: RD,
}

impl<BUS, DC, WR> ParallelInterface<BUS, DC, WR> {
    /// Creates a write-only interface. Performs no pin access.
    pub fn new(bus: BUS, dc: DC, wr: WR) -> Self {
        Self {
            bus,
            dc,
            wr,
            rd: (),
        }
    }

    /// Adds the `RDX` pin, for reading through a [`ReadBus`]. Performs no pin access;
    /// `RDX` must be high until the first read.
    pub fn with_read<RD>(self, rd: RD) -> ParallelInterface<BUS, DC, WR, RD> {
        let Self {
            bus,
            dc,
            wr,
            rd: (),
        } = self;
        ParallelInterface { bus, dc, wr, rd }
    }

    /// Consumes the interface and returns the data bus, `D/CX` and `WRX`.
    pub fn release(self) -> (BUS, DC, WR) {
        (self.bus, self.dc, self.wr)
    }
}

impl<BUS, DC, WR, RD: OutputPin> ParallelInterface<BUS, DC, WR, RD> {
    /// Consumes the interface and returns the data bus, `D/CX`, `WRX` and `RDX`.
    pub fn release_with_read(self) -> (BUS, DC, WR, RD) {
        (self.bus, self.dc, self.wr, self.rd)
    }
}

impl<BUS, DC, WR, RD> ParallelInterface<BUS, DC, WR, RD>
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

impl<BUS, DC, WR, RD> ParallelInterface<BUS, DC, WR, RD>
where
    BUS: ReadBus<Word: Into<u16>>,
    DC: OutputPin,
    WR: OutputPin<Error = DC::Error>,
    RD: OutputPin<Error = DC::Error>,
{
    /// Sends a read command and reads its response into `buffer`, after the dummy word.
    pub(crate) fn read_response(
        &mut self,
        command: u8,
        buffer: &mut [u8],
    ) -> Result<(), InterfaceError<BUS::Error, DC::Error>> {
        self.command(command, &[])?;
        self.bus.set_input().map_err(InterfaceError::Bus)?;
        let read = self.read_words(command, buffer);
        // Drives the bus again even after a failed read.
        let restored = self.bus.set_output().map_err(InterfaceError::Bus);
        read.and(restored)
    }

    fn read_words(
        &mut self,
        command: u8,
        buffer: &mut [u8],
    ) -> Result<(), InterfaceError<BUS::Error, DC::Error>> {
        self.read_word()?;
        let memory = command == crate::Command::RAMRD.code()
            || command == crate::Command::READ_MEMORY_CONTINUE.code();
        if memory && size_of::<BUS::Word>() == 2 {
            for bytes in buffer.chunks_mut(2) {
                let word = self.read_word()?.to_be_bytes();
                bytes.copy_from_slice(&word[..bytes.len()]);
            }
        } else {
            for byte in buffer {
                // Parameters use `D[7:0]` on the 16-bit bus.
                *byte = self.read_word()? as u8;
            }
        }
        Ok(())
    }

    /// Reads one word; the controller drives the data lines while `RDX` is low.
    fn read_word(&mut self) -> Result<u16, InterfaceError<BUS::Error, DC::Error>> {
        self.rd.set_low().map_err(InterfaceError::Pin)?;
        let word = self.bus.read_value().map_err(InterfaceError::Bus);
        self.rd.set_high().map_err(InterfaceError::Pin)?;
        Ok(word?.into())
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

            /// Sends pixel data from a slice, like [`send_pixel_data`](Self::send_pixel_data).
            ///
            /// [`SpiInterface`](crate::SpiInterface) hands the slice to the SPI device in
            /// one write, which a DMA-capable device can send without copying.
            $($asy)* fn send_pixel_slice(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
                self.send_pixel_data(bytes.iter().copied()) $($aw)*
            }

            /// Sends the bytes of one pixel `count` times, like
            /// [`send_pixel_data`](Self::send_pixel_data).
            $($asy)* fn send_pixel_repeat(&mut self, pixel: &[u8], count: u32) -> Result<(), Self::Error> {
                self.send_pixel_data((0..count).flat_map(|_| pixel.iter().copied())) $($aw)*
            }
        }

        /// An [`Interface`] that can read from the controller.
        #[allow(async_fn_in_trait)]
        pub trait ReadInterface: Interface {
            /// Sends a read command with `D/CX` low and reads its response into `buffer`,
            /// skipping the dummy cycles the interface has for `command`.
            $($asy)* fn read(&mut self, command: u8, buffer: &mut [u8]) -> Result<(), Self::Error>;
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

            $($asy)* fn send_pixel_slice(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
                self.dc.set_high().map_err($crate::InterfaceError::Pin)?;
                self.spi.write(bytes) $($aw)* .map_err($crate::InterfaceError::Bus)
            }

            $($asy)* fn send_pixel_repeat(&mut self, pixel: &[u8], count: u32) -> Result<(), Self::Error> {
                const CHUNK: usize = $crate::interface::SPI_CHUNK;
                if pixel.is_empty() || pixel.len() > CHUNK {
                    return self.send_pixel_data((0..count).flat_map(|_| pixel.iter().copied())) $($aw)*;
                }
                self.dc.set_high().map_err($crate::InterfaceError::Pin)?;

                // Whole pixels per chunk, written once and sent as often as needed.
                let per_chunk = CHUNK / pixel.len();
                let mut buffer = [0; CHUNK];
                buffer
                    .chunks_exact_mut(pixel.len())
                    .for_each(|bytes| bytes.copy_from_slice(pixel));
                let mut remaining = count;
                while remaining > 0 {
                    let pixels = remaining.min(per_chunk as u32);
                    let len = pixels as usize * pixel.len();
                    self.spi.write(&buffer[..len]) $($aw)* .map_err($crate::InterfaceError::Bus)?;
                    remaining -= pixels;
                }
                Ok(())
            }
        }

        impl<SPI, DC> ReadInterface for $crate::SpiInterface<SPI, DC>
        where
            SPI: $spi,
            DC: embedded_hal::digital::OutputPin,
        {
            $($asy)* fn read(&mut self, command: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
                use embedded_hal::spi::Operation;
                use $crate::Command;

                // Memory reads start with a dummy byte, reads of more than one byte with
                // a dummy clock; 8-bit reads have none. The datasheet shows the dummy
                // clock for the 24- and 32-bit reads; `GET_SCANLINE` is assumed to have it.
                let dummy_byte = command == Command::RAMRD.code()
                    || command == Command::READ_MEMORY_CONTINUE.code();
                let dummy_bit = command == Command::RDDIDIF.code()
                    || command == Command::RDDST.code()
                    || command == Command::GET_SCANLINE.code();

                // D/CX cannot change within one SPI transaction; the datasheet's read
                // timing only requires it low for the command byte.
                self.dc.set_low().map_err($crate::InterfaceError::Pin)?;
                let mut dummy = [0];
                let mut last = [0];
                self.spi
                    .transaction(&mut [
                        Operation::Write(&[command]),
                        Operation::Read(&mut dummy[..usize::from(dummy_byte)]),
                        Operation::Read(buffer),
                        Operation::Read(&mut last[..usize::from(dummy_bit)]),
                    ])
                    $($aw)*
                    .map_err($crate::InterfaceError::Bus)?;
                if dummy_bit {
                    // Drops the dummy bit: shifts the response left by one bit.
                    let mut next = last[0];
                    for byte in buffer.iter_mut().rev() {
                        let current = *byte;
                        *byte = current << 1 | next >> 7;
                        next = current;
                    }
                }
                Ok(())
            }
        }

        impl<BUS, DC, WR, RD> Interface for $crate::ParallelInterface<BUS, DC, WR, RD>
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

        impl<BUS, DC, WR, RD> ReadInterface for $crate::ParallelInterface<BUS, DC, WR, RD>
        where
            BUS: $crate::ReadBus<Word: Into<u16>>,
            DC: embedded_hal::digital::OutputPin,
            WR: embedded_hal::digital::OutputPin<Error = DC::Error>,
            RD: embedded_hal::digital::OutputPin<Error = DC::Error>,
        {
            $($asy)* fn read(&mut self, command: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
                self.read_response(command, buffer)
            }
        }
    };
}
