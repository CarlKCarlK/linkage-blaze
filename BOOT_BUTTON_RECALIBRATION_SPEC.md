# Boot-Button Recalibration Escape-Hatch Spec

<!-- todo0 consider deleting this spec once both phases are implemented and verified on hardware -->

Follow-up to `TOUCH_CALIBRATION_SPEC.md` and
`TOUCH_CALIBRATION_ROBUSTNESS_SPEC.md`. A bad calibration got saved on the
classic CYD and the on-screen `cal` button became unhittable — and it turned
out there is currently **no** escape hatch at all: the device is stuck until
its flash is erased. This spec adds the physical-button escape hatch (the
BOOT button, via the device-envoy `Button` abstraction).

**Scope:** only the `just run-armatron-classic` example
(`crates/linkage-blaze-classic/examples/armatron.rs`) and the
`just run-armatron-wasm` app (`crates/linkage-blaze-armatron-wasm`) matter.
The `linkage-blaze-armatron-classic` and `linkage-blaze-armatron-c6` binaries
still carry stale pre-refactor calibration copies; leave them completely
untouched (see Out of Scope).

## Field report (what the human observed)

After the robustness spec's Phase 2, a miscalibration was saved. Touch is so
far off that the on-screen `cal` button cannot be hit, so recalibration
cannot be triggered. Recovery required erasing flash from the host.

## Root causes (verified by code reading — fix, do not re-diagnose)

1. **The rewire dropped the boot-time button check.** The pre-refactor code
   ran `recalibration_requested()` *before* loading the stored calibration.
   The shared driver
   (`crates/linkage-blaze-cyd-core/src/calibration/driver.rs`) only polls its
   `recalibration_requested` closure *inside* the four-tap flow loop; when
   the flash block deserializes, `ensure_calibration` returns `Loaded`
   immediately and the button is never read. A saved-but-bad calibration is
   therefore permanent.

2. **No in-app escape.** Nothing polls the button during the game loop, and
   the only in-app trigger is the on-screen `cal` button — which is exactly
   the thing a bad calibration takes away.

## Design

Two independent triggers, both feeding the existing clear-block-then-reset
path, both built on `device_envoy::button::Button` (the trait already
provides debounced `is_pressed()` / `wait_for_press` semantics; the example
already constructs `ButtonEsp::new(p.GPIO0, PressedTo::Ground)`):

- **At boot:** if the button is pressed when `ensure_calibration` starts,
  clear the stored calibration and run the flow. (Gesture: press RST,
  release it, then hold BOOT while the app starts.)
- **During the game loop:** holding the button for
  `RECALIBRATE_HOLD_MILLISECONDS` (suggest 1000 — long enough that a bump or
  a curious short press does nothing) clears the stored calibration and
  software-resets into the calibration flow. App state is discarded by the
  reset, same as the on-screen `cal` button.

**Strapping-pin safety (required, not optional):** GPIO0 low during a reset
makes the ESP32 ROM enter the serial-download bootloader. Pressing BOOT while
the app is *running* is harmless, but the device must **never** issue the
software reset while the button is still held. After a trigger fires, show a
"release button to recalibrate" frame and wait for a debounced release before
resetting. Document this reason at the wait site — it looks like needless
ceremony otherwise.

## How to use this document

- Each work item has two checkboxes: `impl` (written and compiles) and
  `verify` (phase gate passed AND the diff re-read against the spec text).
- **Stop at the end of each phase**, run the gate, suggest a commit message,
  and let the human test before continuing.
- Read `AGENTS.md` first. Rules that bite hardest here: never delete
  `TODO`/`todo` comments (move them; append `(may no longer apply)` if
  stale); no `.unwrap()`/`.expect()` in MCU app paths; keep cyd-core `no_std`
  and allocation-free; no `mod.rs`; descriptive variable names.

## Phase 1 — Boot-time button check in the shared driver

