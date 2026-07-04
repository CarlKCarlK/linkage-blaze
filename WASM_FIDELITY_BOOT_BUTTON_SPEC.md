# WASM Fidelity and Boot-Button Spec

<!-- todo0 consider deleting this spec once all phases are implemented and verified on hardware and in the browser -->

Supersedes `BOOT_BUTTON_RECALIBRATION_SPEC.md` (removed; never implemented).
Follow-up to `TOUCH_CALIBRATION_SPEC.md` and
`TOUCH_CALIBRATION_ROBUSTNESS_SPEC.md`.

Two goals, one theme — make the WASM simulator faithfully model the ESP32
device, and give both platforms the same physical escape hatch:

1. **Persistent simulated flash.** The WASM calibration "flash block"
   currently lives in memory, pre-seeded each page load. Replace it with a
   `localStorage`-backed implementation of device-envoy's `FlashBlock`
   protocol, and let armatron-wasm run the real first-boot calibration flow,
   exactly like the hardware.
2. **A boot button.** All four CYD WASM apps get a small rectangular `boot`
   button below the case image, representing the physical BOOT button on the
   back of the device. Holding it (~1 s) during play clears the stored
   calibration and restarts the device — the escape hatch for a saved-but-bad
   calibration, identical in code and behavior to holding GPIO0 on the real
   board.

**Scope:** the `just run-armatron-classic` example
(`crates/linkage-blaze-classic/examples/armatron.rs`) and the four CYD WASM
apps: `linkage-blaze-armatron-wasm`, `linkage-blaze-classic-wasm` (ballet),
`linkage-blaze-clock-wasm`, `linkage-blaze-skeleton-clock-wasm`. The
`linkage-blaze-armatron-classic` and `linkage-blaze-armatron-c6` binaries
still carry stale pre-refactor calibration copies; leave them completely
untouched.
<!-- todo0 decide whether the armatron-classic and armatron-c6 binaries should be rewired onto the shared driver or deleted -->

## Decisions already made by the human (do not relitigate)

- **No boot-time button check, on any platform.** The press-RST-then-hold-
  BOOT gesture is fiddly and flirts with the GPIO0 strapping pin. The
  in-loop hold is the one and only physical trigger.
- **`localStorage`, not cookies.** Cookies travel with HTTP requests and
  expire; `localStorage` is the browser's local key-value store and is the
  honest analog of on-board flash.
- **Armatron-wasm starts uncalibrated on first visit** — even though
  calibrating a mouse is objectively silly. Faithful simulation of the ESP32
  boot experience is the point (and the synthetic raw distortion makes the
  flow non-trivial). Reloads skip the flow once a calibration is stored.
- **WASM "reset" = in-place device restart**: drop the app state and re-run
  the boot sequence (`ensure_calibration` → app) within the same page
  session. No `location.reload()`.
- **`ButtonWasm` implements the full device-envoy `Button` trait**, not just
  `is_pressed()`, so WASM and ESP are API-identical.
- **The three non-armatron apps get the button present but inert**: it
  renders and is pressable, but nothing is wired to it yet. (Future: wipe
  stored WiFi credentials for the clock apps.)

## Field report driving the escape hatch

A miscalibration was saved on the classic CYD; touch was so far off that the
on-screen `cal` button could not be hit, and recovery required
`espflash erase-flash`. Root cause of the trap: nothing polls the physical
button during the game loop, and the shared driver only consults its
`recalibration_requested` closure *inside* the four-tap flow — a
saved-but-bad calibration is permanent. Once armatron-wasm persists real
calibrations (Phase 1), the same trap exists in the browser, so the escape
hatch lands on both platforms.

## How to use this document

- Each work item has two checkboxes: `impl` (written and compiles) and
  `verify` (phase gate passed AND the diff re-read against the spec text).
- **Stop at the end of each phase**, run the gate, suggest a commit message,
  and let the human test before continuing.
