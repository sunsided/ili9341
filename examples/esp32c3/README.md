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

Uses Wokwi's built-in `wokwi-ili9341` part. Requires
`rustup target add riscv32imc-unknown-none-elf`,
[`wokwi-cli`](https://docs.wokwi.com/wokwi-ci/cli-installation) and `WOKWI_CLI_TOKEN`.

```sh
cargo build --release
wokwi-cli . --scenario scenarios/draw.yaml --timeout 10000
```

## Run on hardware

```sh
cargo run --release   # uses espflash
```

If colors look swapped or inverted, call `set_color_order(ColorOrder::Rgb)` or
`set_inverted(true)` after `init`.
