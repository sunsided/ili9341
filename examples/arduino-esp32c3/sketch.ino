// ILI9341 demo for the Wokwi web editor (Arduino, ESP32-C3).
//
// Draws the same picture as the Rust example in ../esp32c3 with raw ILI9341 commands
// over SPI, and prints the same serial line. The command codes and the init sequence
// are those of the ili9341-registers and ili9341-ng crates. Then it reads the display
// ID and power mode back; that needs the ili9341-sim chip (diagram-chip.json), as
// Wokwi's built-in ILI9341 returns zeros for every read.
// Wiring: SCK = GPIO6, MOSI = GPIO7, MISO = GPIO2, CS = GPIO10, D/C = GPIO4, RST = GPIO5.

#include <SPI.h>

constexpr int PIN_SCK = 6;
constexpr int PIN_MOSI = 7;
constexpr int PIN_MISO = 2;
constexpr int PIN_CS = 10;
constexpr int PIN_DC = 4;
constexpr int PIN_RST = 5;

constexpr uint16_t WIDTH = 320; // Landscape.
constexpr uint16_t HEIGHT = 240;

// Command codes.
constexpr uint8_t SWRESET = 0x01;
constexpr uint8_t RDDIDIF = 0x04;
constexpr uint8_t RDDPM = 0x0A;
constexpr uint8_t SLPOUT = 0x11;
constexpr uint8_t DISPON = 0x29;
constexpr uint8_t CASET = 0x2A;
constexpr uint8_t PASET = 0x2B;
constexpr uint8_t RAMWR = 0x2C;
constexpr uint8_t MADCTL = 0x36;
constexpr uint8_t COLMOD = 0x3A;

// MADCTL: row/column exchange (MV) and BGR, the driver's Orientation::Landscape.
constexpr uint8_t MADCTL_LANDSCAPE_BGR = 0x28;
// COLMOD: 16 bits per pixel (RGB565).
constexpr uint8_t COLMOD_RGB565 = 0x55;

constexpr uint16_t BLACK = 0x0000;
constexpr uint16_t RED = 0xF800;
constexpr uint16_t GREEN = 0x07E0;
constexpr uint16_t BLUE = 0x001F;
constexpr uint16_t WHITE = 0xFFFF;
constexpr uint16_t YELLOW = 0xFFE0;

// Power, VCOM, frame rate and gamma settings from Adafruit_ILI9341, as sent by the
// ili9341-ng driver: command, parameter count, parameters. EFh is undocumented.
const uint8_t PANEL_INIT[] = {
  0xEF, 3, 0x03, 0x80, 0x02,
  0xCF, 3, 0x00, 0xC1, 0x30,             // PWCTRLB
  0xED, 4, 0x64, 0x03, 0x12, 0x81,       // PWRONSEQ
  0xE8, 3, 0x85, 0x00, 0x78,             // DTCTRLA
  0xCB, 5, 0x39, 0x2C, 0x00, 0x34, 0x02, // PWCTRLA
  0xF7, 1, 0x20,                         // PUMPRATIO
  0xEA, 2, 0x00, 0x00,                   // DTCTRLB
  0xC0, 1, 0x23,                         // PWCTRL1
  0xC1, 1, 0x10,                         // PWCTRL2
  0xC5, 2, 0x3E, 0x28,                   // VMCTRL1
  0xC7, 1, 0x86,                         // VMCTRL2
  0xB1, 2, 0x00, 0x18,                   // FRMCTR1
  0xB6, 3, 0x08, 0x82, 0x27,             // DISCTRL
  0xF2, 1, 0x00,                         // ENABLE_3G
  0x26, 1, 0x01,                         // GAMSET
  0xE0, 15, 0x0F, 0x31, 0x2B, 0x0C, 0x0E, 0x08, 0x4E, 0xF1, 0x37, 0x07, 0x10, 0x03, 0x0E, 0x09, 0x00, // PGAMCTRL
  0xE1, 15, 0x00, 0x0E, 0x14, 0x03, 0x11, 0x07, 0x31, 0xC1, 0x48, 0x08, 0x0F, 0x0C, 0x31, 0x36, 0x0F, // NGAMCTRL
};

// Glyphs of the 10x20 X11 "fixed" font (public domain), as used by embedded-graphics'
// FONT_10X20: 20 rows of 10 pixels, most significant of the 10 bits on the left.
struct Glyph {
  char c;
  uint16_t rows[20];
};

