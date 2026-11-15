# Ilitek ILI9341 TFT LCD Controller

[![Build Status](https://github.com/sunsided/ili9341/workflows/Rust/badge.svg)](https://github.com/sunsided/ili9341/actions)
[![codecov](https://codecov.io/gh/sunsided/ili9341/graph/badge.svg)](https://codecov.io/gh/sunsided/ili9341)

> Platform-agnostic Rust support for the Ilitek ILI9341 240×320 TFT LCD controller.

![The ESP32-C3 example on Wokwi's ILI9341: color bars, a circle and text](https://raw.githubusercontent.com/sunsided/ili9341/main/docs/screenshot.png)

*The [ESP32-C3 example](https://github.com/sunsided/ili9341/tree/main/examples/esp32c3) in Wokwi, landscape orientation.*

| Crate                                        | Description                                                        |
|----------------------------------------------|--------------------------------------------------------------------|
| [`ili9341-registers`](ili9341-registers)     | The ILI9341 command set and parameter layouts.                     |
| [`ili9341-ng`](ili9341-ng)                   | An `embedded-hal` 1.0 driver (blocking and async, SPI and parallel). |
| [`ili9341-sim`](ili9341-sim)                 | A behavioral model of the controller for tests.                    |

## Emulation

The behavioral model in `ili9341-sim` decodes the command stream of the SPI and parallel
interfaces into a frame memory, and enforces the datasheet's reset and sleep timing.
Host tests run the driver against it. The example firmware in `examples/` runs against
Wokwi's built-in ILI9341 part using `wokwi-cli`.

## Development

The [Taskfile](Taskfile.dist.yaml) wraps the common commands: `task check` and
`task test` run what CI runs, `task sim` builds the ESP32-C3 firmware and runs the
drawing scenario. `task` lists everything.

## Releasing

Both published crates share one version. From `main`, with the changes listed under
`## [Unreleased]` in each crate's `CHANGELOG.md`:

```sh
cargo release <patch|minor|major>            # dry run
cargo release <patch|minor|major> --execute
```

This commits `Release X.Y.Z` and pushes the tag `vX.Y.Z`. The tag runs the release workflow,
which publishes `ili9341-registers` and `ili9341-ng` to crates.io via
[trusted publishing](https://crates.io/docs/trusted-publishing) (no API token in the
repository) and then creates a GitHub Release named `X.Y.Z` with the changelog sections
as notes.

Each GitHub Release carries the packaged `.crate` files with [build provenance attestations](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations).

## Code of Conduct

We abide by the [Contributor Covenant][cc] and ask that you do as well.

## License

Copyright © 2026 Markus Mayer

Triple licensed under your choice of either of:

- European Union Public Licence, Version 1.2, ([LICENSE-EUPL](LICENSE-EUPL)
  or https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12)
- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

[cc]: https://contributor-covenant.org
