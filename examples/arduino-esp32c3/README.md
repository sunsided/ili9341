# ILI9341 demo for the Wokwi web editor

wokwi.com no longer compiles Rust online, so this demo uses an Arduino sketch. It sends
raw ILI9341 commands over SPI, with the same init sequence as the `ili9341-ng` driver,
and draws the same picture as the [Rust example](../esp32c3): color bars, a circle and
the text `ili9341-ng`, in landscape. It uses Wokwi's built-in `wokwi-ili9341` part, so
no custom chip is needed.

| ESP32-C3 | ILI9341     |
|----------|-------------|
| GPIO6    | SCK         |
| GPIO7    | MOSI (SDI)  |
| GPIO10   | CS          |
| GPIO4    | D/C         |
| GPIO5    | RST         |
| 3V3      | VCC, LED    |

Live project: <TODO>

To recreate it on wokwi.com:

1. Start a new **ESP32-C3** Arduino project.
2. Replace `sketch.ino` and `diagram.json` with the files from this directory.
3. Start the simulation.
