# Touch-Calibration Robustness Spec

<!-- todo0 consider deleting this spec once all phases are implemented and verified on hardware and WASM -->

Follow-up to `TOUCH_CALIBRATION_SPEC.md` (all three phases implemented: the
shared flow lives in `linkage_blaze_cyd_core::calibration` with a sans-io
`CalibrationFlow`, a generic `ensure_calibration` driver, and WASM support
with a synthetic raw distortion). Field testing found one blocking bug per
platform and a UX gap. This spec fixes them and brings the flow up to
resistive-touch best practice.

## Field report (what the human observed)

- **ESP32 classic:** taps on crosses 1 and 2 register, but cross 3 often
  "flashes by" — especially when the stylus lingers on cross 2 — and the
  resulting calibration is badly wrong. The screen then flashes black and
  briefly re-shows the 4th cross while the device reboots.
- **WASM:** the calibration screen draws, but clicks on the first cross are
  ignored; the flow is stuck forever.

## Root causes (verified by code reading — do not re-diagnose, fix)

1. **The flow registers a corner on every raw `Down`**
   (`crates/linkage-blaze-cyd-core/src/calibration/flow.rs`,
   `handle_raw_touch_event`). On a resistive panel the XPT2046 pressure/IRQ
   signal drops out momentarily while the stylus is held, so
   `TouchProbe::read_raw_touch_event`
   (`crates/linkage-blaze-cyd/src/touch.rs`) emits `Up` then a fresh `Down`
   at the same physical spot. That second `Down` is recorded as the *next*
   corner: corner 3 silently receives corner 2's raw coordinates and the
   affine solve is garbage. The determinant assert does not catch this — the
   system is still solvable, just wrong.

2. **The driver starves the browser event loop on WASM**
   (`crates/linkage-blaze-cyd-core/src/calibration/driver.rs`). When no touch
   event is pending it awaits `embassy_futures::yield_now()`, which under
   `wasm_bindgen_futures` schedules a **microtask**. A loop that only ever
   yields via microtasks never returns control to the browser's event loop,
   so `pointerdown` handlers never run and
   `CydTouchWasmSource`'s queue stays empty forever. The armatron app itself
   is immune because its loop awaits `flush()`, and `CydWasm::flush`
   awaits `next_animation_frame()` — a real macrotask yield
   (`crates/linkage-blaze-cyd-wasm/src/lib.rs`).

3. **No feedback at completion.** After the 4th tap the driver saves and the
   binary immediately software-resets with the 4th cross still displayed —
   the black flash and ghost cross the human saw. Cosmetic, but it reads as
   a crash.

## Design direction

Register calibration points on **pen release**, averaged from samples
collected while pressed; validate the solve against its own inputs; then make
the user prove the calibration works by hitting a center **done** target
before anything is saved. Pace the driver loop the same immediate-mode way as
every other loop in this codebase: draw + flush every iteration. These are
standard practice for resistive-touch calibrators (TI app notes, the uGFX and
LVGL calibrator screens all do capture-on-release + verify-with-retry).

## How to use this document

- Each work item has two checkboxes: `impl` (written and compiles) and
  `verify` (phase gate passed AND the diff re-read against the spec text).
- **Stop at the end of each phase**, run the gate, suggest a commit message,
  and let the human test before continuing.
- Read `AGENTS.md` first. Rules that bite hardest here: never delete
  `TODO`/`todo` comments (move them; append `(may no longer apply)` if
  stale); no `.unwrap()`/`.expect()` in MCU app paths; keep cyd-core `no_std`
  and allocation-free; descriptive variable names; numeric colors get an
  approximate-color-name comment; no `mod.rs`.
- `CalibrationFlow` must stay **sans-io**: no clocks, no awaits, no drawing.
  Anything time-like is expressed in sample/poll counts fed by the driver.

## Phase 1 — Unblock WASM: pace the driver with draw + flush

- [ ] impl / [ ] verify — **Flush every iteration.** Rework
  `ensure_calibration` in
  `crates/linkage-blaze-cyd-core/src/calibration/driver.rs` to draw the
  calibration frame and await `flush()` on every loop iteration, deleting the
  `redraw_requested` flag and the `yield_now()` idle path (and the
  `embassy-futures` dependency if nothing else uses it). `flush()` is the
  platform's natural pacing point: `next_animation_frame()` on WASM (which
  lets pointer events fire — this alone fixes the stuck WASM screen) and the
  SPI present on ESP. This also matches the immediate-mode loop shape used
  everywhere else in the repo.

- [ ] impl / [ ] verify — **Drain, don't sip.** With one flush per frame, the
  driver must process **all** queued raw events each iteration (loop on
  `read_raw_touch_event` until `None`) before drawing, so a fast tap's
  `Down`/`Up` pair queued between frames on WASM is not consumed one event
  per animation frame.

### Phase 1 gate

