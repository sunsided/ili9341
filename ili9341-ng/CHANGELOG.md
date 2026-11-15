# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `draw_raw` writes pixel data from a byte slice in the current color depth.
  `SpiInterface` hands the slice to the SPI device in one write, so a DMA-capable device
  can send a framebuffer without a copy. The `asynch` docs show flushing an
  `embedded-graphics` `Framebuffer` with it.
- `Interface::send_pixel_slice` and `Interface::send_pixel_repeat`, provided methods that
  `SpiInterface` overrides. Fills now send one prepared buffer repeatedly instead of
  encoding every pixel.
- Tearing effect output: `set_tearing_effect` with `TearingEffect`, and
  `set_tear_scanline`.
- Idle mode: `set_idle_mode`.
- `init_with` initializes with your own panel settings; `PANEL_INIT` holds the defaults.
- Reading over the parallel bus: `ParallelInterface::with_read` adds `RDX` as a fourth
  type parameter (default `()`), and the new `ReadBus` trait switches the data lines to
  inputs and samples them.
- `ColorDepth::bytes_per_pixel`.
- `wait_for_tear` (blocking, polling) and `asynch::wait_for_tear` (on
  `embedded_hal_async::digital::Wait`) wait for the rising edge of `TE`.
- Reads of the display status (`read_display_status`), image mode (`read_image_mode`),
  signal mode (`read_signal_mode`), scanline (`read_scanline`) and the three ID bytes
  (`read_ids`). Over SPI, `GET_SCANLINE` is assumed to start with a dummy clock like the
  other multi-byte reads.
- `set_frame_rate` sets the division ratio and clocks per line of `FRMCTR1`.

### Changed

- `Error::OutOfBounds` also reports settings out of their range.
- [**breaking**] `ReadInterface::read` drops its `dummy_byte` parameter: the interface
  now handles the dummy cycles of each command, which differ between SPI and the
  parallel bus. `SpiInterface` also drops the dummy bit of 24- and 32-bit reads.

## [0.2.0] - 2026-11-15

[0.2.0]: https://github.com/sunsided/ili9341/releases/tag/v0.2.0

### Added

- 18-bit color: `ColorDepth`, `set_color_depth`, `draw_pixels_rgb666`, and the
  `rgb565_to_rgb666` / `rgb666_to_rgb565` conversions. The 16-bit bus uses the default
  18-bit transfer mode (two bytes of the pixel stream per word).
- Vertical scrolling (`set_scroll_area`, `set_scroll_start`) and partial mode
  (`set_partial_area`, `enter_partial_mode`, `enter_normal_mode`).
- Reading over SPI through the new `ReadInterface`: `read_power_mode`,
  `read_memory_access_control`, `read_pixel_format`, `read_self_diagnostic`,
  `read_display_id` and `read_pixels_rgb666`.
- `Ili9341::rgb666`, an `embedded-graphics` `DrawTarget` with `Rgb666` colors.

### Changed

- [**breaking**] `Interface::send_pixels` (RGB565 pixels) is replaced by
  `Interface::send_pixel_data`, which takes the pixel byte stream.
- [**breaking**] `Word::from_pixel` is replaced by `Word::next_from`, which packs the
  pixel byte stream into bus words.
- [**breaking**] `Error` has the new variant `BufferLength`.

## [0.1.0] - 2026-11-15

[0.1.0]: https://github.com/sunsided/ili9341/releases/tag/v0.1.0

### Added

- Blocking driver on `embedded-hal` 1.0 and async driver on `embedded-hal-async` 1.0
  (`async` feature) over 4-wire SPI and the 8-bit and 16-bit 8080-I parallel bus:
  initialization, hardware reset, orientation, color order, inversion, sleep, display
  on/off, area writes and fills. `init` sends the Adafruit_ILI9341 power, VCOM, frame rate and
  gamma settings.
- `embedded-graphics` `DrawTarget` (`embedded-graphics` feature).
