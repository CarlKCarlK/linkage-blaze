# Calibration Sampling Fix Spec

<!-- todo0 consider deleting this spec once all phases are implemented and verified on hardware and in the browser -->

Follow-up to `TOUCH_CALIBRATION_ROBUSTNESS_SPEC.md` and
`WASM_FIDELITY_BOOT_BUTTON_SPEC.md` (both implemented). Hardware testing on
the ESP32 classic CYD found the calibration mostly working but off in three
ways, all traced to specific defects during code review. This spec fixes
them, plus one API cleanup (closure → device-envoy `Button`).

## Field report (what the human observed on the classic CYD)

1. Two crosses are visible at once for a moment.
2. Even careful calibrations sometimes fail validation and restart.
3. Calibrations that pass leave a systematic offset: the blue touch cursor
   lands farther from the center of the screen than the physical stylus.

## Root causes (verified by code reading — fix, do not re-diagnose)

1. **The averaged samples are the lift-off transient.** The flow keeps a
   most-recent-16 ring buffer (`store_usable_sample` in
   `crates/linkage-blaze-cyd-core/src/calibration/flow.rs`) and captures on
   release. Meanwhile the driver's inner drain loop
   (`crates/linkage-blaze-cyd-core/src/calibration/driver.rs`, the
   `loop { read_raw_touch_event... }`) is unthrottled, and on the ESP
   `read_raw_touch_event` is a direct ADC sample that returns an event on
   **every** call while pressed. A normal press therefore yields thousands
   of samples, and the 16 that survive the ring all come from the final
   milliseconds before pen-up — the lift-off transient, the noisiest and
   most biased data a resistive panel produces (raw coordinates drift as
   contact pressure collapses). This causes both symptom 2 (drifted points
   randomly exceed the `MAX_RESIDUAL_PIXELS` gate) and symptom 3 (a solve
   fitted to systematically displaced points). Corner-center targets are
   single-sourced between drawing and solving — geometry mismatch was ruled
   out.

2. **Consts drifted from the robustness spec.**
   `SAMPLES_DISCARDED_AFTER_DOWN = 1` (spec suggested 2) and
   `MIN_SAMPLES_PER_POINT = 1` (spec suggested 3) — at 1, a single-sample
   graze registers a corner.

3. **The drain loop starves redraw while pressed.** Because the ESP read
   never returns `None` while the stylus is down, the driver cannot break
   out to draw or tick `frames_remaining` counters until lift. It also makes
   the sampling rate unbounded, which is what turned the ring buffer into a
   lift-off-only window.

4. **`ShowCaptured` draws two crosses at once** (symptom 1) — the captured
   corner's acknowledgment cross *and* the next corner's cross, for
   `CAPTURE_ACK_FRAME_COUNT` frames. On a calibration screen this invites
   tapping the wrong target.

5. **`VERIFY_TIMEOUT_POLLS = 600`** decrements once per *drawn frame*; at
   ESP full-frame SPI flush rates this is far more than the intended ~10
   seconds.

## How to use this document

- Each work item has two checkboxes: `impl` (written and compiles) and
  `verify` (phase gate passed AND the diff re-read against the spec text).
- **Stop at the end of each phase**, run the gate, suggest a commit message,
  and let the human test before continuing.
- Read `AGENTS.md` first. Rules that bite hardest here: never delete
  `TODO`/`todo` comments (move them; append `(may no longer apply)` if
  stale); no `.unwrap()`/`.expect()` in MCU app paths; keep cyd-core `no_std`
  and allocation-free; no compatibility shims; descriptive variable names.
- `CalibrationFlow` stays **sans-io**: counts, not clocks.

## Phase 1 — Average the whole press, not the last 16 samples

- [ ] impl / [ ] verify — **Replace the ring buffer with a running mean.**
  In `flow.rs`, `ReleaseTouchCaptureState::Sampling` keeps
  `sum_x: u64, sum_y: u64, usable_sample_count: usize` instead of the
  `[RawPoint; SAMPLE_CAPACITY]` array (delete `store_usable_sample` and
  `SAMPLE_CAPACITY`). Samples accumulate for the **entire** press after the
  initial discard, so the mean is dominated by the long stable middle of
  the press and the handful of lift-off samples become a negligible tail.
  `u64` sums cannot overflow at any realistic sample rate; state a brief
  worst-case argument in a comment (raw is at most 4095 ≈ 2^12; u64 allows
  > 2^50 samples).

- [ ] impl / [ ] verify — **Restore the spec'd thresholds.**
  `SAMPLES_DISCARDED_AFTER_DOWN = 4` (the touch-down transient; the higher
  value is fine now that sampling is plentiful) and
  `MIN_SAMPLES_PER_POINT = 3`. A graze or bounce discards the attempt and
  stays on the same corner (existing behavior, now actually reachable).

