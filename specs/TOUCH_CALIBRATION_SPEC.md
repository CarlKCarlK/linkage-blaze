# Touch-Calibration Consolidation Spec

<!-- todo0 consider deleting this spec once all phases are implemented, verified, and the article draft no longer needs it -->

Touch calibration for the CYD currently exists as three near-copies: the flow
state machine and screen drawing are duplicated in
`crates/linkage-blaze-armatron-classic/src/main.rs` and
`crates/linkage-blaze-classic/examples/armatron.rs`, the crosshair helpers sit
"temporarily" in `crates/linkage-blaze-example-core/src/armatron/calibration.rs`
(whose own doc comment says it belongs in the device layer), and the affine
math lives in `crates/linkage-blaze-cyd/src/calibration.rs` with the
corner-center geometry duplicated between the two `calibration.rs` files.

This spec consolidates all of it into one generic flow in
`linkage-blaze-cyd-core`, rewires the ESP32 classic and C6 armatron binaries
onto it, and adds a WASM calibration demo. It is written to support a Medium
article on good embedded code patterns, so the shape of the code (sans-io
state machine, capability traits, single source of truth) matters as much as
the behavior.

## Decisions already made by the human (do not relitigate)

- The generic flow lives in `linkage-blaze-cyd-core`, generic over the
  existing `CydDisplay`/`CydTouch` trait family plus the
  `device_envoy_core::flash_block::FlashBlock` trait for persistence.
  `device-envoy-rp` already implements `FlashBlock`, so a future Pico port
  needs no new storage design.
- Restart model: on hardware, after a calibration is saved or cleared, the
  binary performs a **software reset** (`esp_hal` software reset). The app
  never handles a "calibration changed mid-session" state; it always boots
  with a valid calibration or into the calibration flow. On WASM, "reset"
  is an in-place restart of the app loop (drop and reconstruct the app
  state); no page reload.
- **Boot button and startup grace window are out of scope.** Do not touch the
  existing GPIO0 `calibration_button` / `recalibration_requested()` wiring in
  either binary. It stays exactly as it is today. (Future work: a generic
  boot-time reset-request mechanism shared with WiFi-credential reset for the
  clock apps; see Out of Scope.)
- WASM starts **calibrated**. The WASM touch layer applies a deliberate fixed
  affine distortion to pointer coordinates (simulating a resistive panel's
  raw output), and the startup calibration is derived by solving against that
  distortion — so touch works immediately. Pressing the on-screen `cal`
  button clears it and runs the real shared calibration flow in the browser.

## How to use this document

- Each work item has two checkboxes:
  - `impl` — check when the change is written and compiles.
  - `verify` — check only after the phase gate passes AND the diff for that
    item has been re-read against its spec text.
- **Stop at the end of each phase.** Run the phase gate, check the `verify`
  boxes, suggest a commit message, and let the human test before the next
  phase begins.
- Read `AGENTS.md` in the repo root before starting. Rules that bite hardest
  here:
  - Never delete `TODO`/`todo` comments. Move them with the code they
    annotate; if one seems obsolete, append `(may no longer apply)`.
  - No `.unwrap()`/`.expect()` in MCU app paths; keep core crates `no_std`
    and allocation-free.
  - No `mod.rs` files: `src/calibration.rs` + `src/calibration/flow.rs`.
  - Descriptive variable names matching type names; no single-letter names.
  - Rust getters do not use a `get_` prefix.
  - Any numeric color definition gets a nearby approximate-color-name
    comment.

## Phase 1 — Calibration domain moves to `linkage-blaze-cyd-core`

Create `linkage_blaze_cyd_core::calibration` as the single home for
calibration types, math, drawing, and the flow state machine. Nothing in this
phase touches a platform crate's behavior; existing binaries keep compiling
against their old code until Phase 2 rewires them.

- [ ] impl / [ ] verify — **Move `RawPoint` and `CalibrationConfig`** from
  `linkage-blaze-cyd/src/calibration.rs` into
  `linkage_blaze_cyd_core::calibration`. `CalibrationConfig` keeps its serde
  derives (add a no-default-features `serde` dependency with `derive` to
  `linkage-blaze-cyd-core`; it must stay `no_std`). Convert the free function
  `map_raw_to_screen(raw_x, raw_y, config)` into a method
  `CalibrationConfig::map_raw_to_screen(&self, raw_x, raw_y)`.