- Read `AGENTS.md` first. Rules that bite hardest here: never delete
  `TODO`/`todo` comments (move them; append `(may no longer apply)` if
  stale); no `.unwrap()`/`.expect()` in MCU app paths; keep cyd-core `no_std`
  and allocation-free; no `mod.rs`; no compatibility shims; descriptive
  variable names; numeric colors get an approximate-color-name comment.

## Phase 1 — Persistent simulated flash and first-boot calibration

- [ ] impl / [ ] verify — **`FlashDevice` over `localStorage`.** In
  `linkage-blaze-cyd-wasm`, implement device-envoy's `FlashDevice`
  (read/write/erase on a byte range) over a fixed-size in-memory byte array
  mirrored to a `localStorage` key (hex or base64 encoded; erase = fill with
  the erased-byte value the shared protocol expects). Then reuse the shared
  block protocol (`save_block`/`load_block` — the `#[doc(hidden)]`
  cross-crate plumbing in `device_envoy_core::flash_block`, which exists for
  exactly this purpose) to provide a `FlashBlockWasm` implementing
  `FlashBlock`. This keeps magic/type-hash/CRC framing byte-identical to the
  ESP, so corruption and absence behave the same. If the plumbing helpers
  genuinely do not fit, a direct `FlashBlock` impl over `localStorage` is an
  acceptable fallback — say so in the code and here.
  Key naming: one `localStorage` key per block, prefixed per app (e.g.
  `linkage-blaze.armatron.calibration`) so the four apps never collide.

- [ ] impl / [ ] verify — **Delete the pre-seeded calibration.** Remove
  `CydWasmCalibrationFlashBlock::new_precalibrated()` (and the type itself
  if `FlashBlockWasm` fully replaces it — no shims). Armatron-wasm passes a
  `FlashBlockWasm` to the existing `ensure_calibration` driver: first visit
  → four-cross flow (through the synthetic distortion) → verify target →
  saved to `localStorage`; subsequent loads boot straight into the app.
  Keep the Phase 1 solver unit test's distortion-derived config available to
  *tests* if they use it; the app itself no longer seeds.

- [ ] impl / [ ] verify — **`cal` button respects the stored block.** The
  existing `TickOut::Calibrate` path becomes: `clear()` the
  `FlashBlockWasm`, then perform the in-place device restart (drop app
  state, re-enter the boot sequence). The restart must re-read the block —
  cleared storage → calibration flow, exactly like the ESP reset path.

### Phase 1 gate

`just check-all` passes. Human: open armatron-wasm in a fresh browser
profile (or after clearing site data) → calibration flow runs → calibrate →
app works → **reload the page** → no flow, still calibrated. Press `cal` →
flow runs again. DevTools shows the `localStorage` key appearing and
clearing.

## Phase 2 — `ButtonWasm` and the on-page boot button

- [ ] impl / [ ] verify — **`ButtonWasm` in `linkage-blaze-cyd-wasm`.**
  Pressed state lives in an `Rc<Cell<bool>>` shared with a JS-facing handle
  (same pattern as `CydTouchWasmSource`). Implement
  `device_envoy_core::button::__ButtonMonitor` (`is_pressed_raw` reads the
  cell; `wait_until_pressed_state` polls the cell at
  `BUTTON_POLL_INTERVAL` via embassy-time) and the `Button` marker impl, so
  the debounce/`wait_for_press`/`PressDuration` defaults come from
  device-envoy unchanged. Enable embassy-time's wasm support for the
  `wasm32` target (its JS-timer driver feature) in the cyd-wasm crate.
  Leave a `TODO` noting `ButtonWasm` could migrate to a future
  `device-envoy-wasm` crate.

- [ ] impl / [ ] verify — **The HTML button, all four apps.** In each app's
  `www/index.html` (armatron, ballet/classic, clock, skeleton-clock): a
  small rectangular `<button class="boot-button">boot</button>` placed below
  the `.case` image, styled to read as a small physical tactile switch
  (consistent CSS across the four; lowercase `boot` label; subtle
  pressed/active state so a hold is visibly engaged). It must not shift the
  case/canvas layout the human already calibrated
  (`.case` fit values are load-bearing).

