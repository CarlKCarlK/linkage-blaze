# Changelog

## 0.2.0

- **Breaking:** `Item3d::project` now returns Linkage Blaze's own
  `render::Item2d` (stroke, ellipse, circle in pixel space) instead of
  Device Envoy's `DrawItem`. `Item2d` implements embedded-graphics `Drawable`
  with `Rgb888` color, so it draws onto any `DrawTarget` (use
  `.color_converted()` for `Rgb565` displays). It rasterizes the same pixels
  as the Device Envoy `DrawItem` it replaces.
- `device-envoy-core` is now an optional dependency, turned on only by the
  `examples-*` features. The library itself no longer depends on Device Envoy.
- `linkage_blaze::Error` now implements `core::error::Error` (previously only
  in tests), so it works with `?` into `Box<dyn Error>`.
- Rewrote the `render` docs: the module example now draws a small robot arm,
  with its picture embedded via the new `doc-images` feature (on for docs.rs).
  `Stroke`, `Disk`, `Sphere`, and `Projection` each have their own example.

## 0.1.11

- Updated the Core, RP, and ESP dependencies and CI/Pages integration to
  Device Envoy 0.2.0 (RP 0.2.1). Device Envoy's declaration macros are now
  defined with `macro-schema`.
- Updated the native RP and ESP examples to the Device Envoy 0.2.0 API:
  `init_and_start!(p)` with `new_rmt80` and `new_ledc`, and no `paste`
  dependency in user crates.
- Moved `defmt` to 1.2.0.
- Kept the existing immutable demo and gallery versions because Device Envoy
  0.2.0 does not change the WASM implementation or browser shell assets.

## 0.1.10

- Updated the Core, RP, and ESP dependencies and CI/Pages integration to
  Device Envoy 0.1.6.
- Updated native CYD examples to use Device Envoy's corrected runtime display
  orientation behavior and landscape calibration buffer sizing.
- Kept the existing immutable demo and gallery versions because Device Envoy
  0.1.6 does not change the WASM implementation or browser shell assets.

## 0.1.9

- Updated the Core, RP, and ESP dependencies and CI integration to Device
  Envoy 0.1.5.
- Added immutable `v8` snapshots for the Armatron demo and gallery page,
  capturing Device Envoy's reduced browser touch latency.
- Updated generated ESP examples to support displays whose reset is wired to
  the board reset rather than a dedicated GPIO.

## 0.1.8

- Updated the Core, RP, and ESP dependencies and CI integration to Device
  Envoy 0.1.4.
- Added immutable `v7` snapshots for the Clock, Skeleton Clock, and Armatron
  demos and for the gallery page, capturing material changes made after the
  previous `v6` snapshots.
- Added a canonical release checklist covering demo-version audits, automatic
  WASM/Pages deployment, crate publication, tagging, and final verification.

## 0.1.7

- Updated the CYD examples for Device Envoy 0.1.3's simplified drawing,
  tiling, and fallible touch APIs.
- Refined the clock and skeleton-clock example layout and imports.
