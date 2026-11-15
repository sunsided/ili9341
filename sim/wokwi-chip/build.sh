#!/usr/bin/env sh
# Builds the chip and places it next to ili9341.chip.json, as Wokwi expects.
set -eu
cd "$(dirname "$0")"
cargo build --release
cp target/wasm32-unknown-unknown/release/ili9341_wokwi_chip.wasm ili9341.chip.wasm