- [ ] impl / [ ] verify — **Genericize the solver over screen size.** The
  current `from_four_points` hard-codes `SCREEN_WIDTH`/`SCREEN_HEIGHT` from
  `linkage-blaze-armatron-core`; `cyd-core` must not depend on an example
  crate. The panel constants already live in `cyd-core` (fixed 320x240), so
  use those directly and drop the width/height plumbing where it becomes
  redundant.

- [ ] impl / [ ] verify — **Single copy of the corner geometry.**
  `CalibrationCorner`, `calibration_corner_for_index`,
  `calibration_corner_center`, and `CALIBRATION_CROSS_MARGIN` currently exist
  in both `linkage-blaze-cyd/src/calibration.rs` and
  `linkage-blaze-example-core/src/armatron/calibration.rs`. One copy survives,
  in `linkage_blaze_cyd_core::calibration`.

- [ ] impl / [ ] verify — **Move the drawing helpers**
  (`draw_calibration_cross` and its size/color consts) from
  `linkage-blaze-example-core/src/armatron/calibration.rs` into the new
  module. When the example-core file is emptied by this, delete it and remove
  the `pub mod calibration;` from `armatron/main.rs` — but first move its
  module-level doc comment's intent ("this belongs in the device layer") into
  the new module's docs as fulfilled history, and preserve any `TODO`
  comments per `AGENTS.md`. Update the doc comment in `armatron/main.rs`
  (around lines 114–117) that references the temporary module.

- [ ] impl / [ ] verify — **Sans-io flow state machine.** Add
  `CalibrationFlow`: it owns the "collect four corner taps" logic that is
  duplicated today in the two binaries (index, collected `RawPoint`s,
  restart-on-request, and completion). It performs no I/O itself: the caller
  feeds it raw touch events and it reports what corner to draw next and, when
  finished, hands back a computed `CalibrationConfig` via
  `CalibrationConfig::from_four_points`. Preserve today's tap semantics
  (register a point per touch with the same debounce/release behavior the
  binaries use now). The platform loop remains responsible for reading raw
  touch, drawing (using the moved helpers), logging, and persistence.

- [ ] impl / [ ] verify — **Solver unit test in cyd-core.** Pick a
  non-trivial synthetic affine distortion (scale + offset + a small skew;
  name the constants), push the four corner-center points through it to get
  fake raw points, run `from_four_points`, and assert the resulting config
  maps each fake raw point back to its corner center within a small epsilon.
  This test doubles as the documented basis for the Phase 3 WASM distortion.

### Phase 1 gate

`just check-all` passes. No behavior change on any platform; the old copies
still exist and are still what the binaries call.

## Phase 2 — Raw-touch capability, generic driver, ESP rewiring

- [ ] impl / [ ] verify — **`CydRawTouch` trait in cyd-core.** The existing
  `CydTouch::read` returns calibrated screen-space events, which the
  calibration flow cannot use. Add a small trait exposing raw touch events
  (the `RawTouchEvent` down/move/up shape already in
  `linkage-blaze-cyd/src/touch.rs`, moved or mirrored into cyd-core alongside
  `RawPoint`). Implement it for `CydEsp`. Keep visibility tight: raw touch is
  for calibration, not for apps.

- [ ] impl / [ ] verify — **Generic calibration driver in cyd-core.** One
  function (or small struct) that the platform binaries call:
  roughly `ensure_calibration(display, raw_touch, flash_block)` — if the
  `FlashBlock` holds a valid `CalibrationConfig`, return it; otherwise run
  `CalibrationFlow` (clear screen, draw crosses, collect taps), save the
  result to the `FlashBlock`, and return a marker telling the caller a new
  calibration was saved so the caller performs its platform reset. The driver
  itself performs **no reset** — reset stays a three-line platform concern in
  each binary (a reset trait was considered and rejected as needless
  abstraction). Garbage or absent flash content must read as `None`
  ("not calibrated"), never as an error that bricks boot; verify the existing
  `FlashBlock` load path already guarantees this and document it at the call
  site.

- [ ] impl / [ ] verify — **Rewire `linkage-blaze-armatron-classic`** onto the
  shared driver: delete its local `calibrate`, `draw_calibration_screen`,
  `CalibrationCorner`, corner math, and cross consts. `TickOut::Calibrate`
  becomes: clear the flash block, then software reset. The existing
  `recalibration_requested()` boot-button check stays byte-for-byte as it is.

