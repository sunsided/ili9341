//! Wokwi custom chip wrapping the `ili9341-sim` behavioral model.
//!
//! Pins: `CS`, `DC`, `SCK`, `MOSI`, `MISO` for 4-wire SPI; `WR`, `DC` and `D0`–`D15` for
//! the 8080-I parallel bus; `RST` (active low). `VCC`, `GND` and `LED` are not used.
//!
//! The `interface` attribute selects the bus, like the `IM[3:0]` straps of the real
//! chip: `0` SPI (default), `1` 8-bit parallel, `2` 16-bit parallel.
//!
//! The display shows [`Ili9341Model::panel_pixel`], refreshed every [`REFRESH_US`] when
//! the model changed. Commands the model ignores because they came too early after a
//! reset or sleep change are reported on the debug console.

use fake_bus::Clock;
use ili9341_sim::{Ili9341Model, InterfaceMode};
use std::cell::RefCell;
use std::ffi::{CString, c_void};
use wokwi_chip_ll::*;

/// How often the display is refreshed, in microseconds.
const REFRESH_US: u32 = 20_000;

struct Chip {
    model: Box<Ili9341Model>,
    cs: PinId,
    dc: PinId,
    data: Vec<PinId>,
    spi: SPIDevId,
    /// The byte exchanged by the SPI transfer in progress; the simulator writes the
    /// received byte into it and shifts out what it held.
    spi_buffer: Box<[u8; 1]>,
    framebuffer: BufferId,
    width: u32,
    height: u32,
    rendered_changes: u64,
    sim_ns: u64,
    ignored_commands: u32,
}

thread_local! {
    static CHIP: RefCell<Option<Chip>> = const { RefCell::new(None) };
}

fn with_chip<R>(f: impl FnOnce(&mut Chip) -> R) -> R {
    CHIP.with(|chip| f(chip.borrow_mut().as_mut().expect("chipInit not called")))
}

fn cstr(s: &str) -> CString {
    CString::new(s).expect("no interior NUL")
}

fn debug(message: &str) {
    let message = cstr(message);
    // SAFETY: `message` is a valid NUL-terminated string for the duration of the call.
    unsafe { debugPrint(message.as_ptr()) };
}

/// Whether a pin reads high.
fn is_high(pin: PinId) -> bool {
    // SAFETY: Pin IDs were obtained from the simulator in `chipInit`.
    unsafe { pinRead(pin) == HIGH }
}

impl Chip {
    /// Advances the model's clock to the simulation time, so its timing rules apply.
    fn sync_time(&mut self) {
        // SAFETY: Reading the simulation time has no preconditions.
        let now = unsafe { getSimNanos() } as u64;
        self.model.advance_ns(now.saturating_sub(self.sim_ns));
        self.sim_ns = now;
    }

    /// Passes a word to the model and reports newly ignored commands.
    fn receive(&mut self, word: u16, dc: bool) {
        self.model.receive(word, dc);
        let ignored = self.model.ignored_commands();
        if ignored != self.ignored_commands {
            self.ignored_commands = ignored;
            debug("ili9341: command ignored, sent too early after a reset or sleep change");
        }
    }

    /// Prepares the next one-byte SPI transfer, shifting out the next byte of a read.
    /// Returns what [`start_spi`] needs, to call outside the chip borrow.
    fn prepare_spi(&mut self) -> (SPIDevId, *const u8) {
        self.spi_buffer[0] = if self.model.spi_reading() {
            self.model.spi_read_byte()
        } else {
            0
        };
        (self.spi, self.spi_buffer.as_ptr())
    }

    fn render(&mut self) {
        let changes = self.model.changes();
        if changes == self.rendered_changes {
            return;
        }
        self.rendered_changes = changes;

        // RGBA, 8 bits per component.
        let mut row = vec![[0u8; 4]; self.width as usize];
        for y in 0..self.height {
            for (x, pixel) in row.iter_mut().enumerate() {
                let [r, g, b] = self.model.panel_pixel(x as u16, y as u16).unwrap_or([0; 3]);
                let expand = |c: u8| c << 2 | c >> 4;
                *pixel = [expand(r), expand(g), expand(b), 0xFF];
            }
            let row = row.as_flattened();
            // SAFETY: The framebuffer ID was obtained in `chipInit`; `row` is valid for
            // the length passed.
            unsafe {
                bufferWrite(
                    self.framebuffer,
                    y * self.width * 4,
                    row.as_ptr(),
                    row.len() as u32,
                );
            }
        }
    }
}

/// Starts an SPI transfer prepared by [`Chip::prepare_spi`].
///
/// Simulator calls can invoke callbacks right away, so they run outside the chip borrow.
fn start_spi((spi, buffer): (SPIDevId, *const u8)) {
    // SAFETY: The buffer is heap-allocated and owned by the chip, which lives for the
    // whole simulation, so it stays valid until the transfer completes.
    unsafe { spiStart(spi, buffer, 1) };
}

