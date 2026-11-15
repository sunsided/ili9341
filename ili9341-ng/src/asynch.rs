//! Async driver on `embedded-hal-async`.
//!
//! Mirrors the blocking [`Ili9341`](crate::Ili9341), [`Interface`](crate::Interface) and
//! [`reset`](crate::reset). [`SpiInterface`](crate::SpiInterface) uses the async
//! [`SpiDevice`](embedded_hal_async::spi::SpiDevice);
//! [`ParallelInterface`](crate::ParallelInterface) drives its pins synchronously.
//!
//! ## Drawing with `embedded-graphics`
//!
//! Its `DrawTarget` is blocking, so the async driver has none. Draw into a big-endian
//! `Framebuffer` instead and flush it with [`Ili9341::draw_raw`]; over a DMA-capable SPI
//! device, the flush can send the buffer without a copy. A full screen takes 150 KiB in
//! 16-bit color, so this draws a band of rows at a time:
//!
//! ```
//! use embedded_graphics::framebuffer::{Framebuffer, buffer_size};
//! use embedded_graphics::pixelcolor::Rgb565;
//! use embedded_graphics::pixelcolor::raw::{BigEndian, RawU16};
//! use embedded_graphics::prelude::*;
//! use embedded_graphics::primitives::{Circle, PrimitiveStyle};
//! use ili9341_ng::asynch::{Ili9341, Interface};
//!
//! // A landscape screen, in bands of 40 rows: 25 KiB.
//! const W: usize = 320;
//! const H: usize = 40;
//! type Band = Framebuffer<Rgb565, RawU16, BigEndian, W, H, { buffer_size::<Rgb565>(W, H) }>;
//!
//! async fn draw<DI: Interface>(
//!     display: &mut Ili9341<DI>,
//!     band: &mut Band,
//! ) -> Result<(), ili9341_ng::Error<DI::Error>> {
//!     let scene = Circle::new(Point::new(100, 60), 120)
//!         .into_styled(PrimitiveStyle::with_fill(Rgb565::RED));
//!     for top in (0..240).step_by(H) {
//!         let Ok(()) = band.clear(Rgb565::BLACK);
//!         // Screen coordinates: the band shows rows `top..top + H`.
//!         let Ok(()) = scene.draw(&mut band.translated(Point::new(0, -(top as i32))));
//!         display.draw_raw(0, top as u16, W as u16, H as u16, band.data()).await?;
//!     }
//!     Ok(())
//! }
//! ```

interface!(
    doc = "The bus between the host and the controller, async.",
    spi = embedded_hal_async::spi::SpiDevice,
    async = [async],
    await = [.await],
);

driver!(
    doc = "Async ILI9341 driver.",
    interface = Interface,
    read_interface = ReadInterface,
    delay = embedded_hal_async::delay::DelayNs,
    async = [async],
    await = [.await],
);
