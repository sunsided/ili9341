//! Writes pixels over the 8-bit 8080-I parallel bus and reads them back with the display
//! ID, through `RDX` and a `ReadBus` on flexible GPIOs.
//!
//! Runs against the `ili9341-sim` Wokwi chip in 8-bit mode (`diagram-parallel.json`).
//! Wiring: D0–D7 = GPIO0, 1, 2, 3, 6, 7, 8, 9; D/C = GPIO4, WR = GPIO5, RD = GPIO10,
//! RST = GPIO18; CS tied low.

#![no_std]
#![no_main]

use core::convert::Infallible;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Flex, InputConfig, Level, Output, OutputConfig};
use esp_hal::main;
use esp_println::println;
use ili9341_ng::{ColorDepth, Ili9341, OutputBus, ParallelInterface, ReadBus};

esp_bootloader_esp_idf::esp_app_desc!();

/// Pixels with distinct 6-bit components.
const PIXELS: [[u8; 3]; 3] = [[63, 0, 1], [1, 2, 3], [32, 33, 34]];

/// The data lines on GPIOs that switch between output and input.
struct FlexBus<'d>([Flex<'d>; 8]);

impl OutputBus for FlexBus<'_> {
    type Word = u8;
    type Error = Infallible;

    fn set_value(&mut self, value: u8) -> Result<(), Infallible> {
        for (n, pin) in self.0.iter_mut().enumerate() {
            pin.set_level(Level::from(value >> n & 1 != 0));
        }
        Ok(())
    }
}

impl ReadBus for FlexBus<'_> {
    fn set_input(&mut self) -> Result<(), Infallible> {
        self.0
            .iter_mut()
            .for_each(|pin| pin.set_output_enable(false));
        Ok(())
    }

    fn set_output(&mut self) -> Result<(), Infallible> {
        self.0
            .iter_mut()
            .for_each(|pin| pin.set_output_enable(true));
        Ok(())
    }

    fn read_value(&mut self) -> Result<u8, Infallible> {
        // 160 MHz is fast enough to need the 340 ns data delay of frame memory reads.
        Delay::new().delay_nanos(340);
        let bits = self.0.iter().enumerate();
        Ok(bits.fold(0, |value, (n, pin)| value | u8::from(pin.is_high()) << n))
    }
}

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut delay = Delay::new();

    let p = peripherals;
    let data = [
        Flex::new(p.GPIO0),
        Flex::new(p.GPIO1),
        Flex::new(p.GPIO2),
        Flex::new(p.GPIO3),
        Flex::new(p.GPIO6),
        Flex::new(p.GPIO7),
        Flex::new(p.GPIO8),
        Flex::new(p.GPIO9),
    ]
    .map(|mut pin| {
        pin.apply_input_config(&InputConfig::default());
        pin.set_input_enable(true);
        pin.apply_output_config(&OutputConfig::default());
        pin.set_output_enable(true);
        pin
    });
    let dc = Output::new(p.GPIO4, Level::High, OutputConfig::default());
    let wr = Output::new(p.GPIO5, Level::High, OutputConfig::default());
    let rd = Output::new(p.GPIO10, Level::High, OutputConfig::default());
    let mut rst = Output::new(p.GPIO18, Level::High, OutputConfig::default());

    ili9341_ng::reset(&mut rst, &mut delay).unwrap();
    let interface = ParallelInterface::new(FlexBus(data), dc, wr).with_read(rd);
    let mut display = Ili9341::new(interface);
    display.init(&mut delay).unwrap();

    let id = display.read_display_id().unwrap();
    let power_mode = display.read_power_mode().unwrap().into_bits();
    println!("parallel id={id:02X?} power_mode={power_mode:02X}");

    display.set_color_depth(ColorDepth::Bits18).unwrap();
    display.draw_pixels_rgb666(10, 20, 3, 1, PIXELS).unwrap();
    let mut pixels = [[0; 3]; 3];
    display
        .read_pixels_rgb666(10, 20, 3, 1, &mut pixels)
        .unwrap();
    if pixels == PIXELS && id == display.read_ids().unwrap() {
        println!("parallel ok");
    } else {
        println!("parallel mismatch: {pixels:?}");
    }

    loop {
        delay.delay_millis(1_000);
    }
}
