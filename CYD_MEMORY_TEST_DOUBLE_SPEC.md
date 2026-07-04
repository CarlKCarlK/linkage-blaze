# CYD Memory Test-Double Spec

<!-- todo0 consider deleting this spec once the crate exists and the driver/example tests are green in CI -->

Create `linkage-blaze-cyd-memory`: an in-memory, host-only implementation of
the CYD device traits, plus an in-memory `FlashBlock`. It is a **fake** (a
small working CYD whose screen is a framebuffer, whose touch is a queue, and
whose flash is a byte vector), with a few **spy** accessors (`flush_count`,
`last_flush_region`) — not a mock. No call-order expectations: the type-state
frame API already enforces interaction rules at compile time, so tests assert
on resulting *state* (pixels, saved flash contents, returned outcomes).

## Motivation

The trait surface in `crates/linkage-blaze-cyd-core/src/cyd.rs` (`Cyd`,
`CydDisplay`, `CydFrame`, `CydTouch`, `CydRawTouch`) has two real
implementations (`CydEsp`, `CydWasm`) but no host-testable one. That leaves a
gap `TOUCH_CALIBRATION_ROBUSTNESS_SPEC.md` cannot close from inside
`CalibrationFlow`: two of its three field bugs (event-loop starvation,
sip-instead-of-drain pacing) lived in the **driver** loop
(`ensure_calibration` in `crates/linkage-blaze-cyd-core/src/calibration/driver.rs`),
which interleaves `read_raw_touch_event` with `flush()`. Only a fake device
can exercise that loop on the host. The fake also lets shared example logic
in `linkage-blaze-example-core` get smoke tests.

## Relationship to the calibration specs

`TOUCH_CALIBRATION_ROBUSTNESS_SPEC.md`, `WASM_FIDELITY_BOOT_BUTTON_SPEC.md`,
and `CALIBRATION_SAMPLING_FIX_SPEC.md` are all implemented. The driver as it
stands today (`crates/linkage-blaze-cyd-core/src/calibration/driver.rs`):

- paces itself with one full-frame flush per loop iteration;
- drains at most `MAX_RAW_EVENTS_PER_FRAME` (64) raw events per frame, and
  runs idle bookkeeping (`advance_driver_state_after_idle`) **only** on
  iterations where the raw source reported `None`;
- captures corners on release via `ReleaseTouchCapture` with a whole-press
  running mean, discarding `SAMPLES_DISCARDED_AFTER_DOWN` (4) samples after
  `Down` and requiring `MIN_SAMPLES_PER_POINT` (3) usable samples;
- validates the solve (`validate_calibration_points`) and runs a
  `Verifying` state with a center target, `VERIFY_TIMEOUT_FRAMES` idle-frame
  timeout, and `ShowCaptured`/`ShowRejected` acknowledgment states;
- takes a `device_envoy_core::button::Button` for recalibration and an
  optional `confirmed_message` drawn and flushed before save/return.

Every driver behavior listed in Phase 2 below therefore exists today and is
testable immediately — there are no conditional/deferred items. Re-check the
driver source before writing tests; these consts and state names are the
spec's snapshot, not a contract.

## How to use this document

- Each work item has two checkboxes: `impl` (written and compiles) and
  `verify` (phase gate passed AND the diff re-read against the spec text).
- **Stop at the end of each phase**, run the gate, suggest a commit message,
  and let the human review before continuing.
- Read `AGENTS.md` first. Rules that bite hardest here: never delete
  `TODO`/`todo` comments; no `mod.rs`; no lint suppression; descriptive
  variable names (no `i`/`x`); variables named after their types in
  snake_case; no builder pattern; visibility instead of `#[doc(hidden)]`;
  `rust,no_run` doctest fences.
- `linkage-blaze-cyd-core` stays `no_std` and allocation-free. The new
  memory crate is **std** and may allocate freely — it never ships to a
  device. It is consumed only as a `dev-dependency` (Cargo permits
  dev-dependency cycles, so `cyd-core` dev-depending on `cyd-memory` while
  `cyd-memory` depends on `cyd-core` is fine).

