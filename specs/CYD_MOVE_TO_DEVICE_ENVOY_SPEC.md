<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Spec: Move CYD from linkage-blaze to device-envoy

Early plan for migrating the four CYD crates (`linkage-blaze-cyd-core`,
`linkage-blaze-cyd`, `linkage-blaze-cyd-wasm`, `linkage-blaze-cyd-memory`) into
the device-envoy workspace. The CYD abstraction was designed for this move
(cyd-core is explicitly modeled on `device-envoy-core` and already depends on
it), so the migration is mostly relocation plus one trait split.

## Decisions

### 1. 2D stays with CYD; 3D stays with linkage-blaze

`DrawItem2d`, `Image565View`, the `ContiguousPixels` rasterizer, and the
`draw_items_2d` / `prepare_draw_items_2d` trait methods move to device-envoy.
Their data models are pure 2D screen-space; the "projected disk / sphere" doc
wording is motivation, not coupling.

Three thin 3D-to-2D converters stay behind in linkage-blaze as extension code
over the moved types:

- `DrawItem3dExt::project` — extension trait on linkage's own `DrawItem3d`
  producing the moved `DrawItem2d` (orphan-rule clean).
- `ContiguousPixels::from_draw_items_3d` — becomes a linkage-blaze helper that
  projects and calls the moved `from_draw_items_2d`.
- `CydDisplay::draw_items_3d` / `prepare_draw_items_3d` — removed from the
  trait; become a linkage-blaze extension trait over `CydDisplay`.

Prerequisite: the generic pixel utilities in `linkage-blaze-core` that the 2D
layer uses move down into `device-envoy-core`, and `linkage-blaze-core` then
depends on them from there: `PixelTarget`, `PixelTargetAdapter`, `pixel_put`,
`pixel_put_565`, `fill_ellipse_pixels`, and the rgb565/rgb888 conversion
helpers (used by `cyd.rs`, `tga.rs`, and the memory and wasm backends).

#### Naming update: `DrawItem`, not `DrawItem2d`, in device-envoy

During phase 2, rename the moved 2D drawing type to `DrawItem` in
`device-envoy-core::cyd`.

`DrawItem2d` was useful while the type lived beside `DrawItem3d` in
linkage-blaze. After the move, the `2d` suffix becomes unnecessary migration
baggage. In device-envoy, CYD is a screen/touch abstraction; all drawing
items in `device_envoy_core::cyd` are screen-space drawing items by
definition. There is no 3D drawing model in device-envoy.

Use these public names in device-envoy:

- `device_envoy_core::cyd::DrawItem`
- `device_envoy_core::cyd::Image565View`
- `device_envoy_core::cyd::ContiguousPixels`

Rename the display methods accordingly:

- `CydDisplay::draw_items(...)`
- `CydDisplay::prepare_draw_items(...)`
- `ContiguousPixels::from_draw_items(...)`

Keep explicit 3D names in linkage-blaze:

- `DrawItem3d`
- `DrawItem3dExt::project(...) -> device_envoy_core::cyd::DrawItem`
- `CydDisplay3dExt::draw_items_3d(...)`
- `CydDisplay3dExt::prepare_draw_items_3d(...)`

If linkage-blaze implementation code needs local clarity, it may use a
private import alias:

```rust,ignore
use device_envoy_core::cyd::DrawItem as DrawItem2d;
```

Do not expose `DrawItem2d` as part of the device-envoy public API. Also avoid
re-exporting plain `DrawItem` at the root of `device-envoy-core`; keep it
scoped under `cyd::DrawItem`, where the name has enough context.

While working on phase 2, prefer idiomatic destination names over preserving
old linkage-blaze names. The moved CYD API should look native to
device-envoy, not like a compatibility layer. After renaming `DrawItem2d` to
`DrawItem`, update docs, examples, tests, and imports in the same phase. Do
not leave public aliases or compatibility shims unless needed temporarily
inside one patch.

Keep the linkage-blaze side explicit about projection: `DrawItem3d` stays in
linkage-blaze, and projection produces `device_envoy_core::cyd::DrawItem`.

When in doubt, use this rule: device-envoy names the screen abstraction;
linkage-blaze names the 3D-to-screen conversion.

### 2. Scope: 320x240 resistive-touch boards only

Following device-envoy's satisfied-with-90%-coverage philosophy, the
abstraction targets 320x240 ILI9341 + XPT2046 boards: the CYD family on ESP
and the Waveshare Pico-ResTouch-LCD-2.8 (same controller pair) on Pico. Panel
size and controller stay fixed; the existing 320x240 constants and tiling and
calibration math keep their current simple form.

Known first ask to defer with a TODO: the two-USB CYD variant
(ESP32-2432S028R rev 2) uses ST7789 with inverted colors. mipidsi treats the
model as a type parameter, so supporting it later is a Cargo feature or a
second constructor, not a redesign.

