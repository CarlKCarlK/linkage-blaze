<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# CydRpOneSpi + `armatron_one_spi` for RP

## Motivation

The two-SPI `CydRp` bundle (`device-envoy-rp/src/cyd.rs`) drives the display on `SPI0`
and touch on `SPI1` — two independent physical peripherals. Real CYD-family boards
wire the ILI9341/ST7789 display and the XPT2046 touch controller onto the **same**
physical SPI bus (shared SCK/MOSI/MISO, separate CS lines). `device-envoy-esp`
already has this shared-bus variant, `CydEspOneSpi`
(`device-envoy-esp/src/cyd/one_spi.rs`), used by the ESP `armatron_one_spi` example
family. RP has no equivalent yet.

This spec adds `CydRpOneSpi` to `device-envoy-rp`, modeled directly on
`CydEspOneSpi`, and a new `armatron_one_spi` example in
`linkage-blaze-examples-rp` that uses it — mirroring how
`linkage-blaze-examples-esp` already has both `armatron` and `armatron_one_spi`
per board.

## Prior art to mirror

- `device-envoy-esp/src/cyd/one_spi.rs` — `CydEspOneSpi`: builds one
  `esp_hal::spi::master::Spi`, wraps it in
  `embassy_sync::blocking_mutex::Mutex<NoopRawMutex, RefCell<SharedSpiBus>>`, and
  gives display and touch each their own
  `embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig` (own CS pin,
  own `Config`/clock speed, re-applied before each transaction).
- `device-envoy-esp/src/cyd.rs` — `CydDisplayEsp<D: SpiDevice<u8> = ...>` and
  `CydTouchUncalibratedEsp<D = ...>` are generic over the SPI device type, with a
  `pub(crate) new_from_device` / `from_device` constructor path used only by
  `one_spi`, alongside the normal `new` path used by the two-SPI `CydEsp`.
- `linkage-blaze-examples-esp/examples/templates/armatron_one_spi.rs.j2` — the
  example itself: same `armatron()` core-crate loop as the two-SPI example, just
  constructed via `CydEspOneSpi::new` with one SPI peripheral instance and two CS
  pins instead of two peripherals.

## Key difference from ESP to account for

`device-envoy-rp/src/cyd/display.rs` currently builds the display bus **TX-only**
(`Spi::new_blocking_txonly`, no MISO) because the two-SPI display never needs to
read. A shared bus must be full-duplex (`Spi::new_blocking`, with MISO wired) since
touch reads response bytes over the same bus. `CydRpOneSpi` must construct the
underlying `embassy_rp::spi::Spi` in full-duplex mode; the two-SPI `CydDisplayRp`
path is unaffected and keeps using TX-only.

`embassy_rp::spi::Spi<'d, T, M>` already implements `embassy_embedded_hal::SetConfig`
(confirmed in `embassy-rp-0.10.0/src/spi.rs`), so
`SpiDeviceWithConfig` — the same shared-bus wrapper `CydEspOneSpi` uses — works
unchanged against the RP HAL. `device-envoy-rp` needs a new dependency on
`embassy-embedded-hal` (already a dependency of `device-envoy-esp`).

## `device-envoy-rp` changes

1. **Genericize `CydDisplayRp` and `CydTouchRp` over their SPI device type**, same
   shape as the ESP versions:
   - `device-envoy-rp/src/cyd/display.rs`: `pub(crate) struct CydDisplayRp<D: SpiDevice<u8> = CydDisplaySpiDevice>`;
     add a `pub(crate) fn new_from_device(spi_device: D, dc_pin, rst_pin, backlight_pin, orientation)`
     constructor alongside the existing `new` (which keeps building its own
     TX-only exclusive device and then calls into the shared inner logic).
   - `device-envoy-rp/src/cyd/touch.rs`: `pub(crate) struct CydTouchRp<D = CydTouchSpiDevice>`;
     add a `pub(crate) fn from_device(spi_device: D, irq_pin)` alongside the
     existing `new`.
   - `device-envoy-rp/src/cyd.rs`: genericize the public wrapper types
     `CydDisplayRp`, `CydTouchUncalibratedRp`, `CydTouchRp`, `CydFrameRp` over `D:
     embedded_hal::spi::SpiDevice<u8>` (defaulting to the existing exclusive-device
     type), exactly like `CydDisplayEsp<D>` / `CydFrameEsp<'a, D>` / etc. All trait
     impls (`CydDisplay`, `CydTouch`, `CydFrame`, `DrawTarget`, `Dimensions`,
     `PixelTarget`) gain the same `<D: SpiDevice<u8>>` bound the ESP versions have.
     `CydRp`/`CydRpUncalibrated` (the two-SPI bundle) keep using the default `D`
     and are otherwise unchanged.