## Design

### Crate and types

- Crate: `crates/linkage-blaze-cyd-memory`, added to the workspace
  `members` list in the root `Cargo.toml`.
- Types: `MemoryCyd`, `MemoryFrame`, `MemoryFlashBlock`, `MemoryButton`,
  `MemoryCydError`.
- Dependencies: `linkage-blaze-cyd-core`, `linkage-blaze-core`,
  `device-envoy-core` (for the `FlashBlock` trait), `embedded-graphics`.
  For awaiting `flush()` in tests, use `futures` (or `futures-executor`)
  `block_on` as a dev-dependency of the *consuming* crates —
  `MemoryFrame::flush` must never actually pend, so a trivial executor is
  sufficient.

### `MemoryCyd`

- Constructed with a screen `Size` (offer `MemoryCyd::classic()` for
  320x240) plus device background/foreground colors defaulting to the real
  CYD's black/white.
- Owns a full-screen row-major `Vec<u16>` RGB565 framebuffer, initialized to
  the background color.
- Implements `CydDisplay`: `frame_mut_with_tile_top_left` hands out a
  `MemoryFrame` whose region-local buffer starts cleared to the background;
  `fill_rectangle` / `fill_contiguous` write directly into the screen
  framebuffer, clipped to the physical screen (empty intersection is a
  no-op), matching the trait's documented semantics.
- Implements `Cyd` with small borrow-splitting part structs, mirroring how
  `CydWasm` implements `parts` (`crates/linkage-blaze-cyd-wasm/src/lib.rs`).
- Implements `CydTouch` (pops a queued calibrated `TouchEvent`) and
  `CydRawTouch` (pops a queued `RawTouchEvent`), both scoped to the current
  script frame (below).

### `MemoryFrame`

Implements `CydFrame` (and its supertraits `DrawTarget<Color = Rgb565,
Error = Infallible>` and `PixelTarget`) over a region-local pixel buffer.
`flush()`:

1. blits the frame buffer into the device framebuffer at the frame's region
   top-left;
2. increments `flush_count` and records `last_flush_region`;
3. advances the input script to the next frame batch;
4. returns `Err(MemoryCydError::OutOfFrames)` once `frame_budget` is
   exceeded (see below);
5. resolves immediately — it must never return `Poll::Pending`.

### Flush is the clock (frame-scripted input)

On real hardware, `flush()` is the pacing point (SPI present on ESP,
`next_animation_frame` on WASM). The fake makes it the test clock:

- `script_raw_frames(&[&[RawTouchEvent]])` / `script_touch_frames(...)`
  load per-frame event batches. `read_raw_touch_event` / `read` pop events
  from the **current** frame's batch only, returning `Ok(None)` when the
  batch is drained; each `flush` advances to the next batch.
- This makes the "drain, don't sip" property directly expressible: put a
  full `Down`+`Up` pair in one frame batch and assert it registers within
  that iteration.
- An **empty** batch is an idle frame: the driver sees `None` and runs its
  idle bookkeeping. Scripts must include idle frames wherever the driver's
  frame counters need to tick — the `ShowCaptured` acknowledgment
  (`CAPTURE_ACK_FRAME_COUNT`), `ShowRejected` (`REJECTED_FRAME_COUNT`), and
  the verify timeout (`VERIFY_TIMEOUT_FRAMES`) all decrement only on idle
  frames. A helper like `script_idle_frames(count)` keeps tests readable.
- A batch **larger** than `MAX_RAW_EVENTS_PER_FRAME` emulates the ESP's
  direct-sampling probe, which returns an event on every call while
  pressed: the driver hits its per-frame cap, flushes anyway, and must
  *not* run idle bookkeeping. Events beyond the cap must remain queued for
  the next frame (do not drop the remainder on flush).