### 3. The name stays "CYD"

`Cyd`, `CydDisplay`, `CydTouch`, `CydFrame` move to `device-envoy-core` as-is.
Implementations follow the platform-suffix convention: `CydEsp`
(device-envoy-esp), `CydWasm` and `CydMemory` (device-envoy-core, see below),
`CydRp` (device-envoy-rp, later). Matches device-envoy's opinionated named
devices (`WifiAuto`, `Led4`).

### 4. WASM and Memory live in device-envoy-core behind features

No `device-envoy-wasm` crate. WASM support is scoped to exactly what exists
today — `CydWasm`, `ButtonWasm` / `ButtonWasmSource`, `FlashDeviceWasm`
(localStorage) — behind a new `wasm` feature on `device-envoy-core` pulling
the optional deps (`wasm-bindgen`, `web-sys`, `js-sys`, `embassy-time` wasm +
generic-queue). Module docs must state this scope explicitly (a simulation
surface for examples, not a general browser platform) so it does not accrete.

`CydMemory` goes behind the existing `host` feature, which already gates
`png` — its only std dependency. This also merges cyd-memory's host-side
flash test double with device-envoy-core's.

Caveats to encode:

- The `wasm` feature must stay strictly opt-in for leaf binaries — never a
  default or transitive feature. `embassy-time/wasm` swaps the global time
  driver and would break an MCU build if feature unification enabled it.
- Before publishing, check whether the `wasm-bindgen = "=0.2.118"` exact pin
  in cyd-wasm can relax to `0.2`; an exact pin in a published
  `device-envoy-core` is hostile to downstreams.

### 5. Open TODO0s move with the code, unchanged in priority

The migration does not require settling them first; they carry over verbatim
(moved, not deleted, per workspace convention):

- `TODO0000` software-vs-hardware clipping question in
  `linkage-blaze-cyd/src/display.rs` (`clip_to_screen`).
- `TODO00` on `CydEsp::rgb565` — likely resolved by deletion once the color
  helpers move to `device-envoy-core` (decision 1).
- cyd-memory flash-test-double consolidation — resolved by decision 4.
- The `TODO` in `cyd-wasm/src/lib.rs` anticipating a `device-envoy-wasm`
  crate — reword: the destination is `device-envoy-core` behind the `wasm`
  feature (decision 4).

## Phases

1. **Split 2D from 3D in place** (inside linkage-blaze, everything still
   compiles here): pull the 3D methods off `CydDisplay` into an extension
   trait, isolate the pixel utilities destined for `device-envoy-core`.
2. **Move traits + memory + wasm into `device-envoy-core`**: `Cyd` traits,
   tiling, calibration, orientation, TGA, `DrawItem` (renamed from
   `DrawItem2d` — see naming update under decision 1), `ContiguousPixels`,
   pixel utilities; `CydMemory` behind `host`; `CydWasm` / `ButtonWasm` /
   `FlashDeviceWasm` behind new `wasm`.
3. **Move `CydEsp` into `device-envoy-esp`**; widen the chip feature matrix
   from {esp32, esp32c6} toward the chips device-envoy-esp already supports
   (mostly Cargo-feature plumbing; verify SPI/DMA setup per chip family).