- [ ] impl / [ ] verify — **Rewire `crates/linkage-blaze-classic/examples/armatron.rs`**
  (the `just run-armatron-classic` path) the same way, deleting its duplicate
  flow.

- [ ] impl / [ ] verify — **Rewire `linkage-blaze-armatron-c6`** the same way.

- [ ] impl / [ ] verify — **Shrink `linkage-blaze-cyd/src/calibration.rs`.**
  After the moves, it should either vanish or contain only re-exports needed
  by the crate's public API; prefer vanish (no compatibility shims, per
  `AGENTS.md`).

### Phase 2 gate

`just check-all` passes. Human flashes the classic CYD via
`just run-armatron-classic` and confirms: fresh flash (or cleared block) →
calibration flow → four taps → reset → armatron runs calibrated; pressing the
on-screen `cal` button → reset → calibration flow again. Same smoke test on
the C6 if hardware is at hand.

## Phase 3 — WASM calibration demo

The point of this phase is that a mouse is already perfectly calibrated, so a
naive port would compute the identity transform and demonstrate nothing.
Instead the WASM touch source deliberately distorts its "raw" coordinates, so
calibration is real and visible in the browser — good for testing the shared
flow end-to-end and a good interactive figure for the article.

- [ ] impl / [ ] verify — **Synthetic raw distortion in `linkage-blaze-cyd-wasm`.**
  `CydTouchWasmSource` maps pointer coordinates through the same named
  distortion constants used by the Phase 1 solver unit test before exposing
  them as raw touch. Implement `CydRawTouch` for `CydWasm` on top of this.
  Calibrated `CydTouch::read` maps raw through the active
  `CalibrationConfig`, exactly as the ESP side does.

- [ ] impl / [ ] verify — **In-memory `FlashBlock` impl for WASM.** A trivial
  session-lifetime store (no `localStorage` yet; see Out of Scope) that
  starts **pre-seeded**: at startup, push the four corner centers through the
  distortion and run `from_four_points` on the result — the app boots
  calibrated without a hand-tuned inverse constant, and the solver itself is
  the single source of truth.

- [ ] impl / [ ] verify — **`cal` button enters WASM calibration.** In
  `linkage-blaze-armatron-wasm`, `TickOut::Calibrate` clears the in-memory
  block and performs the WASM "reset": drop the app state and re-enter the
  same `ensure_calibration` → run-app sequence in place. The user then taps
  the four crosses with the mouse (through the distortion), and the app
  resumes freshly calibrated. No page reload.

- [ ] impl / [ ] verify — **Uncalibrated feel is demonstrable.** During the
  calibration flow the pointer input is raw, so if the distortion constants
  are visibly large enough, a user who clears calibration can feel clicks
  landing off-target before the flow fixes it. Confirm the chosen constants
  make this visible but still leave the four crosses hittable.

### Phase 3 gate

`just check-all` passes. Human runs `just run-armatron-wasm` and confirms:
app starts calibrated and playable; `cal` button → cross flow → four clicks →
app restarts calibrated; deliberately sloppy calibration clicks produce
visibly skewed touch, and re-running `cal` fixes it.

## Out of scope (future work, keep as notes not code)

- **Boot button / startup grace window.** Holding GPIO0 through a hardware
  reset enters the ESP32 serial bootloader (and BOOTSEL on the Pico enters
  the UF2 bootloader), so a hold-through-reset gesture is wrong on both
  families. The eventual design is a startup grace window ("hold BOOT now"
  prompt sampled briefly at app start), generalized into a boot-time
  reset-request mechanism also usable to clear stored WiFi credentials for
  the clock and skeleton-clock apps when moving to a new network. Not in this
  spec.
- **Pico implementation.** `device-envoy-rp` already provides `FlashBlock`;
  once a Pico CYD-like app exists, only `CydRawTouch` + reset wiring are
  needed. Design for it, write none of it.
- **`localStorage` persistence for WASM** so a user's browser calibration
  survives reload. Session-only is fine for the demo.
- **Calibration plausibility validation** beyond the existing determinant
  assert (e.g. rejecting a solve whose corners land wildly off-screen and
  re-running the flow).