- [ ] impl / [ ] verify — **Wire pointer events → `ButtonWasm`** in
  armatron-wasm only: `pointerdown` sets pressed, `pointerup`/
  `pointercancel`/`pointerleave` clear it (mirror `install_touch_handlers`).
  The other three apps render the button but attach nothing; add one comment
  in each app's HTML (or JS) saying the button is intentionally inert
  pending its reset behavior.

### Phase 2 gate

`just check-all` passes. All four apps show the boot button below the case
with identical styling; armatron's visibly depresses and (via a temporary
log or the Phase 3 behavior if already merged) registers press/release; the
other three are visually present and inert.

## Phase 3 — Hold-to-recalibrate on both platforms

- [ ] impl / [ ] verify — **Sans-io hold detector in cyd-core.** A small
  `HoldToRecalibrate` (name flexible) struct in
  `linkage_blaze_cyd_core::calibration`: polled once per frame with
  `(button_is_pressed: bool, now_milliseconds: u64)`, returns whether
  `RECALIBRATE_HOLD_MILLISECONDS` (suggest 1000) of continuous hold has been
  reached. Latches after firing (no repeat-fire while still held); release
  before the threshold resets it. Plain-milliseconds input keeps it
  `no_std`, clock-free, and unit-testable. Tests: fires at threshold;
  bounce/short press does not fire; no double-fire while held.

- [ ] impl / [ ] verify — **ESP wiring**
  (`crates/linkage-blaze-classic/examples/armatron.rs`). Each frame, feed
  `calibration_button.is_pressed()` and the loop's existing time source. On
  fire: clear the calibration flash block, draw a simple "release button to
  recalibrate" frame, **wait for a debounced release**, then
  `esp_hal::system::software_reset()`. The release-wait comment must state
  the reason: GPIO0 is a strapping pin, and resetting while it is held drops
  the ESP32 ROM into the serial-download bootloader. Converge this and the
  on-screen `TickOut::Calibrate` handler on one shared clear-and-reset
  helper in the example.

- [ ] impl / [ ] verify — **WASM wiring** (armatron-wasm). Same detector,
  fed by `ButtonWasm::is_pressed()` and the app's time source. On fire:
  clear `FlashBlockWasm`, show the same "release button" frame, wait for
  release (no strapping pin in a browser, but keeping the sequence identical
  keeps the simulation and the article story honest — say so in a comment),
  then the in-place device restart from Phase 1. Result: hold the on-page
  boot button ~1 s → device "reboots" into the calibration flow.

### Phase 3 gate

`just check-all` passes.

- ESP (classic CYD): save a deliberately bad calibration (sloppy taps that
  survive verification, or temporarily relax it) → touch unusable → hold
  BOOT ~1 s → "release button" frame → release → reset → calibration flow →
  honest recalibration → app works. Short presses and bumps during play do
  nothing. The device does not enter the serial bootloader when the button
  is released promptly.
- WASM: same script with the on-page button, including the
  reload-persistence check from Phase 1 afterward.

## Out of scope

- Wiring the boot button in ballet/clock/skeleton-clock (future: WiFi
  credential reset for the clocks; the button and `ButtonWasm` are ready).
- The `linkage-blaze-armatron-classic` and `linkage-blaze-armatron-c6`
  binaries (stale pre-refactor calibration copies; untouched).
- Any boot-time button check (explicitly dropped by decision, both
  platforms).
- Pico implementation (detector and driver are platform-neutral; only
  wiring would remain).

## Note for the human (not a work item)

Until Phase 3 lands, a stuck ESP device is recovered with
`espflash erase-flash` + reflash; a stuck browser session (after Phase 1) is
recovered by deleting the app's `localStorage` key in DevTools.