3b. **Board example templates for CYD** — done. Added
   `cyd_tiles.rs.j2` (tiled draw demo, display-only, small tile-sized buffer
   rather than a full-screen one) and `cyd_touch_paint.rs.j2` (calibration
   flow + touch-paint), generated per chip/board via `cargo xtask
   generate-board-examples`. Both are device-envoy-native with no
   linkage-blaze dependency.

   Added `CydDisplayWiring`/`CydTouchWiring` pin assignments to every board
   profile: the classic esp32 CYD board's fixed factory wiring, and spare
   SPI-capable GPIOs on the other chips (standalone module wiring).

   This surfaced two pre-existing, previously-unexercised board-data bugs
   (nothing before CYD required real dual-SPI):

   - `spi_count` was wrongly `2` for esp32c3/c5/c6/c61/h2 — esp-hal doesn't
     expose a second general-purpose SPI peripheral (`SPI3`) on those chips,
     only on classic esp32 and esp32s2/s3. Corrected to `1`; those chips now
     correctly get the "requires 2 SPI resources" placeholder for CYD.
   - esp32s2's RAM budget can't fit a full 320x240 framebuffer plus the
     Wi-Fi stack (`cyd_touch_paint` needs `CydEsp::SCREEN_PIXELS`, unlike
     `cyd_tiles`'s small tile buffer) — added a new `large_stack`
     board-example requirement token, matching the existing
     `stack_constrained` board-data field.

   Also added a `Cyd*` family of `device_envoy_esp::Error` variants,
   matching the crate's existing per-device error convention, so the new
   examples can use `device_envoy_esp::{Result, Error}` directly instead of
   a local error enum.

   device-envoy-esp's full multi-chip `check-all` passes clean.

4. **`CydRp` in `device-envoy-rp`** — done. Implements `CydRp` for a
   standalone 320x240 ILI9341 + XPT2046 module wired over SPI to a Pico
   (display on `SPI0`, touch on `SPI1`), mirroring `CydEsp`'s
   buffer/display/text/touch module structure but built on embassy-rp's
   blocking SPI instead of esp-hal. The XPT2046 touch-sampling logic ported
   verbatim, since it only depends on embedded-hal's `SpiDevice`/`InputPin`
   traits, not the platform HAL.

   Pinned `mipidsi = "=0.9.0"` here too, for the same az/embedded-graphics-core
   conflict with this crate's own `fixed` dependency found on the ESP side
   (see decision 3b above).

   Added a `Cyd*` family of `device_envoy_rp::Error` variants matching this
   crate's `derive_more::Display` convention, and a `cyd_touch_paint` example
   (calibration flow + touch-paint) mirroring the ESP one.

   device-envoy-rp's full `check-all` (embedded builds for both Pico 1 and
   Pico 2, docs, packaging verification) passes clean. The tiled-frame model
   (unchanged from `CydEsp`) is what makes 320x240 viable in Pico 1 RAM (a
   full RGB565 framebuffer is 150 KB); the Waveshare Pico-ResTouch-LCD-2.8
   uses the same ILI9341 + XPT2046 controller pair and should work
   identically, though only compile-verified — hardware verification on the
   Pico 1/2 + standalone module on hand is left to the user.

## Logistics

- All migration work happens locally: both repos live in the same VS Code
  workspace on dev branches. During development, point linkage-blaze's
  `device-envoy-*` dependencies at local paths
  (`/home/carlk/programs/mcu/device-envoy/crates/...`) instead of the
  published 0.1.0 crates. Coordinated crates.io releases happen only at the
  end, when both dev branches are ready to land.
- After the move, linkage-blaze keeps: `DrawItem3d` / `Projection` and the 3D
  extension code, the examples, and everything under `linkage-blaze-core`
  that is not a generic pixel utility.

## Handoff Notes for Implementation

- Scope of implementation is phases 1-3 only. Phase 3b (board example
  templates) and phase 4 (`CydRp`) are follow-up passes.
- Hardware on hand for testing: a standalone 320x240 ILI9341 + XPT2046
  touch-screen module (no classic-ESP32 CYD board), various non-classic ESP
  chips, and both Pico 1 and Pico 2. So the phase 3 chip-matrix widening can
  be hardware-verified on the non-classic ESPs, and the classic-esp32 path is
  verified by compile/CI only.
- **Chip-matrix widening status**: `CydEsp`'s display/touch code only uses
  chip-agnostic `esp-hal` generic trait bounds (`impl spi::master::Instance`,
  generic GPIO input/output traits) with no per-chip `cfg`, so once the code
  moved into `device-envoy-esp` (which already declares all 9 chip features)
  it compiled clean across the full matrix with zero source changes:
  `esp32`, `esp32c2`, `esp32c3`, `esp32c5`, `esp32c6`, `esp32c61`, `esp32h2`,
  `esp32s2`, `esp32s3` (compile-verified via `cargo check -p device-envoy-esp
  --no-default-features --features <chip>`, `+esp`/`-Zbuild-std=core,alloc`
  for the xtensa chips). Hardware verification (real SPI/DMA behavior on the
  non-classic ESPs on hand) is still outstanding and left to the user.
- The four `linkage-blaze-cyd*` crates are deleted at the end of phase 3 (no
  compatibility shims, per workspace convention). Dependents to re-point at
  the device-envoy locations, plus the workspace root `Cargo.toml` member and
  dependency lists:
  - `linkage-blaze-example-core`
  - `linkage-blaze-classic` (examples: clock, ballet, armatron,
    skeleton-clock)
  - `linkage-blaze-clock-wasm`, `linkage-blaze-ballet-wasm`,
    `linkage-blaze-armatron-wasm`, `linkage-blaze-skeleton-clock-wasm`
- Both repos have their own `AGENTS.md`; read each before editing that repo.
- Verification: `just check-all` in each repo is the local CI (tests, checks,
  and builds across all targets, including embedded and WASM). Run it in both
  repos after each phase; every phase must end green.
- Suggested destination layout in `device-envoy-core`: a `cyd` module
  following the existing no-`mod.rs` convention (`src/cyd.rs` +
  `src/cyd/*.rs`), with the wasm and memory backends as feature-gated
  submodules (`src/cyd/wasm.rs`, `src/cyd/memory.rs`). The esp implementation
  becomes `src/cyd.rs` + submodules in `device-envoy-esp`, mirroring how the
  other devices there are laid out.