const Glyph GLYPHS[] = {
  {'-', {0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x1FE, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000}},
  {'1', {0x000, 0x000, 0x000, 0x030, 0x070, 0x0F0, 0x1B0, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x1FE, 0x000, 0x000, 0x000, 0x000}},
  {'3', {0x000, 0x000, 0x000, 0x078, 0x0CC, 0x186, 0x186, 0x006, 0x00C, 0x038, 0x00C, 0x006, 0x186, 0x186, 0x0CC, 0x078, 0x000, 0x000, 0x000, 0x000}},
  {'4', {0x000, 0x000, 0x000, 0x004, 0x00C, 0x01C, 0x03C, 0x06C, 0x0CC, 0x18C, 0x18C, 0x1FE, 0x00C, 0x00C, 0x00C, 0x00C, 0x000, 0x000, 0x000, 0x000}},
  {'9', {0x000, 0x000, 0x000, 0x078, 0x0CC, 0x186, 0x186, 0x186, 0x186, 0x0CE, 0x076, 0x006, 0x006, 0x086, 0x0CC, 0x078, 0x000, 0x000, 0x000, 0x000}},
  {'g', {0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x0FA, 0x18E, 0x18C, 0x18C, 0x18C, 0x0F8, 0x180, 0x0FC, 0x186, 0x186, 0x186, 0x0FC}},
  {'i', {0x000, 0x000, 0x000, 0x000, 0x000, 0x030, 0x030, 0x000, 0x0F0, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x1FE, 0x000, 0x000, 0x000, 0x000}},
  {'l', {0x000, 0x000, 0x000, 0x0F0, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x030, 0x1FE, 0x000, 0x000, 0x000, 0x000}},
  {'n', {0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x1B8, 0x1CC, 0x186, 0x186, 0x186, 0x186, 0x186, 0x186, 0x000, 0x000, 0x000, 0x000}},
};

const SPISettings SPI_SETTINGS(40000000, MSBFIRST, SPI_MODE0);
// Reads need an SPI clock of at most 6.6 MHz.
const SPISettings READ_SETTINGS(6000000, MSBFIRST, SPI_MODE0);

// Sends a command with D/CX low, then its parameters with D/CX high.
void command(uint8_t code, const uint8_t *params = nullptr, size_t len = 0) {
  SPI.beginTransaction(SPI_SETTINGS);
  digitalWrite(PIN_CS, LOW);
  digitalWrite(PIN_DC, LOW);
  SPI.transfer(code);
  digitalWrite(PIN_DC, HIGH);
  if (len > 0) {
    SPI.writeBytes(params, len);
  }
  digitalWrite(PIN_CS, HIGH);
  SPI.endTransaction();
}

// Sends a read command and reads its response in the same transaction, with D/CX low
// throughout, as the ili9341-ng driver does.
void readCommand(uint8_t code, uint8_t *buffer, size_t len) {
  SPI.beginTransaction(READ_SETTINGS);
  digitalWrite(PIN_CS, LOW);
  digitalWrite(PIN_DC, LOW);
  SPI.transfer(code);
  for (size_t i = 0; i < len; i++) {
    buffer[i] = SPI.transfer(0);
  }
  digitalWrite(PIN_CS, HIGH);
  SPI.endTransaction();
}

// Sets the address window (inclusive end) and starts a memory write.
void startWrite(uint16_t x, uint16_t y, uint16_t w, uint16_t h) {
  const uint16_t x1 = x + w - 1, y1 = y + h - 1;
  const uint8_t columns[] = {uint8_t(x >> 8), uint8_t(x), uint8_t(x1 >> 8), uint8_t(x1)};
  const uint8_t pages[] = {uint8_t(y >> 8), uint8_t(y), uint8_t(y1 >> 8), uint8_t(y1)};
  command(CASET, columns, sizeof(columns));
  command(PASET, pages, sizeof(pages));
  command(RAMWR);
}

// Fills a rectangle with one RGB565 color, sent most significant byte first.
void fillRect(uint16_t x, uint16_t y, uint16_t w, uint16_t h, uint16_t color) {
  if (w == 0 || h == 0) {
    return;
  }
  startWrite(x, y, w, h);
  uint8_t chunk[256];
  for (size_t i = 0; i < sizeof(chunk); i += 2) {
    chunk[i] = color >> 8;
    chunk[i + 1] = color & 0xFF;
  }
  uint32_t bytes = uint32_t(w) * h * 2;
  SPI.beginTransaction(SPI_SETTINGS);
  digitalWrite(PIN_CS, LOW);
  digitalWrite(PIN_DC, HIGH);
  while (bytes > 0) {
    const uint32_t n = bytes < sizeof(chunk) ? bytes : sizeof(chunk);
    SPI.writeBytes(chunk, n);
    bytes -= n;
  }
  digitalWrite(PIN_CS, HIGH);
  SPI.endTransaction();
}

