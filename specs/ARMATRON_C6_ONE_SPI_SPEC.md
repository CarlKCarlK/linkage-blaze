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
   `esp_hal::spi::master::Spi`, shares it via one `embassy_embedded_hal::shared_bus::blocking::
   spi::SpiDeviceWithConfig` per chip select (display and touch), and reuses those same drivers —
   no separate/duplicated implementation. It does not implement `CydParts`, matching that trait's
   documented rule that shared-bus backends can't safely split into independently-owned parts.
   `CydEspOneSpi::new` is now `async`, generic over `R: Button` (mirroring the two-SPI
   `CydEsp::new` exactly), and runs the same shared `ensure_calibration` flow — flash-backed
   load/save, interactive recalibration via the button — instead of taking a hardcoded identity
   `CalibrationConfig`. No calibration-specific code was needed in `device-envoy-core`:
   `ensure_calibration` was already generic over any `CydTouchUncalibrated`/`CydDisplay`/
   `FlashBlock`/`Button`, so this was purely a call-site change in `one_spi.rs`. The bespoke
   `CydOneSpiInitError` type was removed (unused once construction errors flow through the same
   `device_envoy_esp::Error` the calibration flow already uses).

2. **Per-device SPI clock speed**: an initial version of this bundle ran the *entire* shared bus
   at the touch controller's 2.5 MHz ceiling (`TOUCH_SPI_HZ`), since both peripherals shared one
   clock config — this capped every full-frame flush at ~2 fps (measured: ~492ms/flush), making
   `armatron` unusably slow. Fixed by giving each device its own `spi::master::Config` via
   `SpiDeviceWithConfig`, which esp-hal supports out of the box (`Spi` already implements
   `embassy_embedded_hal::SetConfig`, calling its own `apply_config`) — no bespoke wrapper needed.
   Display now runs at `DEFAULT_DISPLAY_SPI_HZ` (60 MHz, same as the two-SPI design); touch stays at
   `TOUCH_SPI_HZ` (2.5 MHz), and only ever engages when `T_IRQ` is low (touch was already
   IRQ-gated before this fix, so idle frames pay ~zero touch-read cost). Measured on real
   ESP32-C6-devkitc1-n8 hardware: `armatron` now runs at **9.8 fps — parity with the two-SPI
   classic ESP32 board**. An earlier A/B test also confirmed the per-device config genuinely takes
   effect: an explicit 20 MHz request measured slower (~90ms/flush) than the 60 MHz request
   (~59ms/flush) in an isolated full-frame-flush benchmark; both numbers run above their pure
   bit-rate predictions because of fixed per-frame overhead (ILI9341 addressing commands, CS/DC
   toggling, driver-side pixel iteration) that doesn't scale with SPI clock. With `armatron` now
   at parity with the two-SPI board, the ~9.8 fps ceiling is a shared, board-agnostic bottleneck
   (likely app-level draw/CPU work) — not a one-SPI-specific gap left to close.

3. **linkage-blaze repo**: a new template,
   `crates/linkage-blaze-esp/examples/templates/armatron_one_spi.rs.j2`, generates a real
   `armatron_one_spi_<chip>_<board>` example for every board profile, reusing the same
   `linkage_blaze_example_core::armatron::armatron()` app logic as the original two-SPI example
   (that function is generic over `Cyd`, so it needed no changes). The template pulls its pins
   straight from each board's `cyd_display_wiring` (shared bus + display) and
   `cyd_touch_wiring.{cs,irq}` (touch chip-select/IRQ) — the same source ballet.rs already uses.
   The original `armatron.rs.j2` (two-SPI) is unchanged and still requires 2 SPI peripherals, so
   it stays a placeholder stub on C6 and every other one-SPI board.

4. Because the new template wires up display, touch CS/IRQ, and the button simultaneously (the
   two-SPI template and `cyd_touch_paint` never do this on one-SPI boards, since they're always
   placeholder-stubbed there), it surfaced a **latent pin conflict in the ESP32-C2 board
   profile** (`Generic` and `Devkitm1V1_0`): touch CS/IRQ and the button collided with display
   pins. The generator gained a `no_cyd_one_spi_pin_conflict` requirement
   (`BoardTemplateRequirement::NoCydOneSpiPinConflict` in
   `xtask/src/linkage_esp_examples_generated.rs`) that checks every board's `cyd_display_wiring` +
   `cyd_touch_wiring.{cs,irq}` + `button_pin` for distinctness and falls back to the existing
   placeholder-stub mechanism when they collide.

5. **ESP32-C2 pin conflict fixed**: both C2 board profiles (`Generic` and `Devkitm1V1_0`) in
   `boards.rs` were rewired using the documented ESP8684-DevKitM-1 v1.1 header table (its 14
   usable GPIOs — 0–10, 18–20 — since GPIO11–17 are reserved for SiP flash), avoiding GPIO8/9
   (strapping pins) and GPIO19/20 (UART0). New wiring: `cyd_display_wiring` sck=6/mosi=7/miso=2/
   cs=10/dc=3/rst=4/backlight=5, `cyd_touch_wiring` cs=0/irq=1, `button_pin` moved from GPIO6 to
   GPIO18. This fixed two things at once: `armatron_one_spi` is no longer placeholder-stubbed on
   C2, and — more importantly — `clock`/`skeleton_clock` were **already broken** on both C2
   boards before this fix (`button_pin` and the old `cyd_display_wiring.cs_pin_num` were both
   literally GPIO6, so the generated file tried to consume `p.GPIO6` twice — a genuine
   `E0382 use of moved value` compile error, unrelated to one-SPI or armatron at all). All of
   clock/skeleton_clock/armatron_one_spi build clean on both C2 boards now. Not yet verified on
   physical ESP32-C2 hardware — the wiring is sourced from the official DevKitM-1 header docs, not
   independently confirmed on a real board.

## Follow-ups

None outstanding.
