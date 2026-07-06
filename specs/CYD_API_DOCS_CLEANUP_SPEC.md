<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# CYD API Docs Cleanup Spec

Bring the `cyd` module docs (core, ESP, and RP) up to the standard set by
[`wifi_auto`](file://wsl$/Ubuntu/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/wifi_auto/index.html).
All code changes are in the `device-envoy` repo; downstream import fixes land in
`linkage-blaze`.

## What makes `wifi_auto` the model

The `wifi_auto` index page shows exactly four public items (`WifiAuto`,
`WifiAutoEvent`, `WifiAutoError`, `WifiStack`), because:

- The `portal` and `fields` submodules are **private**; only the two items users
  need (`FormData`, `WifiAutoField`) are re-exported.
- Cross-crate plumbing that must stay `pub` (used by the RP/ESP platform crates)
  is `#[doc(hidden)]` with a comment explaining why (`HtmlBuffer`,
  `generate_config_page`, `parse_post`, `WifiAutoBackend`, ...).
- The module doc is two sentences that point at the primary trait, and the one
  compilable example lives on that trait.

The core `cyd` index page instead shows **28 flat calibration re-exports, 9
inlined pixel-plumbing items, 3 modules, and ~15 traits/types** — roughly 55
entries for an API whose real user surface is about a dozen items.

## Phase 1 — core `cyd` module (`device-envoy-core/src/cyd.rs`)

### 1.1 Resolve the `todo000` in the module doc

Line 1 currently renders `todo000` into the published docs:

```rust
//! The opinionated CYD device traits. todo000
```

Rewrite the first line as a plain summary (mirroring `wifi_auto`'s "A device
abstraction for ..."), e.g. "An opinionated device abstraction for the 'Cheap
Yellow Display' (CYD): tiled RGB565 drawing plus calibrated touch." Resolve the
`todo000` with Carl before deleting it.

### 1.2 Collapse the 28 flat calibration re-exports to the used surface

Verified usage (device-envoy examples + all of linkage-blaze): the only
calibration items consumed outside `device-envoy-core/src/cyd/` are:

- `ensure_calibration`, `ensure_calibration_with_settings`
- `EnsureCalibrationSettings`, `EnsureCalibrationOutcome` (return type), `EnsureCalibrationError`
- `CalibrationConfig`
- `RawTouchEvent`, `RawPoint` (also referenced by the `CydRawTouch` trait)

The other 20 re-exports (`draw_calibration_*`, `calibration_corner_*`,
`CALIBRATION_*` constants, `MAX_RESIDUAL_PIXELS`, `VERIFY_HIT_RADIUS_PIXELS`,
`CalibrationCorner`, `CalibrationFlow`, `CalibrationSolveError`,
`CalibrationValidation`, `validate_calibration_points`,
`distort_demo_screen_to_raw`, `calibration_verify_target_center`) have **zero
users outside core's own cyd module** (the driver, `wasm`, and `memory`
submodules).

Apply the `wifi_auto` pattern:

- Make the module private: `mod calibration;` (folding `driver`/`flow` with it).
- Re-export only the eight items above from `cyd`.
- Internal helpers stay `pub` inside the now-private module (invisible in docs),
  since all their users are in-crate. No `#[doc(hidden)]` needed.

Before hiding each item, re-run a workspace-wide grep (both repos) to confirm
no user was missed, then `just check-all`.

### 1.3 Move the pixel plumbing out of the `cyd` page

`cyd.rs` re-exports nine generic pixel items from the private
`crate::pixel_target` module (`PixelTarget`, `PixelTargetAdapter`,
`fill_ellipse_pixels`, `pixel_put`, `pixel_put_565`, `rgb565_from_rgb888`,
`rgb565_from_rgb888_components`, `rgb565_raw_from_rgb888_components`,
`rgb888_from_rgb565`). Rustdoc inlines all of them into the `cyd` index, adding
whole `Functions` and extra `Structs`/`Traits` noise that has nothing
CYD-specific about it.

Fix:

- Make the module public at the crate root: `pub mod pixel_target;` in
  `lib.rs`, and give it a proper module doc.
- Delete the flat re-exports from `cyd.rs`. `CydFrame`'s supertrait bound
  becomes a cross-module doc link to the [`pixel_target`] module.
- Update `linkage-blaze` imports (`linkage-blaze-cyd-3d` and friends use
  `cyd::PixelTarget`, `cyd::pixel_put`, `cyd::fill_ellipse_pixels`,
  `cyd::rgb565_from_rgb888*`). No compatibility aliases, per AGENTS.md.

### 1.4 Delete the invisible `tga565` macro re-exports

`cyd.rs` line 32 re-exports `tga565`, `tga565_magenta_mask`, `tga565_mask`,
`tga565_white_mask`. The docs goal is the opposite of exposing pretty crate-root
macros: callers should use `device_envoy_core::cyd::tga565` and friends, not
top-level macros. Keep any crate-root `#[macro_export]` helpers hidden/internal
if rustc requires that plumbing, but expose the public names only from `cyd`.
For discoverability, make the `Image565Fixed` / `Image565Mask` docs link to
their matching `tga565!` / `tga565_*_mask!` macros (they are the only way to
construct these types).

### 1.5 Fill the doc gaps that render as blank rows

Every public item should have a one-line summary (these currently render with
an empty description cell somewhere):

- `RegionPixels` trait — completely undocumented; also decide whether it earns
  its `pub` slot or can shrink (its only role is `CydDisplay::flush_at`).
- `ContiguousPixels` struct summary.
- `memory` module (`#[cfg(feature = "host")]`) — no module doc at all.
- `RawPoint`, `RawTouchEvent` — show without descriptions on the ESP/RP pages.

### 1.6 Add the single compilable example on the primary trait

Per convention ("link readers to the primary type and keep a single compilable
example on that type"), `Cyd` has no example today. Add one `rust,no_run`
doctest on the `Cyd` trait mirroring the `WifiAuto::connect` example shape:
`parts()` → `frame_mut` → `write_text` → `flush().await?` → `touch.read()?`.
Boilerplate (`#![no_std]`, a test-double device) hidden with `#` lines. The
module doc keeps its narrative and links to [`Cyd`].

## Phase 2 — ESP and RP `cyd` modules (same issues, fix together)

### 2.1 Drop the renamed trait aliases (both crates)

`device-envoy-esp/src/cyd.rs:33` and `device-envoy-rp/src/cyd.rs:21` re-export:

```rust
Cyd as CydDevice, CydDisplay as CydDisplayTrait,
CydFrame as CydFrameTrait, CydTouch as CydTouchTrait,
```

so the platform doc pages list the traits under fake names (`CydDevice`,
`CydDisplayTrait`, ...) that exist nowhere else, and doc comments/examples then
propagate the fake names. There is no name collision forcing the rename
(`CydEsp` / `CydRp` are the structs). AGENTS.md: no redundant API paths, no
alias shims.

- Re-export under the real names: `Cyd, CydDisplay, CydFrame, CydTouch`.
- Mechanical rename in callers: examples import them only anonymously
  (`CydDevice as _` → `Cyd as _`, etc. in `cyd_tiles.rs` / `cyd_touch_paint.rs`
  across all chip dirs), plus doc-link fixes in `src/cyd.rs` and
  `src/cyd/text.rs` of both crates (`CydDisplayTrait::frame_mut` →
  `CydDisplay::frame_mut`).

### 2.2 Port RP's doc comments to the undocumented ESP items

The RP page documents its items; the ESP equivalents render blank. Copy/adapt
the RP one-liners to:

- `CydEsp` (the primary type — needs summary *and* keeps its usage example),
  `CalibratedCydEsp`, `CydDisplayEspPart`, `CydTouchEspPart`, `CydFrameEsp`
- `PixelBuffer`, `RegionBuffer`, `RegionView`
- `CydError`, `CydDisplayEspFlushError`, `CydDisplayEspInitError`,
  `CydTouchEspInitError`
- `DISPLAY_SPI_HZ`, `TOUCH_SPI_HZ`

Then re-check the RP page for any item ESP documents better (currently none
spotted, but verify after the alias rename).

### 2.3 Re-check the platform re-export lists after Phase 1

Both platform modules re-export `RegionPixels`, `RawPoint`, `RawTouchEvent`,
`CalibrationConfig`, `Orientation`, `TouchEvent`, `SCREEN_*`, `tiling` "as the
public surface". After Phase 1 trims core, confirm each remaining re-export is
actually used from the platform path; drop any that aren't so the two platform
pages and the core page tell one consistent story.

## Phase 3 — verification

1. In `device-envoy`: `just check-all`, then `just show-docs-core` /
   `show-docs-esp` / `show-docs-rp` and eyeball the three index pages against
   `wifi_auto`.
2. In `linkage-blaze`: `just check-all` (catches the `pixel_target` and
   calibration path moves in the WASM/example crates).
3. Update `specs/CYD_DEVICE_ENVOY_API_DOC_REVIEW_CHECKLIST.md`: its ESP/RP doc
   links point at `crates/device-envoy-{esp,rp}/target/...`, but the built docs
   actually live under the workspace target, e.g.
   `target/riscv32imac-unknown-none-elf/doc/device_envoy_esp/cyd/index.html`
   and `target/thumbv8m.main-none-eabihf/doc/device_envoy_rp/cyd/index.html`.

## Expected result

- Core `cyd` index: ~3 modules (`calibration` gone, `tiling`, `memory`,
  `wasm`), the 6 traits, ~8 curated calibration items, `Tiles`,
  `Orientation`, `TouchEvent`, `DrawItem`, `ContiguousPixels`, the three
  `Image565*` types, `CopySizeError`, and the `SCREEN_*` constants — roughly
  half the current page, every row documented, no `todo000`, no generic pixel
  functions.
- ESP/RP pages: real trait names, no blank description cells.
