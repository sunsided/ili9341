# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
