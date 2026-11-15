//! Draws a test pattern on an ILI9341 over SPI.
//!
//! Wiring: SCK = GPIO6, MOSI = GPIO7, CS = GPIO10, D/C = GPIO4, RST = GPIO5.

#![no_std]
#![no_main]

use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::pixelcolor::Rgb565;
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
use esp_println::println;
use ili9341_ng::{Ili9341, Orientation, SpiInterface};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut delay = Delay::new();

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(40))
            .with_mode(Mode::_0),
    )
    .expect("valid SPI config")
    .with_sck(peripherals.GPIO6)
    .with_mosi(peripherals.GPIO7);
    let cs = Output::new(peripherals.GPIO10, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
    let spi = ExclusiveDevice::new(spi, cs, Delay::new()).expect("CS pin");

    ili9341_ng::reset(&mut rst, &mut delay).unwrap();
    let mut display = Ili9341::new(SpiInterface::new(spi, dc));
    display.init(&mut delay).unwrap();
    display.set_orientation(Orientation::Landscape).unwrap();
    display.clear(Rgb565::BLACK).unwrap();

    let (width, height) = display.dimensions();
    let bar = width as u32 / 4;
    for (i, color) in [Rgb565::RED, Rgb565::GREEN, Rgb565::BLUE, Rgb565::WHITE]
        .into_iter()
        .enumerate()
    {
        Rectangle::new(Point::new(i as i32 * bar as i32, 0), Size::new(bar, 40))
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut display)
            .unwrap();
    }
    Circle::with_center(Point::new(width as i32 / 2, height as i32 / 2 + 20), 100)
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::YELLOW, 3))
        .draw(&mut display)
        .unwrap();
    Text::new(
        "ili9341-ng",
        Point::new(110, 135),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE),
    )
    .draw(&mut display)
    .unwrap();

    println!("ILI9341 ok {width}x{height}");
    loop {
        delay.delay_millis(1_000);
    }
}