`just check-all` passes. Human runs `just run-armatron-wasm`, presses `cal`,
and confirms clicks on the crosses now register (completion quality is
Phase 2's problem). ESP behavior unchanged or better.

## Phase 2 — Capture on release with sample averaging

All logic in this phase goes into `CalibrationFlow`
(`crates/linkage-blaze-cyd-core/src/calibration/flow.rs`) so it is shared and
unit-testable. Suggested per-corner state machine:

- **Armed** — waiting for `Down`. A stray `Up` is ignored.
- **Sampling** — on `Down` and subsequent `Move`s, accumulate raw samples
  into fixed arrays (heapless). Discard the first few samples after `Down`
  (the touch-down transient is the noisiest part of a resistive read);
  suggested consts: `SAMPLES_DISCARDED_AFTER_DOWN = 2`,
  `SAMPLE_CAPACITY = 16` (once full, keep the most recent — a ring or simple
  shift is fine).
- **On `Up`** — if at least `MIN_SAMPLES_PER_POINT` (suggest 3) usable
  samples were kept, register the corner as the **average** (or median) of
  the kept samples and advance to the next corner in the **Armed** state.
  Fewer samples = the touch was a bounce or graze: discard it and stay on
  the same corner.

Work items:

- [ ] impl / [ ] verify — **Rework `CalibrationFlow` to the state machine
  above.** Key property: a noise dropout while the stylus is held (spurious
  `Up` quickly followed by `Down` near the same spot) can at worst register
  the *current* corner from its own samples — it can never donate
  coordinates to the *next* corner, because the next corner re-arms and
  requires its own full `Down`→samples→`Up` cycle. Holding the stylus
  indefinitely on one cross registers nothing until release.

- [ ] impl / [ ] verify — **`CalibrationFlowEvent` gains progress feedback.**
  Emit enough for the driver to show per-corner acknowledgment (e.g. the
  captured corner briefly drawn in a "hit" color before the next cross).
  Keep the existing `PointCaptured`/`Completed` shape as the base.

- [ ] impl / [ ] verify — **Unit tests in cyd-core** feeding synthetic event
  sequences:
  - clean tap per corner → completes with the expected averaged points;
  - held stylus with a mid-hold `Up`/`Down` dropout on corner 2 → corner 3
    is **not** captured from corner-2 coordinates (the regression that
    motivated this spec);
  - graze (`Down` then `Up` with < `MIN_SAMPLES_PER_POINT`) → corner not
    advanced;
  - `Move`-only noise while Armed → ignored.

- [ ] impl / [ ] verify — **ESP probe sanity pass.** Read
  `crates/linkage-blaze-cyd/src/touch.rs` `read_raw_xy` sampling; if it
  averages raw ADC reads including obvious outliers, note it, but do **not**
  redesign it in this spec — the flow-level averaging above is the fix of
  record. Leave a `TODO` if median-of-N at the probe level looks worthwhile.

### Phase 2 gate

`just check-all` passes. Human recalibrates the classic CYD: lingering on a
cross must not skip the next one; each cross should take exactly one
deliberate tap-and-lift.

## Phase 3 — Validate the solve, then verify with a done target

- [ ] impl / [ ] verify — **Self-consistency validation.** After
  `from_four_points`, map each captured raw point through the solved config
  and compute its distance to its target cross center. If any residual
  exceeds `MAX_RESIDUAL_PIXELS` (suggest 12), the data was self-contradictory:
  the flow reports a `Rejected` event (with the worst residual, for logging)
  and restarts from corner 1. The driver shows a brief "try again" frame.
  Also replace the bare determinant `assert!` in the solver with a fallible
  path feeding the same rejection — a degenerate tap set must never panic a
  device. This validation is pure math: implement and unit-test it in
  cyd-core (contradictory duplicate-corner input → rejected; clean input →
  accepted).

- [ ] impl / [ ] verify — **Verification screen ("done" target).** After a
  config passes validation, the driver draws a target (reuse the cross
  drawing) in the **screen center** plus a short instruction line, and maps
  incoming raw touch through the **candidate** config. A registered tap
  (same release semantics as Phase 2) within `VERIFY_HIT_RADIUS_PIXELS`
  (suggest 20) of the center → save to flash and return `Saved`. A tap
  outside the radius, or no tap within `VERIFY_TIMEOUT_POLLS` frames
  (suggest ~10 seconds' worth), → discard the candidate and restart the
  whole flow. This is the human's "done button": a calibration so bad the
  center is unhittable can never be saved, which is precisely the situation
  the physical-button escape hatch was for.

- [ ] impl / [ ] verify — **Completion frame before reset.** On save, draw a
  simple "calibrated" confirmation frame and flush it before returning, so
  the software reset no longer looks like a crash (the black flash + ghost
  4th cross from the field report). Keep it to roughly one second worth of
  frames; no new timer machinery.

- [ ] impl / [ ] verify — **WASM parity check.** The verification screen and
  retry loop run unmodified in the browser (they live in the shared driver);
  confirm the `cal` button → calibrate → verify → resume path works with the
  synthetic distortion, and that intentionally sloppy cross clicks get
  rejected and re-prompted rather than saved.

### Phase 3 gate

`just check-all` passes. Human on the classic CYD: (a) deliberately tap one
cross twice in the same place → flow rejects and restarts rather than saving
a bad config; (b) honest calibration → center target → tap → confirmation →
reboot → armatron touch is accurate; (c) same script on
`just run-armatron-wasm`.

## Out of scope (unchanged from the parent spec)

- Boot button / startup grace window (GPIO0 wiring stays exactly as is).
- Pico implementation.
- `localStorage` persistence for WASM.
- Redesigning the XPT2046 probe's ADC sampling (flow-level averaging is the
  fix of record; at most leave a `TODO`).
