# Privatize `ContiguousPixels`, Consolidate Core Errors, and Move Display Plumbing into `cyd::display`

<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

Three related API cleanups for `device-envoy-core`'s `cyd` surface, plus the
downstream updates in `linkage-blaze`. This spec spans **two repos**:

- `/home/carlk/programs/mcu/device-envoy` (crates `device-envoy-core`, `device-envoy-esp`, `device-envoy-rp`)
- `/home/carlk/programs/linkage-blaze` (crates `linkage-blaze-cyd-3d`, `linkage-blaze-example-core`, and anything else that fails to compile)

Verification for every phase: `just check-all` in **both** repos must pass.
Follow all rules in each repo's `AGENTS.md` (no `#[allow]`, no deleted TODOs,
`Fixed`/`View` naming, no `mod.rs`, etc.).

## Part 1: Make `ContiguousPixels` private

### Motivation for Part 1

`ContiguousPixels` appears on the public docs page of `device_envoy_core::cyd`,
but no downstream code needs the type — every real use compiles draw items and
immediately streams them via `fill_contiguous`. The type is public only because:

1. `CydDisplay::prepare_draw_items` returns it (zero external callers — only
   `draw_items` calls it internally).
2. `CydDisplay::draw_items` takes `&[DrawItem]` (a slice), while the one real
   consumer (`clock.rs` in linkage-blaze) has an *iterator* of draw items, so it
   bypasses `draw_items` and constructs `ContiguousPixels` by hand.

Fix the second point and the first becomes deletable, per the "avoid redundant
API paths" rule.

### Changes in `device-envoy-core` (`src/cyd.rs`, `src/cyd/contiguous_pixels.rs`)

- [ ] Change `CydDisplay::draw_items` to take `impl IntoIterator<Item = DrawItem>`
      instead of `items: &[DrawItem]`. Keep the const generic; rename its
      parameter from `PRIMITIVE_COUNT` to `PIXEL_SOURCE_COUNT` to match
      `ContiguousPixels` (a background bitmap counts as a pixel source but not a
      primitive).
- [ ] Move the screen-bounds intersection (`bounds.intersection(...)` currently
      in `prepare_draw_items`) into `draw_items` so behavior is unchanged.
