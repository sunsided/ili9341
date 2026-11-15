# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Blocking driver on `embedded-hal` 1.0 and async driver on `embedded-hal-async` 1.0
  (`async` feature) over 4-wire SPI and the 8-bit and 16-bit 8080-I parallel bus:
  initialization, hardware reset, orientation, color order, inversion, sleep, display
  on/off, area writes and fills. `init` sends the Adafruit_ILI9341 power, VCOM, frame rate and
  gamma settings.
- `embedded-graphics` `DrawTarget` (`embedded-graphics` feature).