- [ ] impl / [ ] verify — **Check `recalibration_requested()` before loading
  flash** in `ensure_calibration`
  (`crates/linkage-blaze-cyd-core/src/calibration/driver.rs`): if it returns
  true at entry, clear the flash block (ignore-if-absent) and run the flow
  regardless of what is stored. This restores the pre-refactor escape hatch.
  Log one line (via the existing logging pattern the driver's callers use)
  so the human can tell the button was honored.

- [ ] impl / [ ] verify — **Confirm the closures.** The ESP example passes
  `|| calibration_button.is_pressed()`; the WASM app keeps `|| false`
  (WASM cannot get stuck — its seeded calibration is always correct, so the
  on-screen `cal` button is always hittable).

### Phase 1 gate

`just check-all` passes. On the classic CYD with a deliberately bad saved
calibration: press RST, release, hold BOOT while the app starts → the
calibration flow runs instead of the broken app.

## Phase 2 — Hold-to-recalibrate in the game loop

- [ ] impl / [ ] verify — **Sans-io hold detector in cyd-core.** A small
  `HoldToRecalibrate` (name flexible) struct in
  `linkage_blaze_cyd_core::calibration`: the caller polls it once per frame
  with `(button_is_pressed: bool, now_milliseconds: u64)` and it returns
  whether the configured hold duration has been reached. It latches after
  firing (no repeat-fire while still held), and a release before the
  threshold resets it. Caller supplies time as plain milliseconds so the
  type stays `no_std`, clock-free, and unit-testable. Tests: fires at
  threshold; bounce/short press does not fire; no double-fire while held.

- [ ] impl / [ ] verify — **Wire it into the example's game loop**
  (`crates/linkage-blaze-classic/examples/armatron.rs` only). Each frame,
  feed `calibration_button.is_pressed()` and the loop's existing time
  source. On fire: clear the calibration flash block, draw a simple
  "release button to recalibrate" frame, **wait for a debounced release**
  (`Button::wait_for_press` internals show the debounce pattern; a simple
  poll-until-released-then-settle is fine), then
  `esp_hal::system::software_reset()`. The release-wait comment must state
  the GPIO0 strapping/download-mode reason.

- [ ] impl / [ ] verify — **Keep the on-screen `cal` path consistent.** The
  `TickOut::Calibrate` handler and the hold detector should converge on one
  shared "clear + reset" helper in the example rather than two slightly
  different copies.

### Phase 2 gate

`just check-all` passes. On the classic CYD:

- (a) Save a deliberately bad calibration (sloppy taps). With the app
  running and touch unusable, hold BOOT ~1 second → "release button" frame →
  release → device resets into the calibration flow. Recalibrate honestly →
  app works. This is the scenario from the field report and the reason this
  spec exists.
- (b) Short BOOT presses and bumps during play do nothing.
- (c) Confirm the device does **not** enter the serial bootloader when the
  button is released promptly after the prompt.
- (d) `just run-armatron-wasm` is unaffected: starts calibrated, `cal`
  button still round-trips through the flow.

## Out of scope

- **The `linkage-blaze-armatron-classic` and `linkage-blaze-armatron-c6`
  binaries.** They still use stale pre-refactor calibration copies; the
  human only cares about the `run-armatron-classic` example and the WASM
  app. Do not rewire, extend, or delete them here.
  <!-- todo0 decide whether the armatron-classic and armatron-c6 binaries should be rewired onto the shared driver or deleted -->
- WASM parity for the physical button (a keyboard-key `Button` impl); not
  needed since WASM cannot get stuck.
- The startup grace window / generic boot-time reset-request mechanism
  (WiFi-credential reset for the clock apps). The Phase 1 boot check here is
  deliberately minimal; the generalized mechanism remains future work.
- Pico implementation (the hold detector and driver changes are already
  platform-neutral; only wiring would remain).

## Note for the human (not a work item)

Until Phase 1 lands, a device stuck with a bad calibration can be recovered
with `espflash erase-flash` followed by reflashing via
`just run-armatron-classic` — the erased block reads as "not calibrated" and
the flow runs at boot.