- Also offer plain `push_raw_touch_event(...)` / `push_touch_event(...)`
  appenders onto the current frame for simple tests.
- Because `MIN_SAMPLES_PER_POINT` and `SAMPLES_DISCARDED_AFTER_DOWN` gate
  capture, a "tap" helper that scripts `Down` + enough `Move` samples + `Up`
  at a point (`script_tap(raw_point)`) avoids every test hand-counting
  samples against the flow's thresholds.

### `MemoryButton`

The driver takes `recalibration_button: &mut impl Button`
(`device_envoy_core::button::Button`). Provide a `MemoryButton` whose
pressed state a test sets directly (or schedules by frame index, sharing the
flush clock, if a test needs a mid-flow press). Implementing the trait means
implementing the `__ButtonMonitor` supertrait: `is_pressed_raw` returns the
scripted state and `wait_until_pressed_state` can resolve immediately — the
calibration driver only ever calls the synchronous `is_pressed()`. There is
a `ButtonMock` doctest in device-envoy's `button.rs` showing the minimal
shape.

### Fuel: `frame_budget`

`ensure_calibration` loops forever by design; a test whose script never
completes the flow must fail red, not hang CI. `MemoryCyd` carries a
`frame_budget: usize` (default e.g. 1000, settable). When `flush_count`
would exceed it, `flush` returns `Err(MemoryCydError::OutOfFrames)`.
`MemoryCydError` implements `CydFlushError` and `Debug` so it propagates
through generic drivers and prints usefully on `unwrap` in tests. Do not
panic inside the fake; return the error.

### Spy accessors (state inspection)

- `flush_count() -> usize`, `last_flush_region() -> Option<Rectangle>`.
- `pixel(x, y) -> Rgb565` (panicking on out-of-bounds is fine here — this
  is host test code, and fail-fast beats silent clamping per `AGENTS.md`).
- `framebuffer() -> &[u16]` for bulk assertions.
- `write_framebuffer_tga(path)` debug helper that encodes the framebuffer
  with a small local TGA writer (or reuses cyd-core's TGA support if its
  API fits) so a failing rendering test can dump what the screen looked
  like. Keep this behind an explicit call, not automatic-on-failure
  machinery.

Keep the fake **dumb**. No timing simulation, no partial-flush emulation,
no configurable failure injection beyond the frame budget. Complexity in a
test double is where fake-induced false confidence comes from; anything
fancier belongs in the test, not the fake.

### `MemoryFlashBlock`

Implements `device_envoy_core::flash_block::FlashBlock` over an in-memory
byte store: `load` returns `Ok(None)` when empty or when the stored bytes
do not deserialize as the requested type (matching the trait contract),
`save` overwrites, `clear` empties. Spy accessor: `save_count() -> usize`.
Constructors: `MemoryFlashBlock::new()`
(empty), `::with_value(&T)` (pre-loaded), and a way to preload **corrupt
bytes** for the bad-flash test. Note: `device-envoy-core` already has a
test-private `MemoryFlashDevice` in its `flash_block.rs` tests — if the
public `FlashDevice`-plumbing route is cleaner than implementing
`FlashBlock` directly, take it; otherwise implement `FlashBlock` directly
and leave a `TODO` pointing at the possible consolidation.

## Phase 1 — The crate

- [ ] impl / [ ] verify — **Crate skeleton.** `crates/linkage-blaze-cyd-memory`
  with `MemoryCyd`, `MemoryFrame`, `MemoryCydError`, `MemoryFlashBlock` as
  designed above; workspace member; `just check-all` covers it.

