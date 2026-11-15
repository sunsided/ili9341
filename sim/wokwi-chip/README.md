# ILI9341 Wokwi custom chip

Wraps the [`ili9341-sim`](../../ili9341-sim) behavioral model into a
[Wokwi custom chip](https://docs.wokwi.com/chips-api/getting-started) with a 240×320
display.

**[Live demo on wokwi.com](https://wokwi.com/projects/480785747677505537)**

```sh
./build.sh   # produces ili9341.chip.wasm next to ili9341.chip.json
```

Reference it from a project's `wokwi.toml`:

```toml
[[chip]]
name = 'ili9341'
binary = '../../sim/wokwi-chip/ili9341.chip.wasm'
```

and add a `chip-ili9341` part to `diagram.json`.

Projects outside this repository (on wokwi.com, in VS Code or with `wokwi-cli`) can use
the `chip.zip` attached to each GitHub Release instead of building the chip. Add a
`chip-ili9341` part and a dependency to `diagram.json`; no `[[chip]]` entry is needed:

```json
"dependencies": { "chip-ili9341": "github:sunsided/ili9341@0.2.0" }
```

The version is given without the `v` prefix of the release tag; `@v0.2.0` fails to load.

| Pin               | Function                                              |
|-------------------|-------------------------------------------------------|
| `CS`              | SPI chip select, active low                           |
| `DC`              | `D/CX`: low for commands, high for parameters and data |
| `SCK`, `MOSI`     | SPI clock and data in                                 |
| `MISO`            | SPI data out, for reads                               |
| `WR`              | Parallel write strobe; data latched on the rising edge |
| `RD`              | Parallel read strobe; the chip drives `D0`–`D15` while it is low |
| `TE`              | Tearing effect output: high during vertical blanking while enabled |
| `D0`–`D15`        | Parallel data lines (`D0`–`D7` on the 8-bit bus)      |
| `RST`             | Reset input, active low                               |
| `VCC`, `GND`, `LED` | Not used                                            |

The `interface` attribute selects the bus like the real chip's `IM[3:0]` straps: `"0"`
SPI (default), `"1"` 8-bit parallel, `"2"` 16-bit parallel.

Unlike Wokwi's built-in `wokwi-ili9341` part, the chip answers reads over SPI and the
parallel bus, supports the parallel buses, 18-bit color, scrolling, partial and idle
mode, drives `TE` at the frame rate set by `FRMCTR1`, and reports commands sent too
early after a reset or sleep change on the debug console. It follows the model's reading
of the datasheet; see the [`ili9341-sim`](../../ili9341-sim/src/lib.rs) assumptions.
