# Ilitek ILI9341 TFT LCD Controller Commands

[![Crates.io][crates-image]][crates-link]
[![Docs][docs-image]][docs-link]
[![Build Status][build-image]][build-link]
[![Safety Dance][safety-image]][safety-link]
![MSRV][msrv-image]
[![EUPL 1.2 licensed][license-eupl-image]][license-eupl-link]
[![Apache 2.0 licensed][license-apache-image]][license-apache-link]
[![MIT licensed][license-mit-image]][license-mit-link]

> A typed map of the ILI9341's command set.

The ILI9341 has no addressable registers: the host sends a command byte with `D/CX` low,
followed by its parameters with `D/CX` high. This crate provides the command codes
(`Command`), the bit layouts of flag parameters (`MemoryAccessControl` for `MADCTL`,
`PixelFormat` for `COLMOD`) and the `CASET`/`PASET` encoding (`address_range`).

Codes and layouts follow the ILI9341 datasheet (V1.11).

## License

Copyright © 2026 Markus Mayer

Triple licensed under your choice of either of:

- European Union Public Licence, Version 1.2, ([LICENSE-EUPL](LICENSE-EUPL)
  or https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12)
- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

[crates-image]: https://img.shields.io/crates/v/ili9341-registers
[crates-link]: https://crates.io/crates/ili9341-registers
[docs-image]: https://docs.rs/ili9341-registers/badge.svg
[docs-link]: https://docs.rs/ili9341-registers/
[build-image]: https://github.com/sunsided/ili9341/workflows/Rust/badge.svg
[build-link]: https://github.com/sunsided/ili9341/actions
[safety-image]: https://img.shields.io/badge/unsafe-forbidden-success.svg
[safety-link]: https://github.com/rust-secure-code/safety-dance/
[msrv-image]: https://img.shields.io/badge/rustc-1.85+-blue.svg
[license-eupl-image]: https://img.shields.io/badge/license-EUPL_1.2-blue.svg
[license-eupl-link]: https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12
[license-apache-image]: https://img.shields.io/badge/license-Apache_2.0-blue.svg
[license-apache-link]: https://www.apache.org/licenses/LICENSE-2.0
[license-mit-image]: https://img.shields.io/badge/license-MIT-blue.svg
[license-mit-link]: https://opensource.org/licenses/MIT