extern "C" fn on_cs_change(_: *mut c_void, _pin: PinId, value: u32) {
    if value == LOW {
        start_spi(with_chip(|chip| {
            chip.sync_time();
            chip.prepare_spi()
        }));
    } else {
        let spi = with_chip(|chip| chip.spi);
        // SAFETY: The SPI device ID was obtained in `chipInit`. Called outside the chip
        // borrow, as stopping may complete the transfer in progress.
        unsafe { spiStop(spi) };
        with_chip(|chip| chip.model.spi_end_read());
    }
}

extern "C" fn on_spi_done(_: *mut c_void, buffer: *mut u8, count: u32) {
    let next = with_chip(|chip| {
        chip.sync_time();
        if count == 1 && !chip.model.spi_reading() {
            // SAFETY: The simulator passes back the one-byte buffer given to `spiStart`.
            let byte = unsafe { *buffer };
            chip.receive(byte.into(), is_high(chip.dc));
        }
        (!is_high(chip.cs)).then(|| chip.prepare_spi())
    });
    if let Some(next) = next {
        start_spi(next);
    }
}

extern "C" fn on_write_strobe(_: *mut c_void, _pin: PinId, _value: u32) {
    with_chip(|chip| {
        chip.sync_time();
        let word = chip.data.iter().enumerate().fold(0u16, |word, (n, &pin)| {
            word | (u16::from(is_high(pin)) << n)
        });
        chip.receive(word, is_high(chip.dc));
    });
}

extern "C" fn on_reset_change(_: *mut c_void, _pin: PinId, value: u32) {
    with_chip(|chip| {
        chip.sync_time();
        chip.model.set_reset_line(value == HIGH);
    });
}

extern "C" fn on_refresh(_: *mut c_void) {
    with_chip(Chip::render);
}

/// Entry point called by the simulator.
#[unsafe(no_mangle)]
pub extern "C" fn chipInit() {
    // Wasm panics only show as "unreachable"; print the message first.
    std::panic::set_hook(Box::new(|info| debug(&format!("ili9341: {info}"))));
    // SAFETY: All pointers passed to the simulator are valid for the duration of each
    // call; configs are copied by the simulator. Init functions are only called here.
    unsafe {
        let mode = match attrRead(attrInit(cstr("interface").as_ptr(), 0.0)) {
            1 => InterfaceMode::Parallel8,
            2 => InterfaceMode::Parallel16,
            _ => InterfaceMode::Spi,
        };
        let cs = pinInit(cstr("CS").as_ptr(), INPUT_PULLUP);
        let dc = pinInit(cstr("DC").as_ptr(), INPUT);
        let rst = pinInit(cstr("RST").as_ptr(), INPUT_PULLUP);
        let wr = pinInit(cstr("WR").as_ptr(), INPUT_PULLUP);
        let sck = pinInit(cstr("SCK").as_ptr(), INPUT);
        let mosi = pinInit(cstr("MOSI").as_ptr(), INPUT);
        let miso = pinInit(cstr("MISO").as_ptr(), INPUT);
        let width = match mode {
            InterfaceMode::Spi => 0,
            InterfaceMode::Parallel8 => 8,
            InterfaceMode::Parallel16 => 16,
        };
        let data = (0..width)
            .map(|n| pinInit(cstr(&format!("D{n}")).as_ptr(), INPUT))
            .collect();

        let spi = spiInit(&SPIConfig {
            user_data: std::ptr::null(),
            sck,
            mosi,
            miso,
            mode: 0,
            done: on_spi_done,
        });
        let (mut fb_width, mut fb_height) = (0, 0);
        let framebuffer = framebufferInit(&mut fb_width, &mut fb_height);

        let mut model = Box::new(Ili9341Model::new(mode));
        model.set_reset_line(pinRead(rst) == HIGH);
        let chip = Chip {
            model,
            cs,
            dc,
            data,
            spi,
            spi_buffer: Box::new([0]),
            framebuffer,
            width: fb_width,
            height: fb_height,
            // Forces the first render.
            rendered_changes: u64::MAX,
            sim_ns: getSimNanos() as u64,
            ignored_commands: 0,
        };
        CHIP.with(|c| *c.borrow_mut() = Some(chip));

        let watch = |pin, edge, pin_change| {
            pinWatch(
                pin,
                &WatchConfig {
                    user_data: std::ptr::null(),
                    edge,
                    pin_change,
                },
            );
        };
        watch(rst, BOTH, on_reset_change);
        match mode {
            InterfaceMode::Spi => watch(cs, BOTH, on_cs_change),
            _ => watch(wr, RISING, on_write_strobe),
        }

        let refresh = timerInit(&TimerConfig {
            user_data: std::ptr::null(),
            callback: on_refresh,
        });
        timerStart(refresh, REFRESH_US, true);
    }

    debug("ili9341: chip initialized");
}
