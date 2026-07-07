<!-- todo0 consider deleting this spec now that the work below is implemented; kept for now as a record of what changed and why. -->

# Armatron Example on ESP32-C6 with One-SPI Touch

## Overview

Give ESP32-C6 (and any other board with only one SPI peripheral) a real, working `armatron`
example by driving display and touch over a single shared SPI bus via `CydEspOneSpi`, instead of
the two-SPI `CydEsp` the original `armatron` example requires.

## Status

Implemented. `CydEspOneSpi` (in the sibling `device-envoy` repo,
`crates/device-envoy-esp/src/cyd/one_spi.rs`) now drives real ILI9341/XPT2046 hardware over a
shared bus, and a new generated example family, `armatron_one_spi_<chip>_<board>`, exists
alongside the original two-SPI `armatron_<chip>_<board>` family. See "What changed" below.

## An earlier draft of this spec had wrong pin numbers — don't reuse them

A prior version of this document proposed GPIO25/32/39 for the ESP32-C6 display CS/DC/RST pins.
That was wrong on two counts: those are actually the **classic ESP32's touch bus SCK/MOSI/MISO**
pins (see `cyd_touch_wiring` for `ChipId::Esp32` in
`crates/device-envoy-esp/xtask/src/boards.rs` in the `device-envoy` repo), not display pins, and
ESP32-C6 doesn't even expose GPIO32 or GPIO39. Its proposed touch CS/IRQ pins (GPIO5/GPIO4) also
collided with its own display DC/RST pins.

**Do not hand-pick GPIOs for a `Cyd` example.** Every board's real, already-validated wiring
lives in `BoardProfile::{cyd_display_wiring, cyd_touch_wiring, button_pin}` in that same
`boards.rs`, and the `cargo xtask generate-board-examples` template pipeline (in linkage-blaze's
`xtask/src/linkage_esp_examples_generated.rs`) turns that data into per-board example source
automatically — see `crates/linkage-blaze-esp/examples/c6/devkitc1_n8/ballet.rs` for the pins
ESP32-C6-devkitc1-n8 actually uses (SCK=19, MOSI=18, MISO=20, CS=21, DC=4, RST=5, backlight=7).

## What changed

1. **`device-envoy` repo**: `CydDisplayEsp` and `CydTouchEsp` (the ILI9341/XPT2046 drivers) were
   made generic over the `embedded-hal` SPI device type, defaulting to the existing
   exclusively-owned-peripheral type. `CydEspOneSpi` now builds one real
   `esp_hal::spi::master::Spi`, shares it via one `embedded_hal_bus::spi::RefCellDevice` per chip
   select (display and touch), and reuses those same drivers — no separate/duplicated
   implementation. It runs the shared bus at the touch controller's 2.5 MHz ceiling (see
   `TOUCH_SPI_HZ`), since both peripherals share one clock config; this is slower than the
   two-SPI design's 60 MHz display bus, an accepted trade-off for one-SPI boards. It does not
   implement `CydParts`, matching that trait's documented rule that shared-bus backends can't
   safely split into independently-owned parts. `CydEspOneSpi::new` is now `async`, generic over
   `R: Button` (mirroring the two-SPI `CydEsp::new` exactly), and runs the same shared
   `ensure_calibration` flow — flash-backed load/save, interactive recalibration via the button —
   instead of taking a hardcoded identity `CalibrationConfig`. No calibration-specific code was
   needed in `device-envoy-core`: `ensure_calibration` was already generic over any
   `CydTouchUncalibrated`/`CydDisplay`/`FlashBlock`/`Button`, so this was purely a call-site change
   in `one_spi.rs`. The bespoke `CydOneSpiInitError` type was removed (unused once construction
   errors flow through the same `device_envoy_esp::Error` the calibration flow already uses).

2. **linkage-blaze repo**: a new template,
   `crates/linkage-blaze-esp/examples/templates/armatron_one_spi.rs.j2`, generates a real
   `armatron_one_spi_<chip>_<board>` example for every board profile, reusing the same
   `linkage_blaze_example_core::armatron::armatron()` app logic as the original two-SPI example
   (that function is generic over `Cyd`, so it needed no changes). The template pulls its pins
   straight from each board's `cyd_display_wiring` (shared bus + display) and
   `cyd_touch_wiring.{cs,irq}` (touch chip-select/IRQ) — the same source ballet.rs already uses.
   The original `armatron.rs.j2` (two-SPI) is unchanged and still requires 2 SPI peripherals, so
   it stays a placeholder stub on C6 and every other one-SPI board.

3. Because the new template wires up display, touch CS/IRQ, and the button simultaneously (the
   two-SPI template and `cyd_touch_paint` never do this on one-SPI boards, since they're always
   placeholder-stubbed there), it surfaced a **latent pin conflict in the ESP32-C2 board
   profile** (`Generic` and `Devkitm1V1_0`): touch CS/IRQ and the button collide with display
   pins. Rather than invent new physical wiring for hardware we can't verify, the generator gained
   a `no_cyd_one_spi_pin_conflict` requirement (`BoardTemplateRequirement::NoCydOneSpiPinConflict`
   in `xtask/src/linkage_esp_examples_generated.rs`) that checks every board's
   `cyd_display_wiring` + `cyd_touch_wiring.{cs,irq}` + `button_pin` for distinctness and falls
   back to the existing placeholder-stub mechanism when they collide. ESP32-C2 currently renders
   as an honest placeholder for `armatron_one_spi`; every other board profile is conflict-free.

## Follow-ups

- If ESP32-C2's touch CS/IRQ should get non-conflicting pins, that's a `boards.rs` change in the
  `device-envoy` repo requiring real hardware wiring knowledge — not something to guess from this
  repo.
