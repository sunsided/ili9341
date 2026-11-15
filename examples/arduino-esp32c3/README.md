# ILI9341 demo for the Wokwi web editor

wokwi.com no longer compiles Rust online, so this demo uses an Arduino sketch. It sends
raw ILI9341 commands over SPI, with the same init sequence as the `ili9341-ng` driver,
and draws the same picture as the [Rust example](../esp32c3): color bars, a circle and
the text `ili9341-ng`, in landscape. Then it reads the display ID and power mode back
and prints them.

Two diagrams, one sketch:

- `diagram.json` uses Wokwi's built-in `wokwi-ili9341` part. Reads return zeros.
- `diagram-chip.json` uses the [`ili9341-sim` chip](../../sim/wokwi-chip/README.md),
  loaded from this repository's GitHub Release via its `dependencies` entry. It answers
  the reads: `readback id=8A 93 41 power_mode=9C`.

| ESP32-C3 | ILI9341     |
|----------|-------------|
| GPIO6    | SCK         |
| GPIO7    | MOSI (SDI)  |
| GPIO2    | MISO (SDO)  |
| GPIO10   | CS          |
| GPIO4    | D/C (`DC` on the chip) |
| GPIO5    | RST         |
| 3V3      | VCC, LED    |

Live project (built-in part): <TODO>

Live project (ili9341-sim chip): <TODO>

To recreate them on wokwi.com:

1. Start a new **ESP32-C3** Arduino project.
2. Replace `sketch.ino` with the file from this directory, and `diagram.json` with
   `diagram.json` or `diagram-chip.json` from this directory.
3. Start the simulation.
