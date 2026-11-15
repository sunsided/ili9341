# ILI9341 on ESP32-C3

Draws a test pattern (color bars, a circle and text) on an ILI9341 over SPI with the
blocking `ili9341-ng` driver and `embedded-graphics`.

| ESP32-C3 | ILI9341     |
|----------|-------------|
| GPIO6    | SCK         |
| GPIO7    | MOSI (SDI)  |
| GPIO10   | CS          |
| GPIO4    | D/C         |
| GPIO5    | RST         |
| 3V3      | VCC, LED    |

## Run in Wokwi

`diagram.json` uses Wokwi's built-in `wokwi-ili9341` part; `diagram-chip.json` swaps in
the [`ili9341-sim` chip](../../sim/wokwi-chip/README.md) and wires MISO to GPIO2.
Requires `rustup target add riscv32imc-unknown-none-elf wasm32-unknown-unknown`,
[`wokwi-cli`](https://docs.wokwi.com/wokwi-ci/cli-installation) and `WOKWI_CLI_TOKEN`.

```sh
../../sim/wokwi-chip/build.sh
cargo build --release
wokwi-cli . --scenario scenarios/draw.yaml --timeout 10000 \
  --screenshot-part lcd1 --screenshot-time 1500 --screenshot-file screenshot.png
wokwi-cli . --diagram-file diagram-chip.json --scenario scenarios/draw.yaml --timeout 10000 \
  --screenshot-part lcd1 --screenshot-time 1500 --screenshot-file screenshot-chip.png
```

The scenario only checks the serial output; look at the screenshots to see what the
display shows. Both parts show the same picture. CI uploads them as the
`wokwi-screenshot` artifact.

## Read back (`src/bin/readback.rs`)

Writes 18-bit pixels, reads them back with the display ID and power mode, and prints
`readback ok` if the pixels match. It needs MISO, so it runs against the chip; Wokwi's
built-in part returns zeros for every read. The SPI clock is 6 MHz, the datasheet's read
limit.

```sh
wokwi-cli . --diagram-file diagram-chip.json --scenario scenarios/readback.yaml \
  --elf target/riscv32imc-unknown-none-elf/release/readback --timeout 10000
```

On hardware, wire `SDO` to GPIO2 and run `cargo run --release --bin readback`.

## Async with DMA (`src/bin/async.rs`)

Draws the same picture with the async driver: into a 320×40 `embedded-graphics`
framebuffer, one band of rows at a time, each flushed with `draw_raw` over the async,
DMA-backed SPI bus. It runs the future with `embassy_futures::block_on` and a blocking
delay; under an executor, use `embassy_time::Delay` instead.

```sh
wokwi-cli . --scenario scenarios/draw.yaml --timeout 10000 \
  --elf target/riscv32imc-unknown-none-elf/release/async \
  --screenshot-part lcd1 --screenshot-time 1500 --screenshot-file screenshot-async.png
```

On hardware, run `cargo run --release --bin async`.

## Run on hardware

```sh
cargo run --release   # uses espflash
```

If colors look swapped or inverted, call `set_color_order(ColorOrder::Rgb)` or
`set_inverted(true)` after `init`.
