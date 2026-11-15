//! Draws the test pattern of the main example with the async driver: into a framebuffer
//! one band of rows at a time, flushed over SPI with DMA.
//!
//! Wiring: SCK = GPIO6, MOSI = GPIO7, CS = GPIO10, D/C = GPIO4, RST = GPIO5.

#![no_std]
#![no_main]

use embassy_futures::block_on;
use embedded_graphics::framebuffer::{Framebuffer, buffer_size};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::pixelcolor::raw::{BigEndian, RawU16};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::main;
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use esp_hal::{dma_rx_buffer, dma_tx_buffer};
use esp_println::println;
use ili9341_ng::Orientation;
use ili9341_ng::SpiInterface;
use ili9341_ng::asynch::Ili9341;

esp_bootloader_esp_idf::esp_app_desc!();

const WIDTH: usize = 320;
const HEIGHT: usize = 240;
/// Rows per band: 25 KiB of framebuffer in place of 150 KiB for the whole screen.
const BAND: usize = 40;

type Band =
    Framebuffer<Rgb565, RawU16, BigEndian, WIDTH, BAND, { buffer_size::<Rgb565>(WIDTH, BAND) }>;

/// The blocking delay as an async one. Under an executor, use a timer that yields
/// instead, such as `embassy_time::Delay`.
struct BlockingDelay(Delay);

impl embedded_hal_async::delay::DelayNs for BlockingDelay {
    async fn delay_ns(&mut self, ns: u32) {
        self.0.delay_nanos(ns);
    }
}

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    // esp-hal copies slices through these DMA buffers, up to 32000 bytes per transfer.
    let rx = dma_rx_buffer!(4).expect("DMA RX buffer");
    let tx = dma_tx_buffer!(32000).expect("DMA TX buffer");
    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(40))
            .with_mode(Mode::_0),
    )
    .expect("valid SPI config")
    .with_sck(peripherals.GPIO6)
    .with_mosi(peripherals.GPIO7)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(rx, tx)
    .into_async();
    let cs = Output::new(peripherals.GPIO10, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
    let spi = ExclusiveDevice::new_no_delay(spi, cs).expect("CS pin");

    let mut delay = BlockingDelay(Delay::new());
    let mut display = Ili9341::new(SpiInterface::new(spi, dc));
    let mut band = Band::new();
    block_on(async {
        ili9341_ng::asynch::reset(&mut rst, &mut delay)
            .await
            .unwrap();
        display.init(&mut delay).await.unwrap();
        display
            .set_orientation(Orientation::Landscape)
            .await
            .unwrap();

        for top in (0..HEIGHT).step_by(BAND) {
            let Ok(()) = band.clear(Rgb565::BLACK);
            // Draw in screen coordinates; the band holds rows `top..top + BAND`.
            let Ok(()) = draw(&mut band.translated(Point::new(0, -(top as i32))));
            display
                .draw_raw(0, top as u16, WIDTH as u16, BAND as u16, band.data())
                .await
                .unwrap();
        }
    });

    println!("ILI9341 ok {WIDTH}x{HEIGHT}");
    loop {
        Delay::new().delay_millis(1_000);
    }
}

/// The test pattern of the main example: color bars, a circle and text.
fn draw<D: DrawTarget<Color = Rgb565>>(target: &mut D) -> Result<(), D::Error> {
    let bar = WIDTH as u32 / 4;
    for (i, color) in [Rgb565::RED, Rgb565::GREEN, Rgb565::BLUE, Rgb565::WHITE]
        .into_iter()
        .enumerate()
    {
        Rectangle::new(Point::new(i as i32 * bar as i32, 0), Size::new(bar, 40))
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(target)?;
    }
    Circle::with_center(Point::new(WIDTH as i32 / 2, HEIGHT as i32 / 2 + 20), 100)
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::YELLOW, 3))
        .draw(target)?;
    Text::new(
        "ili9341-ng",
        Point::new(110, 135),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE),
    )
    .draw(target)?;
    Ok(())
}