- [ ] impl / [ ] verify — **Self-tests of the fake** (in the memory crate).
  Keep these minimal — they exist to make the fake trustworthy, not clever:

  - a fresh frame is cleared to the background color;
  - drawing a pixel via `DrawTarget` then flushing puts that color at the
    right screen coordinate; `last_flush_region` matches the frame region;
  - `fill_rectangle` clips at screen edges; fully off-screen is a no-op;
  - scripted raw frames: reads drain the current batch to `None`; flush
    advances to the next batch; `flush_count` increments;
  - exceeding `frame_budget` returns `Err(MemoryCydError::OutOfFrames)`;
  - `MemoryFlashBlock` round-trips a value; corrupt bytes load as
    `Ok(None)`; `clear` empties.

- [ ] impl / [ ] verify — **Replace the hand-rolled `TestCyd`/`TestFrame`**
  in the `#[cfg(test)]` module of
  `crates/linkage-blaze-cyd-core/src/cyd.rs` with `MemoryCyd` via a
  dev-dependency, so the repo has one test double, not two (`AGENTS.md`:
  refactor aggressively, no parallel shims). If the dev-dependency cycle
  proves troublesome in practice, keep `TestCyd` and record why in a
  comment plus a `TODO`.

### Phase 1 gate

`just check-all` passes. The memory crate's own tests pass under plain
`cargo test -p linkage-blaze-cyd-memory`.

## Phase 2 — Calibration driver tests

All in `crates/linkage-blaze-cyd-core`, driving the real
`ensure_calibration` with `MemoryCyd` + `MemoryFlashBlock` + `MemoryButton`
as dev-dependencies. Put them in `#[cfg(test)]` unit-test modules (per repo
convention) rather than `tests/` integration tests, so they can reference
the driver's private consts (`CAPTURE_ACK_FRAME_COUNT`,
`VERIFY_TIMEOUT_FRAMES`, `MAX_RAW_EVENTS_PER_FRAME`, …) instead of
duplicating their values. These tests are the point of the whole exercise.

- [ ] impl / [ ] verify — **Happy path.** Script one clean tap-and-release
  per corner (each with enough usable samples to clear
  `MIN_SAMPLES_PER_POINT` after the post-`Down` discards), idle frames
  between corners so the `ShowCaptured` acknowledgment expires, then a
  verify tap at the screen-center target → returns `Saved`; the flash block
  deserializes to a `CalibrationConfig`; mapping each scripted raw corner
  point through the config lands within a small tolerance of its target
  cross center. Choose scripted raw points via a known synthetic mapping
  (as the WASM distortion does) so the expected config is predictable.

- [ ] impl / [ ] verify — **Preloaded flash.** A valid pre-saved config →
  returns `Loaded` with that config, `flush_count() == 0`, and no touch
  events consumed.

- [ ] impl / [ ] verify — **Corrupt flash reruns calibration.** Preloaded
  garbage bytes → the flow runs and overwrites the block (the driver's
  documented "don't brick boot" promise).

- [ ] impl / [ ] verify — **Pacing.** With empty event batches, the driver
  flushes once per loop iteration: after running under a small
  `frame_budget`, it fails with `OutOfFrames` (not a hang) and
  `flush_count` equals the budget — proving there is no non-flushing idle
  path left.

- [ ] impl / [ ] verify — **Drain, don't sip.** A complete tap
  (`Down`/samples/`Up`) inside a single frame batch registers the corner in
  that same iteration rather than one event per frame.

- [ ] impl / [ ] verify — **Drain cap under a held stylus.** A frame batch
  larger than `MAX_RAW_EVENTS_PER_FRAME` (the ESP direct-sampling case:
  events on every read while pressed) → the driver still flushes that
  iteration (no frozen screen), the leftover events are consumed on later
  frames, and — because the queue never reported idle — no `ShowCaptured` /
  `ShowRejected` / verify-timeout counters ticked during the hold.

- [ ] impl / [ ] verify — **Dropout regression, end-to-end.** The field bug
  from `TOUCH_CALIBRATION_ROBUSTNESS_SPEC.md`: hold on corner 2 with a
  mid-hold spurious `Up` + `Down`, then release → corner 3 is **not**
  captured from corner-2 coordinates. This duplicates a flow-level unit
  test on purpose: it proves the *driver* wiring preserves the property.