- [ ] impl / [ ] verify — **Update and extend the flow unit tests.**
  Existing tests change from last-16 averages to whole-press averages. Add
  the regression test this spec exists for: a long press of many stable
  samples (say 1000 at one point) followed by a few drifted lift-off
  samples (say 5, drifted by hundreds of raw units) must capture within ~1
  raw unit of the stable point.

### Phase 1 gate

`just check-all` passes. Human recalibrates the classic CYD: careful
calibrations should now pass validation consistently, and the post-save
cursor offset (symptom 3) should be visibly gone — the verify target and the
in-app cursor land under the stylus.

## Phase 2 — Bound the drain loop and fix the timeout

- [ ] impl / [ ] verify — **Cap events drained per frame.** In `driver.rs`,
  the inner drain loop processes at most `MAX_RAW_EVENTS_PER_FRAME`
  (suggest 64) events before falling through to draw. Note in a comment why:
  direct-sampling platforms (ESP) never return `None` while pressed, and the
  screen must keep drawing during a press. The idle bookkeeping
  (`advance_driver_state_after_idle`) must still run **only** when the queue
  is actually idle (`None` seen), not when the cap is hit — holding the
  stylus must not tick `ShowCaptured`/`Confirming` frame counters as if
  idle. Restructure the loop however reads best; do not contort to keep the
  current shape.

- [ ] impl / [ ] verify — **Make the verify timeout mean seconds.** Replace
  `VERIFY_TIMEOUT_POLLS = 600` with a value derived from an explicit,
  commented frames-per-second assumption (or measure nothing and simply
  document "N drawn frames ≈ 10 s at the ESP's observed ~X fps" with the
  const adjusted to match). The timeout only needs to be roughly right; it
  must not be 60+ seconds.

### Phase 2 gate

`just check-all` passes. Human: while holding the stylus down on a cross,
the instruction text/screen still repaints (no frozen frame); the verify
screen times out in roughly ten seconds when ignored.

## Phase 3 — One cross at a time

- [ ] impl / [ ] verify — **`ShowCaptured` draws only the next cross.**
  Acknowledge the capture without a second cross: keep the "Corner
  captured" instruction text, optionally with a small filled dot at the
  captured corner (a dot cannot be mistaken for a tap target). Delete
  `draw_calibration_captured_cross` if nothing else uses it. Keep the
  acknowledgment brief (`CAPTURE_ACK_FRAME_COUNT` can stay).

### Phase 3 gate

`just check-all` passes. Human: during calibration exactly one cross is ever
visible; the flow reads as tap → brief ack → next cross. Same check on
`just run-armatron-wasm`.

## Phase 4 — `recalibration_requested` closure becomes a device-envoy `Button`

Decision (human-approved): use the plain **`Button`** trait, not
`ButtonWatch`. Every use here is a per-frame synchronous `is_pressed()`
poll; `ButtonWatch` exists to keep press detection alive when *futures* are
cancelled by fast loops — nothing here awaits button futures — and
`ButtonWatch` is ESP-only (requires an embassy `Spawner`), which would fork
the two platforms. Record this rationale in a comment at the driver
parameter so `ButtonWatch` is not "helpfully" swapped in later.

- [ ] impl / [ ] verify — **Driver takes the button.** Change
  `ensure_calibration`'s `recalibration_requested: R` closure parameter to
  `recalibration_button: &mut impl Button`
  (`device_envoy_core::button::Button`), reading
  `recalibration_button.is_pressed()` where the closure was called. Update
  both callers: the ESP example passes its existing `ButtonEsp`
  (GPIO0); armatron-wasm passes its existing `ButtonWasm` — which also
  means the physical/on-page button can now restart the flow *during*
  calibration on WASM, matching the ESP.

- [ ] impl / [ ] verify — **Sweep the seam.** If the game-loop
  hold-to-recalibrate wiring in either app still routes button state
  through closures or ad-hoc flags where a `&mut impl Button` (or the
  shared hold detector fed by `is_pressed()`) would do, converge them.
  While in `crates/linkage-blaze-example-core/src/armatron/main.rs`, leave
  the `todo000000` about `ArmatronOutcome`-as-`Error` in place (not this
  spec's call).

### Phase 4 gate

`just check-all` passes. ESP: pressing BOOT during the four-cross flow
restarts the flow (existing behavior, now via the button parameter). WASM:
holding the on-page boot button during the flow restarts it too. Both apps'
game-loop hold-to-recalibrate still works.

## Out of scope

- Redesigning the XPT2046 probe's per-read ADC sampling
  (`read_raw_xy`); the whole-press mean supersedes the earlier `TODO`
  about probe-level medians — append `(may no longer apply)` to it rather
  than deleting it.
- The `linkage-blaze-armatron-classic` and `linkage-blaze-armatron-c6`
  binaries (stale pre-refactor calibration copies; untouched).
- Wiring the boot button in ballet/clock/skeleton-clock (still inert by
  decision).
- Pico implementation.
