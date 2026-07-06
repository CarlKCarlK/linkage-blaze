# CYD Module-Surface Follow-Ups: `touch` Submodule, Screen Constants, and Root Slimming

<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

Follow-up cleanups to `device_envoy_core::cyd` raised while reviewing the docs
index after `CYD_CONTIGUOUS_PIXELS_AND_CORE_ERRORS_SPEC.md` landed. Same
two-repo scope and rules as that spec:

- `/home/carlk/programs/mcu/device-envoy` (crates `device-envoy-core`, `device-envoy-esp`, `device-envoy-rp`)
- `/home/carlk/programs/linkage-blaze` (crates `linkage-blaze-example-core`, `linkage-blaze-armatron-wasm`, and anything else that fails to compile)

Verification for every phase: `just check-all` in **both** repos must pass,
plus `cargo doc -p device-envoy-core --no-deps --features host,wasm` with a
clean docs build and a `cyd` index matching the target layout below. Follow
all rules in each repo's `AGENTS.md` (no `#[allow]`, no deleted TODOs, no
`mod.rs`, no compatibility re-exports/shims).

## Motivation

`CYD_CONTIGUOUS_PIXELS_AND_CORE_ERRORS_SPEC.md` Part 3 moved the display-side
plumbing into `cyd::display`, but the resulting root is still asymmetric and
carries a few items that don't fit its "device model only" rule:

- The touch-side plumbing (`CydRawTouch`, `RawTouchEvent`, `RawPoint`) hides
  inside `calibration`, so `display` has no touch-side mirror.
- `SCREEN_WIDTH`/`SCREEN_HEIGHT` silently assume landscape; [`Orientation`]
  already models oriented dimensions properly.
- `Tiles`/`TileGrid` are display-only strategy but sit in a root-level
  `tiling` submodule beside `display`.
- `CydFrame` (reachable only through `CydDisplay::Frame`) and `TouchEvent`
  (reachable only through `CydTouch::read`) are part-specific, not shared
  device model.
- `EnsureCalibrationError`/`EnsureCalibrationOutcome`/`ensure_calibration`
  are re-exported at the root even though they belong to calibration.

## Target layout

`cyd` root after this spec:

- Traits: `Cyd`, `CydDisplay`, `CydTouch`, `CydFlushError`
- `SCREEN_PIXELS` (orientation-independent; keeps
  `CydStaticEsp<{ CydEsp::SCREEN_PIXELS }>` buffer sizing working)
- Public submodules: `display`, `touch`

`cyd::display` after this spec (adds to the current contents):

- `CydFrame` (moved from the root — it is `CydDisplay::Frame`'s bound, i.e.
  display-side model, not shared model)
- `RectanglePixels`, `DrawItem`, `Image565Fixed`, `Image565Mask`,
  `Image565View`, `Orientation`, the `tga565*` macro re-exports,
  `pub(crate) ContiguousPixels` (all unchanged from the previous spec)
- `tiling` moved here as `cyd::display::tiling` (contents unchanged:
  `TileGrid`, `Tiles`, `rectangle_pixel_count`, `max_rectangle_pixel_count`)

New `cyd::touch` submodule (renamed/reshaped from `calibration`):