- [ ] impl / [ ] verify — **Lift-off transient regression, end-to-end.**
  The field bug from `CALIBRATION_SAMPLING_FIX_SPEC.md`: a long press of
  many stable samples followed by a few heavily drifted lift-off samples →
  the captured point stays within ~1 raw unit of the stable point (the
  whole-press mean, not a last-N window).

- [ ] impl / [ ] verify — **Recalibration button.** A `MemoryButton`
  pressed for one frame mid-flow → the flow restarts from corner 1 and
  still completes correctly afterwards.

- [ ] impl / [ ] verify — **Rendering spot-checks.** After the first frame,
  the pixel at corner 1's cross center is the foreground color and a
  far-away pixel is the background. Assert individual meaningful pixels,
  not full-buffer snapshots — snapshot-exact tests break on every cosmetic
  tweak and teach people to regenerate them blindly.

- [ ] impl / [ ] verify — **Rejected solve restarts.** Contradictory
  duplicate-corner taps → the flow enters `ShowRejected` and restarts from
  corner 1; nothing is saved; a subsequent honest script still completes.

- [ ] impl / [ ] verify — **Verify miss restarts.** Four good corners, then
  a verify tap outside `VERIFY_HIT_RADIUS_PIXELS` of the center → candidate
  discarded, flow restarts, nothing saved.

- [ ] impl / [ ] verify — **Verify timeout restarts.** Four good corners,
  then `VERIFY_TIMEOUT_FRAMES` idle frames with no tap → candidate
  discarded, flow restarts, nothing saved. (This is the test that makes
  reading the private const worthwhile.)

- [ ] impl / [ ] verify — **Save exactly once, confirmation first.** Across
  any completing script, `MemoryFlashBlock` observes exactly one `save`,
  and when `confirmed_message` is `Some`, the last flushed frame before
  `ensure_calibration` returns shows the message (spot-check a pixel or,
  simpler, assert `last_flush_region` is the full screen and the flush
  count advanced after the verify tap).

### Phase 2 gate

`just check-all` passes. Human reviews the test list against the field
reports in `TOUCH_CALIBRATION_ROBUSTNESS_SPEC.md` and
`CALIBRATION_SAMPLING_FIX_SPEC.md` and confirms each field bug has a
corresponding driver-level test.

## Phase 3 — Shared example smoke tests

- [ ] impl / [ ] verify — **Ballet smoke test.** In
  `linkage-blaze-example-core` (dev-dependency on the memory crate), run the
  ballet loop for a bounded number of frames against `MemoryCyd`: no error,
  `flush_count` advanced, and every `last_flush_region` observed lies within
  the screen bounds. If the loop shape does not currently allow "run N
  frames then stop", add the smallest hook that allows it — prefer the
  frame-budget error as the natural stop signal over new parameters.

- [ ] impl / [ ] verify — **One state-based interaction test.** Pick one
  touch-driven behavior in example/armatron core logic (for example a
  button hit region) and test it: script a calibrated `TouchEvent`, run a
  frame, assert the expected state change or pixel change. This is the
  template test that shows future contributors the intended pattern.

### Phase 3 gate

`just check-all` passes. Human skims the new tests and confirms they read as
state-based ("given these inputs, the screen/flash/outcome is X") with no
call-order assertions.

## Out of scope

- Visual/interactive simulation (windowed preview, timing emulation) — the
  WASM build already serves that purpose.
- Mock-style expectations (call counts/order on draw methods). The
  type-state API plus state assertions make these redundant and brittle.
- Failure injection for display/flash errors beyond `OutOfFrames`. Add only
  when a real test needs it.
- Redesigning the `Cyd` trait surface. If implementing the fake exposes a
  trait wart (for example the `todo0000000000` note on `frame_mut` region
  semantics in `cyd.rs`), leave the existing `todo` in place and note the
  finding — do not fix it in this spec.
- Pico or any new hardware backends.
