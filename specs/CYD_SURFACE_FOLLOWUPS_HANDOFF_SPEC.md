<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# CYD Surface Follow-Ups: Handoff Plan

This is a handoff companion to
[`CYD_SURFACE_FOLLOWUPS_SPEC.md`](./CYD_SURFACE_FOLLOWUPS_SPEC.md).

Use this when the agent session can only write to one repo at a time. The
current tool behavior appears to scope each session to a single writable repo,
so the work should be split into:

1. a `device-envoy` session that performs the API refactor there
2. a `linkage-blaze` session that updates downstream usage after the API is stable

## Scope split

### Session A: `/home/carlk/programs/mcu/device-envoy`

Do all CYD API and module-surface changes here first.

Primary crates:

- `crates/device-envoy-core`
- `crates/device-envoy-esp`
- `crates/device-envoy-rp`

Required outcomes:

- `device_envoy_core::cyd` root keeps only:
  - `Cyd`, `CydDisplay`, `CydTouch`, `CydFlushError`
  - `SCREEN_PIXELS`
  - `display`
  - `touch`
- `SCREEN_WIDTH` and `SCREEN_HEIGHT` become `pub(crate)`
- `CydFrame` moves to `device_envoy_core::cyd::display::CydFrame`
- `TouchEvent` moves to `device_envoy_core::cyd::touch::TouchEvent`
- raw-touch types move to `device_envoy_core::cyd::touch`
  - `CydRawTouch`
  - `RawTouchEvent`
  - `RawPoint`
- calibration flow moves under `device_envoy_core::cyd::touch::calibration`
- `tiling` moves to `device_envoy_core::cyd::display::tiling`
- root re-exports are removed for:
  - `ensure_calibration`
  - `EnsureCalibrationError`
  - `EnsureCalibrationOutcome`

Files known to need attention:

- `crates/device-envoy-core/src/cyd.rs`
- `crates/device-envoy-core/src/cyd/display.rs`
- `crates/device-envoy-core/src/cyd/tiling.rs`
- `crates/device-envoy-core/src/cyd/touch_event.rs`
- `crates/device-envoy-core/src/cyd/calibration.rs`
- `crates/device-envoy-core/src/cyd/calibration/driver.rs`
- `crates/device-envoy-core/src/cyd/calibration/flow.rs`
- `crates/device-envoy-core/src/memory.rs`
- `crates/device-envoy-core/src/wasm.rs`
- `crates/device-envoy-esp/src/cyd.rs`
- `crates/device-envoy-esp/src/cyd/touch.rs`
- `crates/device-envoy-esp/src/lib.rs`
- `crates/device-envoy-rp/src/cyd.rs`
- `crates/device-envoy-rp/src/cyd/touch.rs`
- `crates/device-envoy-rp/src/error.rs`
- CYD examples/templates in `device-envoy-esp` and `device-envoy-rp`

### Session B: `/home/carlk/programs/linkage-blaze`

Do downstream import and constant fallout only after Session A is complete.

Primary crates:

- `crates/linkage-blaze-example-core`
- `crates/linkage-blaze-armatron-wasm`
- any wasm/example crate that fails to compile after the API move

Known downstream fallout:

- `device_envoy_core::cyd::CydFrame` imports must become
  `device_envoy_core::cyd::display::CydFrame`
- `device_envoy_core::cyd::TouchEvent` imports must become
  `device_envoy_core::cyd::touch::TouchEvent`
- `device_envoy_core::cyd::calibration::*` imports must become
  `device_envoy_core::cyd::touch::...` or
  `device_envoy_core::cyd::touch::calibration::...`
- `SCREEN_WIDTH` / `SCREEN_HEIGHT` usage in armatron must switch to
  `Orientation::Landscape.width()` / `.height()`

Known files likely to need edits:

- `crates/linkage-blaze-example-core/src/armatron/main.rs`
- `crates/linkage-blaze-example-core/src/ui.rs`
- `crates/linkage-blaze-armatron-wasm/src/lib.rs`
- `crates/linkage-blaze-clock-wasm/src/lib.rs`
- `crates/linkage-blaze-skeleton-clock-wasm/src/lib.rs`
- tests/doctests that import moved CYD items

## Current status snapshot

As of 2026-07-06:

- `dev2026may` in `device-envoy` still matched the pre-follow-up public layout
  when reviewed:
  - root `calibration`
  - root `tiling`
  - root `TouchEvent`
  - root `CydFrame`
  - public `SCREEN_WIDTH` / `SCREEN_HEIGHT`
- a partial refactor attempt was started in one session but not verified
- because the writable-root split prevented completion, treat the refactor as
  unfinished and re-check the worktree before continuing

### Linkage-blaze session progress

As of 2026-07-06 in `/home/carlk/programs/linkage-blaze`:

- downstream CYD import fallout has been updated to the intended new paths:
  - `cyd::display::CydFrame`
  - `cyd::display::tiling::*`
  - `cyd::touch::TouchEvent`
  - `cyd::touch::calibration::*`
- armatron no longer depends on public `SCREEN_WIDTH` / `SCREEN_HEIGHT`; it now
  uses `Orientation::Landscape.width()` / `.height()`
- verification in this repo is still blocked because the path dependency
  `/home/carlk/programs/mcu/device-envoy` does not currently compile after its
  partial CYD refactor

Known blocker details from `cargo check` on 2026-07-06:

- unresolved `CalibrationFlow` imports inside
  `device-envoy-core/src/cyd/touch/calibration.rs` and `touch/driver.rs`
- `device-envoy-core/src/cyd/display/tga.rs` still refers to the old root
  `CydFrame` path
- `device-envoy-core/src/cyd/touch.rs` re-export visibility is incomplete for
  `CalibrationFlow`

## Recommended handoff instructions

### For the `device-envoy` session

```text
Implement the device-envoy side of specs/CYD_SURFACE_FOLLOWUPS_SPEC.md using specs/CYD_SURFACE_FOLLOWUPS_HANDOFF_SPEC.md as the execution plan. Work only in /home/carlk/programs/mcu/device-envoy. Do not touch linkage-blaze. Finish the CYD API refactor, run the relevant checks in device-envoy, and then summarize the exact linkage-blaze fallout paths/import changes.
```

### For the `linkage-blaze` session

```text
Apply the linkage-blaze fallout from specs/CYD_SURFACE_FOLLOWUPS_SPEC.md after the device-envoy CYD API refactor is complete. Work only in /home/carlk/programs/linkage-blaze. Update imports and armatron's screen-size usage, then run linkage-blaze checks.
```

## Verification split

### Session A verification

Run in `device-envoy`:

- targeted `cargo check` / `cargo test` for CYD-touch/display fallout
- `cargo doc -p device-envoy-core --no-deps --features host,wasm`
- ideally `just check-all` if the session budget allows

### Session B verification

Run in `linkage-blaze`:

- targeted `cargo check` for the affected crates first
- then `just check-all`

## Notes for the implementing agent

- Do not add compatibility re-exports just to preserve old paths.
- Prefer finishing the `device-envoy` API move completely before touching
  `linkage-blaze`.
- If `device-envoy` still has a dirty partial refactor in the worktree, review
  it carefully instead of discarding it blindly.
