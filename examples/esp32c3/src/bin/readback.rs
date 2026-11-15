//! Writes pixels and reads them back over SPI, together with the controller status.
//!
//! Needs `SDO` wired to MISO, so it runs against the `ili9341-sim` Wokwi chip
//! (`diagram-chip.json`); Wokwi's built-in ILI9341 returns zeros for every read.
//! Wiring: SCK = GPIO6, MOSI = GPIO7, MISO = GPIO2, CS = GPIO10, D/C = GPIO4, RST = GPIO5.

#![no_std]
#![no_main]

use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::main;
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use esp_println::println;
use ili9341_ng::{ColorDepth, Ili9341, SpiInterface};

esp_bootloader_esp_idf::esp_app_desc!();

/// Pixels with distinct 6-bit components.
const PIXELS: [[u8; 3]; 3] = [[63, 0, 1], [1, 2, 3], [32, 33, 34]];

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut delay = Delay::new();

    // Reads need an SPI clock of at most 6.6 MHz.
    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(6))
            .with_mode(Mode::_0),
    )
    .expect("valid SPI config")
    .with_sck(peripherals.GPIO6)
    .with_mosi(peripherals.GPIO7)
    .with_miso(peripherals.GPIO2);
    let cs = Output::new(peripherals.GPIO10, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());
    let spi = ExclusiveDevice::new(spi, cs, Delay::new()).expect("CS pin");

    ili9341_ng::reset(&mut rst, &mut delay).unwrap();
    let mut display = Ili9341::new(SpiInterface::new(spi, dc));
    display.init(&mut delay).unwrap();

    let id = display.read_display_id().unwrap();
    let power_mode = display.read_power_mode().unwrap().into_bits();
    println!("readback id={id:02X?} power_mode={power_mode:02X}");

    display.set_color_depth(ColorDepth::Bits18).unwrap();
    display.draw_pixels_rgb666(10, 20, 3, 1, PIXELS).unwrap();
    let mut pixels = [[0; 3]; 3];
    display
        .read_pixels_rgb666(10, 20, 3, 1, &mut pixels)
        .unwrap();
    if pixels == PIXELS {
        println!("readback ok");
    } else {
        println!("readback mismatch: {pixels:?}");
    }

    loop {
        delay.delay_millis(1_000);
    }
}