- [ ] Delete `CydDisplay::prepare_draw_items`.
- [ ] Change `pub use contiguous_pixels::ContiguousPixels;` so the type is no
      longer re-exported publicly. Make `ContiguousPixels`,
      `ContiguousPixelsIter`, and their methods `pub(crate)` (the default body of
      `draw_items` still uses them — a private type may appear in a default
      method *body*, just not in the trait's public signature).
- [ ] If demoting visibility leaves methods dead (`is_empty`, `pixel_at`, …),
      delete the dead methods rather than suppressing warnings. Keep any unit
      tests that still compile against the `pub(crate)` type; keep all TODO
      comments (move them if needed).

### Changes in `linkage-blaze-cyd-3d` (`src/lib.rs`)

- [ ] Delete the free function `contiguous_pixels_from_draw_items_3d` and the
      trait method `CydDisplay3dExt::prepare_draw_items_3d` (their only caller
      is `draw_items_3d` itself).
- [ ] Reimplement `CydDisplay3dExt::draw_items_3d` as: project the 3D items to
      2D (`.map(|draw_item_3d| draw_item_3d.project(projection))`), then call
      `self.draw_items::<PIXEL_SOURCE_COUNT>(bounds, background, projected)`.
- [ ] Drop the now-unused `ContiguousPixels` import.

### Changes in `linkage-blaze-example-core` (`src/clock.rs`)

- [ ] Replace the manual `ContiguousPixels::<{ 1 + LINKAGE.draw_item_3d_count() }>::from_draw_items(...)`
      + `display.fill_contiguous(...)` pair (around lines 110–121) with a single
      call:

```rust,ignore
display
    .draw_items::<{ 1 + LINKAGE.draw_item_3d_count() }>(
        CLOCK_BOUNDS,
        background,
        iter::once(CLOCK_BACKGROUND_BITMAP).chain(draw_items_2d),
    )
    .map_err(Error::Flush)?;
```

- [ ] Update the nearby explanatory comment: the point (row-major streaming with
      no frame/tile buffer, background bitmap as first pixel source) still
      holds, but it now happens inside `draw_items`.
- [ ] Fix any other compile breaks from the removed re-export across the
      workspace (search both repos for `ContiguousPixels` and
      `prepare_draw_items`).

## Part 2: Consolidate core error types into `Error`

### Motivation for Part 2

`device-envoy-core` already has a unified, `#[non_exhaustive]` `Error` enum in
`src/error.rs` (with a `Result` alias), yet several small crate-owned error
types live beside it. Fold the foldable ones in so downstream error enums need
one `From<device_envoy_core::Error>` instead of one variant per little type.

### Decision table

**Fold into `Error` as variants** (each returned directly by a core-owned
public API; none are generic):

| Current type | New `Error` variants |
| --- | --- |
| `cyd::CopySizeError` (struct) | `CopySize { src_len: usize, frame_len: usize }` |
| `lcd_text::LcdTextError` | `LcdI2cWrite { address: u8 }`, `LcdRowOutOfBounds { row: usize }` |
| `cyd::calibration::CalibrationSolveError` | `CalibrationDegenerateGeometry`, `CalibrationResidualTooLarge { worst_residual_pixels: f32 }` |
| `wifi_auto::WifiAutoError` | `WifiAutoFormat`, `WifiAutoStorageCorrupted`, `WifiAutoMissingCustomField` |

**Delete outright:**

- `cyd::CydInfallibleError` — it is an empty never-type, so it does *not*
  belong inside a fallible `Error` enum. Replace it with
  `core::convert::Infallible` plus `impl CydFlushError for Infallible {}` in
  place at the time (the trait was later renamed to `CydIoError` and then removed by `CYD_IO_ERROR_REMOVAL_SPEC.md`) in
  `src/cyd.rs`. Update all `type Error = CydInfallibleError` impls (`src/wasm.rs`
  has several) and every doctest that names it (`src/cyd.rs`,
  `src/cyd/calibration/driver.rs`, `src/memory.rs` if present).

**Keep as-is (do not fold):**

- `CydFlushError` (trait) — it is the *bound* on each device's associated
  error type in this spec snapshot; the trait was later renamed to `CydIoError`
  and then removed by `CYD_IO_ERROR_REMOVAL_SPEC.md`.
  `type Error`, the mechanism that lets platform crates (`device-envoy-esp`,
  `device-envoy-rp`) carry their own device errors and lets downstream generic
  code distinguish flush errors from local errors (see `ballet::Error` in
  `linkage-blaze-example-core`, the canonical example referenced by AGENTS.md).
  A marker trait cannot be an enum variant.
- Per-device associated errors: `memory::MemoryCydError`,
  `wasm::FlashDeviceWasmError`, and the platform crates' `CydError`s — the
  whole point of the associated-type design is that these stay per-device.
- Generic wrappers: `EnsureCalibrationError<DeviceError, FlashError>`,
  `FlashBlockError<E>`, `Led4SimpleLoopError<E>` — `Error` is non-generic and
  cannot absorb them.
- `led4::Led4BitsToIndexesError` — `#[doc(hidden)]` platform plumbing consumed
  only by `Led4SimpleLoopError<E>`; not on the public docs page, leave it.

### Implementation notes

- [ ] Add the variants above to `Error` in `src/error.rs`, with doc comments.
      Match each variant's `#[cfg(feature = ...)]` gating to the module it
      serves, exactly like the existing `TaskSpawn` variant (e.g. the wifi-auto
      variants gate on the same feature as `wifi_auto`; check each module's
      gating in `src/lib.rs` before writing the cfgs).
- [ ] Change the producing APIs to return `crate::Error` (prefer the crate's
      `Result<T>` alias): `CydFrame::copy_from_565`, `Image565Fixed::copy_to`
      (`src/cyd/tga.rs`), the calibration solve path
      (`src/cyd/calibration.rs`), the lcd-text path (`src/lcd_text.rs` and its
      platform users), and the wifi-auto paths (`src/wifi_auto.rs`,
      `src/wifi_auto/fields.rs`).
- [ ] Delete the folded type definitions. Do **not** keep type aliases or
      re-exports for compatibility (AGENTS.md: no backwards-compatibility
      shims).
- [ ] The old `CopySizeError` derived `Clone, Copy, PartialEq, Eq`; `Error`
      derives only `Debug`. Rewrite any `assert_eq!` on these errors as
      `matches!` / pattern asserts instead of adding derives to `Error`, unless
      a derive is genuinely needed everywhere.
- [ ] Update `copy_from_565` implementors: `src/memory.rs`, `src/wasm.rs`,
      `device-envoy-esp/src/cyd.rs`, `device-envoy-rp/src/cyd.rs`, plus
      doctest doubles in `src/cyd.rs` and `src/cyd/calibration/driver.rs`.
- [ ] Check `device-envoy-rp/src/error.rs` and `device-envoy-esp` error enums:
      where they wrapped a folded type (e.g. `LcdTextError`, `WifiAutoError`),
      wrap `device_envoy_core::Error` once instead, with a derived `From` so
      plain `?` works (AGENTS.md error-propagation rules).
- [ ] Downstream `linkage-blaze-example-core/src/ballet.rs`: replace the
      `CopySize(CopySizeError)` variant with `Core(device_envoy_core::Error)`
      (derived `From`), and update the module-level comment that explains the
      `CydFlushError` coherence pattern — the pattern itself is unchanged, and
      the trait was later renamed to `CydIoError` and then removed by `CYD_IO_ERROR_REMOVAL_SPEC.md`.
- [ ] Update the wifi-auto examples under `device-envoy-esp/examples/*/` that
      name `WifiAutoError`.
- [ ] Sweep both repos for remaining references to the deleted names
      (`CydInfallibleError`, `CopySizeError`, `LcdTextError`,
      `CalibrationSolveError`, `WifiAutoError`) including doc links.

## Part 3: Move display-only plumbing into a `cyd::display` submodule

### Motivation for Part 3

`device_envoy_core::cyd` documents itself as the *device* abstraction
(display **and** touch), yet most of its index is display-only data and asset
plumbing flattened to the root: the four `tga565*` macros, `DrawItem`,
`Image565Fixed`/`Image565View`/`Image565Mask`, `Orientation`,
`RectanglePixels`, and `Tiles`. The earlier `CYD_MODULE_SURFACE_SPEC.md`
(device-envoy repo) already set the precedent with `cyd::calibration`:
implementor/plumbing items live one level down in a public submodule; the
`cyd` root shows only the device model. Apply the same treatment to the
display side.

Organizing rule: the `cyd` root keeps the device model — the part traits, the
error/marker plumbing shared by both parts, the screen constants, and the
everyday entry points. Data, asset, and drawing types move to `cyd::display`.

### Target layout

`cyd` root after this part:

- Traits: `Cyd`, `CydDisplay`, `CydTouch`, `CydFrame`, `CydFlushError` (later renamed to `CydIoError`, then removed by `CYD_IO_ERROR_REMOVAL_SPEC.md`)
- `TouchEvent` (return type of `CydTouch::read`)
- `SCREEN_WIDTH`, `SCREEN_HEIGHT`, `SCREEN_PIXELS`
- Calibration re-exports: `EnsureCalibrationError`, `EnsureCalibrationOutcome`,
  `ensure_calibration`
- Public submodules: `calibration`, `display`, `tiling`

New public `cyd::display` submodule:

- `DrawItem`, `Image565View` (from `cyd/draw_item.rs`)
- `Image565Fixed`, `Image565Mask` (from `cyd/tga.rs`)
- `Orientation` (from `cyd/orientation.rs`)
- `RectanglePixels` (currently defined in `cyd.rs`)
- The macro re-exports `tga565`, `tga565_mask`, `tga565_magenta_mask`,
  `tga565_white_mask` (move the `pub use crate::{__cyd_tga565 as tga565, ...}`
  lines here)
- `pub(crate) ContiguousPixels` (from Part 1) also lives under this submodule

Move `Tiles` (currently defined in `cyd.rs`) into the existing `tiling`
submodule as `cyd::tiling::Tiles`, next to the `TileGrid` it iterates.

### Implementation notes for Part 3

- [ ] Restructure files per the no-`mod.rs` convention: `src/cyd/display.rs`
      declaring `mod draw_item; mod tga; mod orientation; mod contiguous_pixels;`
      with the module files moved to `src/cyd/display/*.rs`, and a short module
      doc comment linking `DrawItem` as the primary type (workspace docs
      convention). Inner modules stay private; items are re-exported from
      `display.rs`.
- [ ] Trait methods at the root may keep referencing submodule types in their
      signatures (`CydDisplay::draw_items` takes `display::DrawItem`,
      `flush_at` takes `impl display::RectanglePixels`, `tiles` returns
      `tiling::Tiles`); only the *definition/re-export location* changes.
- [ ] No compatibility re-exports at the `cyd` root for moved items
      (AGENTS.md: no shims). Update all import fallout instead. Known users to
      re-grep (list from the previous surface spec still applies):
  - `device-envoy-core`: `src/memory.rs`, `src/wasm.rs`, `src/to_png.rs` (if
    it names `Orientation`/image types), doctests in `src/cyd.rs` and
    `src/cyd/calibration/driver.rs`
  - `device-envoy-esp` and `device-envoy-rp`: `src/cyd.rs`,
    `src/cyd/display.rs`, `src/cyd/buffer.rs`, `src/cyd/touch.rs`, board
    examples under `examples/**`, and the `.j2` example templates
    (`examples/templates/`) — templates must change in lockstep or
    regeneration reintroduces old paths
  - `linkage-blaze`: `linkage-blaze-cyd-3d`, `linkage-blaze-example-core`
    (`clock.rs`, `ballet.rs`), `linkage-blaze-armatron-wasm`,
    `linkage-blaze-classic` examples
- [ ] The `tga565*` macros are `#[macro_export]` and thus also exist at the
      crate root as `__cyd_tga565` etc.; that is the existing documented
      macro-helper exception (`__` prefix + `#[doc(hidden)]`) and stays as-is —
      only the friendly re-export moves from `cyd` to `cyd::display`.
- [ ] Update the module doc at the top of `cyd.rs` if it references items that
      moved, and confirm intra-doc links still resolve (`cargo doc`).
- [ ] After the move, rebuild docs and confirm the `cyd` index matches the
      "Target layout" list above.

## Suggested order

1. Part 1 entirely (both repos compile).
2. Part 2 core-side (`device-envoy-core` + platform crates compile).
3. Part 2 downstream (`linkage-blaze` compiles).
4. Part 3 (pure moves/renames, easiest once the surface is already smaller).
5. `just check-all` in both repos; `cargo doc -p device-envoy-core` and check
   the `cyd` index page.