- `TouchEvent` (moved from the root — it is `CydTouch::read`'s return type)
- The raw-touch device plumbing: `CydRawTouch`, `RawTouchEvent`, `RawPoint`
- `calibration` nested as `cyd::touch::calibration` holding everything else
  the current `calibration` module has (flow, driver, UI helpers, constants,
  `CalibrationConfig`, `EnsureCalibration*`, `ensure_calibration`), minus the
  raw-touch types which move up one level to `touch`
- No re-exports of any of this at the `cyd` root (drops the current
  `pub use calibration::{EnsureCalibrationError, EnsureCalibrationOutcome, ensure_calibration}`)

## Decision notes (answers to the review questions)

- **`EnsureCalibrationError` into `crate::Error`?** No — it is generic over
  `<DeviceError, FlashError>` and `Error` is non-generic; this is the same
  "generic wrappers stay out" rule from the previous spec's decision table.
  What *does* change: its root re-export goes away; the canonical path becomes
  `cyd::touch::calibration::EnsureCalibrationError`.
- **`CydFlushError` into `src/error.rs`?** No — it is not an error type but
  the marker *bound* on the CYD part traits' associated `Error` types, i.e.
  part of the device-model contract and meaningless outside `cyd`. It stays
  at the `cyd` root. (`error.rs` stays the home of the unified `Error` enum
  only.)
- **`SCREEN_WIDTH`/`SCREEN_HEIGHT`?** Demote to `pub(crate)` rather than
  rename: their only in-crate uses are `Orientation`'s oriented dimensions
  and calibration's native-panel-coordinate math. Public consumers switch to
  `Orientation` (`const fn width()/height()/size()/pixels()` already exist).
  `SCREEN_PIXELS` stays public at the root: it is orientation-independent and
  load-bearing for `CydStaticEsp`/`CydStaticRp` buffer sizing in board
  examples and templates.
- **`CydFrame` vs `RectanglePixels`?** Different roles, keep both:
  `CydFrame` is the active, device-owned, in-progress drawing surface (a
  `DrawTarget` + text + `flush()` to the panel); `RectanglePixels` is a
  passive read-only view (`width`/`height`/`raw_pixels`) over an
  already-rendered RGB565 buffer so `CydDisplay::flush_at` can present any
  buffer (`RegionBuffer`, `RegionView`, `MemoryFrame` implement it). The
  names don't communicate that split — see the open question below.

## Part A: `cyd::touch` (rename + reshape `calibration`)

- [ ] Rename `src/cyd/calibration.rs` → `src/cyd/touch.rs` and
      `src/cyd/calibration/` → `src/cyd/touch/` per the no-`mod.rs`
      convention; inside `touch.rs`, declare `pub mod calibration;` holding
      the flow/driver/UI/constants (i.e. today's `calibration` minus the
      raw-touch types), with `driver` and `flow` nested under it.
- [ ] Move `CydRawTouch`, `RawTouchEvent`, `RawPoint` up to `cyd::touch`.
- [ ] Move `TouchEvent` (currently `src/cyd/touch_event.rs` at the root) into
      `cyd::touch`; update `CydTouch::read`'s signature path and delete the
      root re-export.
- [ ] Drop the root re-export of `EnsureCalibrationError`,
      `EnsureCalibrationOutcome`, `ensure_calibration`.
- [ ] Update all import fallout: `memory.rs`, `wasm.rs`, platform crates'
      `cyd.rs`/`cyd/touch.rs`/`error.rs`/`lib.rs`, board examples and `.j2`
      templates (`cyd_touch_paint`, armatron), linkage-blaze
      (`armatron/main.rs`, `ui.rs`, `armatron-wasm`, classic examples).

## Part B: screen constants

- [ ] Make `SCREEN_WIDTH`/`SCREEN_HEIGHT` `pub(crate)`; keep `SCREEN_PIXELS`
      public at the root.
- [ ] Replace the two public const uses in
      `linkage-blaze-example-core/src/armatron/main.rs` with
      `Orientation::Landscape.width()/height()` (already `const fn`).
- [ ] Sweep both repos for other public uses (none known beyond armatron at
      the time of writing).

## Part C: `tiling` under `display`, `CydFrame` into `display`

- [ ] Move `src/cyd/tiling.rs` → `src/cyd/display/tiling.rs`; declare
      `pub mod tiling;` from `display.rs`. `CydDisplay::tiles` returns
      `display::tiling::Tiles`.
- [ ] Move the `CydFrame` trait definition into `cyd::display` (re-exported
      from `display.rs`); `CydDisplay::Frame`'s bound path updates, root loses
      the item.
- [ ] Update all import fallout (same downstream list as Part A; `CydFrame`
      is imported nearly everywhere the device is used).

## Open questions (decide before implementing)

1. **`touch::calibration` nesting vs flat `touch`**: the layout above nests
   calibration one level down so `touch`'s index stays small. If the extra
   level feels heavy, the alternative is a flat `cyd::touch` holding
   everything calibration has today. Nesting is recommended (mirrors how
   `display` keeps its inner modules private but is itself one level down).
2. **Rename `RectanglePixels`?** The `TODO0x` in `cyd.rs` already flags the
   naming. If renamed, pick a name that says "readable, finished pixels"
   (e.g. `Rgb565PixelSource`) as opposed to `CydFrame`'s "drawable,
   in-progress frame". Could ride along with Part C or stay a TODO.
3. **Is `Cyd::parts` symmetry worth documenting at the root?** After this
   spec the root doc comment should explain the model in one paragraph:
   `Cyd` → (`CydDisplay`, `CydTouch`), display-side details in `display`,
   touch-side details in `touch`.

## Suggested order

1. Part B (smallest, no structure changes).
2. Part C (pure moves within the display side).
3. Part A (biggest rename; do last so earlier parts don't churn twice).
4. `just check-all` in both repos; `cargo doc -p device-envoy-core --no-deps
   --features host,wasm` and check the `cyd` index against the target layout.