2. **New module `device-envoy-rp/src/cyd/one_spi.rs`** exporting `CydRpOneSpi`,
   directly parallel to `CydEspOneSpi`:
   - Build one `embassy_rp::spi::Spi<'static, T, Blocking>` in full-duplex mode
     (`Spi::new_blocking`, taking sck/mosi/miso pins), generic over
     `T: embassy_rp::spi::Instance` (see the peripheral-genericity decision
     below) — not hardcoded to `SPI0`. Wrap it in
     `embassy_sync::blocking_mutex::Mutex<NoopRawMutex, RefCell<...>>` behind a
     `StaticCell` (same pattern as `CydEspOneSpi::new`).
   - Give the display and touch each an
     `embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig`, with
     independent CS `Output` pins and independent `embassy_rp::spi::Config`
     (display clock from `DEFAULT_DISPLAY_SPI_HZ`/caller-supplied `display_spi_hz`,
     touch clock from `TOUCH_SPI_HZ`).
   - Build the display via `CydDisplayRp::new_from_device(...)` and touch via
     `CydTouchUncalibratedRp::from_device(...)`, then run the same
     `ensure_calibration` flow `CydRp::new` already uses.
   - `CydRpOneSpi::new_static` / `SCREEN_PIXELS` mirror `CydRp`'s.
   - Implement `Cyd` for `CydRpOneSpi` (display + touch parts) but **not**
     `CydParts` — same rationale as `CydEspOneSpi`: shared-bus backends can't
     safely split into independently-owned parts because both halves reference
     the same mutex-guarded bus.
   - Constructor argument order/names mirror `CydEspOneSpi::new` exactly, adapted
     to RP's `Peri<'static, T>` pin type. Per the decision below, the SPI
     peripheral is generic: `CydRpOneSpi::new<T: spi::Instance, ...>(spi: Peri<'static, T>, ...)`,
     so callers can pick either `SPI0` or `SPI1` for the shared bus (mirroring
     ESP's `impl spi::master::Instance` genericity) instead of being locked to
     `SPI0`.

3. **`Cargo.toml`**: add `embassy-embedded-hal` as a dependency (same version as
   `device-envoy-esp` pins, `0.6.0`, `default-features = false`).

4. **Errors**: `CydError` (`device-envoy-rp/src/cyd.rs`) already covers
   `DisplayInit` / `TouchInit` / `DisplayFlush` generically — no new variants
   needed; `CydRpOneSpi::new` reuses it exactly like `CydEspOneSpi::new` does.

## `linkage-blaze` changes

1. New example `crates/linkage-blaze-examples-rp/examples/armatron_one_spi.rs`,
   copied from `examples/armatron.rs` with the same `main`/`inner_main` structure
   and the same `linkage_blaze_core::examples::armatron::armatron` loop, but:
   - imports `CydRpOneSpi` instead of `CydRp` from `device_envoy_rp::cyd`,
   - constructs via `CydRpOneSpi::new(...)` with one SPI peripheral (`p.SPI0`),
     shared sck/mosi/miso pins, and two CS pins (`lcd_cs_pin`, `touch_cs_pin`) plus
     `touch_irq_pin` — no second SPI peripheral, no separate touch sck/mosi/miso.
   - Pin assignment: made up, not tied to a specific real board — reuse
     `armatron.rs`'s display pins (`PIN_18` sck, `PIN_19` mosi, `PIN_16` miso,
     `PIN_17` lcd_cs, `PIN_20` dc, `PIN_21` rst, `PIN_22` backlight) for the
     shared bus, and keep `armatron.rs`'s touch CS/IRQ pins (`PIN_13` touch_cs,
     `PIN_14` touch_irq) since those don't collide with the bus pins. The
     compiler (via `ClkPin<T>`/`MosiPin<T>`/`MisoPin<T>` bounds) will reject the
     assignment if it's invalid for a given chip/target, so no further
     confirmation is needed before implementing — `cargo check-all` across
     pico1/pico2 is the actual validator.
   - `MainError`/`From` impls copied unchanged (same `CydError` shape as today).

2. `crates/linkage-blaze-examples-rp/Cargo.toml`: add

   ```toml
   [[example]]
   name = "armatron_one_spi"
   path = "examples/armatron_one_spi.rs"
   required-features = ["armatron"]
   ```

   reusing the existing `armatron` feature (same pattern the ESP crate uses for
   its `armatron_one_spi_*` entries — one core-crate feature gates both wiring
   variants).

## Verification

- `device-envoy-rp`: `cargo check-all` (per that crate's `AGENTS.md`) to confirm
  the genericized `CydDisplayRp`/`CydTouchRp` still build for both pico1/pico2
  targets and that `CydRpOneSpi` compiles against the new `embassy-embedded-hal`
  dependency.
- `linkage-blaze`: `just check-all` to build `armatron_one_spi` for RP alongside
  the existing examples.
- Neither of these can be exercised against real hardware in this environment;
  a hardware smoke test (calibration flow + full-frame flush) is required before
  calling the one-SPI wiring confirmed, same as any CYD display change.

## Decisions (resolved)

- Example pin assignment is made up (not tied to a real board); the per-chip
  `ClkPin<T>`/`MosiPin<T>`/`MisoPin<T>` trait bounds make the compiler the
  actual validator across pico1/pico2 targets.
- `CydRpOneSpi` is generic over the SPI peripheral instance
  (`T: embassy_rp::spi::Instance`), not hardcoded to `SPI0`, so callers can pick
  either `SPI0` or `SPI1` for the shared bus.
