//! Async driver on `embedded-hal-async`.
//!
//! Mirrors the blocking [`Ili9341`](crate::Ili9341), [`Interface`](crate::Interface) and
//! [`reset`](crate::reset). [`SpiInterface`](crate::SpiInterface) uses the async
//! [`SpiDevice`](embedded_hal_async::spi::SpiDevice);
//! [`ParallelInterface`](crate::ParallelInterface) drives its pins synchronously.

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