// A circle outline around a circle of diameter `d` whose top left corner is (x, y),
// pixel for pixel like embedded-graphics' Circle with a centered 3-pixel stroke: one
// pixel outside, two inside. Drawn as horizontal runs, in doubled coordinates so the
// center may lie between pixels.
void strokeCircle(int x, int y, int d, uint16_t color) {
  const int cx2 = 2 * x + d - 1, cy2 = 2 * y + d - 1;
  const int inner = d - 4, outer = d + 2;
  for (int py = y - 1; py < y + d + 1; py++) {
    int run = -1;
    for (int px = x - 1; px <= x + d + 1; px++) {
      const int dx = 2 * px - cx2, dy = 2 * py - cy2;
      const int d2 = dx * dx + dy * dy;
      const bool on = px < x + d + 1 && d2 >= inner * inner && d2 < outer * outer;
      if (on && run < 0) {
        run = px;
      } else if (!on && run >= 0) {
        fillRect(run, py, px - run, 1, color);
        run = -1;
      }
    }
  }
}

// Draws text with its baseline at y, like embedded-graphics' Text with FONT_10X20.
void drawText(int x, int y, const char *text, uint16_t color) {
  const int top = y - 15; // The font's baseline is at row 15.
  for (; *text; text++, x += 10) {
    for (const Glyph &glyph : GLYPHS) {
      if (glyph.c != *text) {
        continue;
      }
      for (int row = 0; row < 20; row++) {
        for (int col = 0; col < 10; col++) {
          if (glyph.rows[row] & (0x200 >> col)) {
            fillRect(x + col, top + row, 1, 1, color);
          }
        }
      }
    }
  }
}

void setup() {
  Serial.begin(115200);
  pinMode(PIN_CS, OUTPUT);
  pinMode(PIN_DC, OUTPUT);
  pinMode(PIN_RST, OUTPUT);
  digitalWrite(PIN_CS, HIGH);
  SPI.begin(PIN_SCK, PIN_MISO, PIN_MOSI, -1);

  // Hardware reset: RESX low for at least 10 us, then 120 ms until commands are accepted.
  digitalWrite(PIN_RST, LOW);
  delayMicroseconds(20);
  digitalWrite(PIN_RST, HIGH);
  delay(120);

  // The driver's init: software reset, panel settings, color and orientation, sleep out.
  command(SWRESET);
  delay(120);
  for (size_t i = 0; i < sizeof(PANEL_INIT);) {
    const uint8_t code = PANEL_INIT[i], len = PANEL_INIT[i + 1];
    command(code, &PANEL_INIT[i + 2], len);
    i += 2 + len;
  }
  command(COLMOD, &COLMOD_RGB565, 1);
  command(MADCTL, &MADCTL_LANDSCAPE_BGR, 1);
  command(SLPOUT);
  delay(120);
  command(DISPON);

  fillRect(0, 0, WIDTH, HEIGHT, BLACK);
  const uint16_t bars[] = {RED, GREEN, BLUE, WHITE};
  for (int i = 0; i < 4; i++) {
    fillRect(i * 80, 0, 80, 40, bars[i]);
  }
  // Circle::with_center((160, 140), 100): top left corner (160, 140) - (100 - 1) / 2.
  strokeCircle(111, 91, 100, YELLOW);
  drawText(110, 135, "ili9341-ng", WHITE);

  Serial.printf("ILI9341 ok %ux%u\n", WIDTH, HEIGHT);

  // RDDIDIF: one dummy clock precedes the 24 data bits.
  uint8_t id[4];
  readCommand(RDDIDIF, id, sizeof(id));
  const uint32_t bits = (uint32_t(id[0]) << 24 | uint32_t(id[1]) << 16 | uint32_t(id[2]) << 8 | id[3]) << 1;
  uint8_t powerMode;
  readCommand(RDDPM, &powerMode, 1);
  Serial.printf("readback id=%02X %02X %02X power_mode=%02X\n", uint8_t(bits >> 24), uint8_t(bits >> 16),
                uint8_t(bits >> 8), powerMode);
}

void loop() {
  delay(1000);
}
