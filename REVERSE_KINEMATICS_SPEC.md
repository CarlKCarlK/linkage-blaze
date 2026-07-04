# Reverse-Kinematics Restoration Spec

<!-- todo0 Consider deleting this spec once all phases are verified and shipped. -->

Restore the armatron reverse-kinematics (RK) solver that was removed in commit
`4fbeda7` ("Remove active armatron reverse kinematics loop"), re-fitted onto
the immediate-mode UI that replaced the old touch-dispatch code. The old
implementation is readable at:

```text
git show 4fbeda7^:crates/linkage-blaze-example-core/src/armatron/main.rs
```

The parked module `crates/linkage-blaze-example-core/src/armatron/reverse_kinematics.rs`
lists what to restore; this spec is the concrete plan. When the work is done,
that module's "parked notes" doc comment is replaced by real documentation of
the live solver.

This spec is written to be executed by any capable agent (Claude, Codex,
etc.), one phase per session.

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
  - No `.unwrap()`/`.expect()` in app paths; keep the core crates `no_std`
    and allocation-free.
  - Descriptive variable names matching type names; no single-letter names.
  - Any color defined with numeric components gets a nearby approximate-name
    comment.
  - Rust getters do not use a `get_` prefix.

## Design decisions (agreed with the human, 2026-07-03)

1. **All solver state lives in `reverse_kinematics.rs`** behind one
   controller struct, `ReverseKinematics`. The game loop in `main.rs` holds
   exactly one local of this type and calls a five-method API (below). No
   `Run`/`Phase` internals, no `playing: bool`, no `Option<Run>` in the loop.
2. **Ticking is explicit frame scheduling.** The current loop redraws every
   frame unconditionally, so the solver advances via one plain
   `reverse_kinematics.tick(...)` call per frame — never hidden inside touch
   handling (this was the parked module's main complaint about the old code).
3. **The step button is a hold-button, not a click-button.** A new `Ui`
   interaction method `hold_button` captures the touch like a slider and
   reports `true` every frame from touch-down to touch-up. The first held
   frame always yields exactly one solver step (so a tap does one step);
   continued holding repeats steps at a fixed real-time rate, independent of
   frame rate. Same `IconButton` widget data — the *interaction* differs, so
   it is a new `Ui` method, not a new widget struct.
4. **Manual arm-slider touch cancels a running solve** — touching an arm
   slider means the user is taking control, whether or not the value moved.
   Camera sliders (tilt / zoom / x-y view) do NOT cancel; the old code had
   the same distinction. Prev/next target buttons also cancel.
5. **Param indexes come from slider labels**, following commit `4658ea2`:
   the solver derives its bend-elbow and spin-whole-arm indexes via
   `LINKAGE.param_index("bend elbow", 0)` etc., and sweeps the existing
   `ARM_PARAM_INDEXES` array. No hardcoded `BEND_ELBOW_PARAM: usize = 4`.
6. **The solver reads its parent's items directly** (`super::LINKAGE`,
   `super::ARM_TIP_LINKAGE`, `super::compute_target_distance`,
   `super::ARM_PARAM_INDEXES`, `super::DOF`). The ownership rule from the
   parked doc is one-directional: solver state must not leak into the loop;
   the solver using `armatron` constants is fine. Mark the needed `main.rs`
   items `pub(super)`/module-visible as required.
7. **Play/stop feedback via two `IconButton` statics** sharing one
   rectangle: `RK_RUN_BUTTON` (`Icon::Play`) and a new `RK_STOP_BUTTON`
   (`Icon::Stop`, filled square). `ReverseKinematics::run_button()` returns
   whichever matches the playing state; the loop passes it straight to
   `ui.icon_button`.
8. **Distance-to-target display stays in `main.rs`** — it is part of the
   manual game and is not solver-owned.
9. **All solver pacing is real-time (dt-based), never per-frame.** The old
   constants were tuned when the game ran at about 9 fps, so "per tick"
   secretly meant "per ~0.11 s". At today's frame rates a per-frame solver
   would run several times faster than tuned. Every rate below is expressed
   per second and consumed through a dt budget; the values are chosen to
   reproduce the old *effective* speeds at 9 fps.

## Solver behavior to restore (search logic unchanged, pacing now real-time)

Greedy coordinate search with step decay, spread across frames:

- State: `search_params`, `best_params`, `best_distance`, `step`,
  `candidate_index`, `sweep_improved`, and a `Phase` enum
  (`BeginCandidate`, `EvaluateSingleHigh`, `EvaluateSingleLow`,
  `EvaluatePair`).
- Candidates per sweep: one high/low probe per arm param (6) plus 4 paired
  bend-elbow × spin-whole-arm candidates
  (`[(1,1), (1,-1), (-1,1), (-1,-1)]`), 10 total.
- After a sweep with no improvement, halve `step`; stop when `step` drops
  below the minimum.
- The visible arm interpolates toward `best_params` at a dt-scaled rate while
  playing (this part was already real-time in the old code).

### Real-time pacing (replaces the old per-frame pacing)

The old code ran `RK_SEARCH_CANDIDATES_PER_TICK = 4` candidates per frame and
one hold-step per frame — tuned at about 9 fps. Convert both to per-second
rates consumed through fractional `f32` budget accumulators on the
controller:

- Each frame, clamp `dt_seconds` to `MAX_TICK_SECONDS` (guards against
  pauses/hitches), then add `dt * rate` to the relevant budget; consume whole
  units from the budget, carrying the fraction to the next frame.
- `tick` (playing): candidate budget accrues at
  `SEARCH_CANDIDATES_PER_SECOND`; each whole unit runs one
  `tick_search_candidate`. Visible params move by
  `dt_clamped * VISIBLE_PARAM_POINTS_PER_SECOND` as before.
- `hold_step` (step button held): step budget accrues at
  `HOLD_STEPS_PER_SECOND`, **and the first frame of a hold seeds the budget
  with 1.0** so a tap always performs exactly one step regardless of dt.
  Each whole step runs one `tick_search_candidate` and moves the visible
  params by `SINGLE_STEP_VISIBLE_PARAM_STEP`.
- Budgets reset to zero on `clear`, `toggle`, and when `hold_step` sees
  `Idle`, so no stale fraction fires later.

Constants (drop the old `RK_` prefix — they are private to the module, so
plain names suffice). Rates are the old per-frame values × 9 fps:

| old name | new name | value | note |
| --- | --- | --- | --- |
| `RK_INITIAL_STEP` | `INITIAL_STEP` | `0.125` | unchanged |
| `RK_MIN_STEP` | `MIN_STEP` | `0.001` | unchanged |
| `RK_VISIBLE_PARAM_POINTS_PER_SECOND` | `VISIBLE_PARAM_POINTS_PER_SECOND` | `0.6` | already real-time |
| `RK_MAX_TICK_SECONDS` | `MAX_TICK_SECONDS` | `0.1` | now clamps dt for budgets too |
| `RK_SINGLE_STEP_VISIBLE_PARAM_STEP` | `SINGLE_STEP_VISIBLE_PARAM_STEP` | `0.01` | per step, not per frame |
| `RK_SEARCH_CANDIDATES_PER_TICK` (`4`/frame) | `SEARCH_CANDIDATES_PER_SECOND` | `36.0` | 4 × 9 fps |
| — (1 hold-step/frame) | `HOLD_STEPS_PER_SECOND` | `9.0` | 1 × 9 fps |
| `RK_PAIRED_CANDIDATES` | `PAIRED_CANDIDATES` | 4 sign pairs above | unchanged |

`move_params_toward` iterates `ARM_PARAM_INDEXES` (label-derived, possibly
non-contiguous) instead of the old `ARM_PARAM_START..ARM_PARAM_START + ARM_PARAM_COUNT`
range.

## Controller API

```rust,no_run
# struct ReverseKinematics;
# const DOF: usize = 15;
impl ReverseKinematics {
    /// Idle controller: no run state, not playing.
    pub(super) fn new() -> Self { unimplemented!() }

    /// Play/stop. Starting seeds the search from the current params.
    pub(super) fn toggle(&mut self, params: &[f32; DOF]) { unimplemented!() }

    /// Called once per frame with the step button's hold state.
    /// `Idle` resets the step budget. `Pressed` (touch-down frame) stops
    /// play, ensures a run exists, and seeds the budget with 1.0 so a tap
    /// always yields one step. `Held` accrues budget at
    /// `HOLD_STEPS_PER_SECOND`. Each whole budget unit advances one search
    /// candidate and nudges the visible params by
    /// `SINGLE_STEP_VISIBLE_PARAM_STEP`.
    pub(super) fn hold_step(
        &mut self,
        params: &mut [f32; DOF],
        hold: HoldButtonState,
        dt_seconds: f32,
    ) { unimplemented!() }

    /// Forget the run and stop playing (manual interference, target change).
    pub(super) fn clear(&mut self) { unimplemented!() }

    /// Per-frame advance while playing: consume the candidate budget
    /// (accrued at `SEARCH_CANDIDATES_PER_SECOND`) running search
    /// candidates, then move the visible params toward `best_params` at the
    /// dt-scaled rate. Stops playing on convergence (search exhausted and
    /// visible params caught up).
    pub(super) fn tick(&mut self, params: &mut [f32; DOF], dt_seconds: f32) { unimplemented!() }

    /// The play or stop `IconButton` matching the current playing state.
    pub(super) fn run_button(&self) -> &'static IconButton { unimplemented!() }
}
# struct IconButton;
# enum HoldButtonState { Idle, Pressed, Held }
```

The controller privately carries the two fractional `f32` budget
accumulators from the real-time pacing section. The old
`tick_reverse_kinematics_at` / `previous_tick` plumbing collapses because
the loop computes `dt_seconds` once per frame (Phase 3).

## Phase 1 — `Ui` additions (no armatron behavior change)

All in `crates/linkage-blaze-example-core/src/ui.rs` and
`armatron/controls.rs`.

- [ ] impl / [ ] verify — **`Icon::Stop`**: filled square icon, drawn inside
  the button rectangle with the same inset style as `Play`. Reuse the play
  icon's green unless the old stop drawing used another color (check
  `4fbeda7^`); add the approximate-color-name comment.
- [ ] impl / [ ] verify — **`Ui::hold_button`**: same drawing as
  `icon_button`, different interaction. On an unconsumed `Down` inside the
  rectangle, the button captures the touch (new `active_hold_button:
  Option<&'static IconButton>` field, cleared on `Up` in `Ui::begin`,
  mirroring `active_slider`). Returns `Ok(HoldButtonState)`:
  `Pressed` on the capturing `Down` frame, `Held` on subsequent frames
  while captured, `Idle` otherwise. While captured, draw a pressed state
  (e.g. filled background behind the icon) so the repeat is visible.
  `HoldButtonState` is a new public enum in `ui.rs`.
- [ ] impl / [ ] verify — **`Ui::slider` returns `Ok(bool)`**: `true` when
  this slider is the active slider this frame (grabbed on `Down` or being
  dragged). Camera-slider call sites simply keep `?;` and drop the bool.
- [ ] impl / [ ] verify — **`RK_STOP_BUTTON`** static in `controls.rs`,
  same rectangle as `RK_RUN_BUTTON`, `Icon::Stop`.

Phase gate: `just check-all`.

## Phase 2 — Solver module

All in `crates/linkage-blaze-example-core/src/armatron/reverse_kinematics.rs`.

- [ ] impl / [ ] verify — Restore `Run` (old `ReverseKinematicsRun`) and
  `Phase` (old `ReverseKinematicsPhase`) as private types, logic unchanged
  except: label-derived `BEND_ELBOW_PARAM_INDEX` / `SPIN_WHOLE_ARM_PARAM_INDEX`
  consts (via `LINKAGE.param_index`), sweeps over `super::ARM_PARAM_INDEXES`,
  and the un-prefixed constant names from the table above.
- [ ] impl / [ ] verify — `ReverseKinematics` controller with the exact API
  above wrapping `Option<Run>` + `playing: bool`. `move_params_toward` and
  the paired-candidate helper live here, private.
- [ ] impl / [ ] verify — Replace the parked-notes module doc with real
  documentation: what the solver does (greedy coordinate search with step
  decay, amortized across frames), and the ownership rule (solver state
  never leaks into the game loop; the distance label in `main.rs` is not
  solver-owned).
- [ ] impl / [ ] verify — Make the `main.rs` items the solver needs
  visible to it (`pub(super)` or moving `ARM_PARAM_INDEXES` etc. as
  appropriate — prefer the smallest visibility change, no new modules).

Phase gate: `just check-all`. The solver is not yet wired to the loop; it
must compile warning-free anyway (no lint suppression — if dead-code
warnings appear, wire a minimal call or finish Phase 3 in the same session
rather than adding `#[allow]`).

## Phase 3 — Game-loop wiring

All in `crates/linkage-blaze-example-core/src/armatron/main.rs`.

- [ ] impl / [ ] verify — Compute `dt_seconds` once per frame from
  `previous_tick` at the top of the loop and feed both the fps label and
  `reverse_kinematics.tick`; refactor `next_fps_label` accordingly.
- [ ] impl / [ ] verify — One `let mut reverse_kinematics =
  ReverseKinematics::new();` before the loop. Wire the buttons:

  ```rust,no_run
  # // sketch, not literal code
  if ui.icon_button(&mut frame, reverse_kinematics.run_button())? {
      reverse_kinematics.toggle(&params);
  }
  let hold_button_state = ui.hold_button(&mut frame, &RK_STEP_BUTTON)?;
  reverse_kinematics.hold_step(&mut params, hold_button_state, dt_seconds);
  ```

  `hold_step` is called unconditionally every frame; the `Idle` state is how
  the controller learns the hold ended and resets its step budget.

  Remove the "Clicks not yet wired to actions" comment (it is a plain
  comment, not a `todo`).
- [ ] impl / [ ] verify — Cancellation: arm-slider calls become
  `if ui.slider(...)? { reverse_kinematics.clear(); }`; prev/next target
  handlers call `reverse_kinematics.clear()` before reseeding. Camera
  sliders keep plain `?;`.
- [ ] impl / [ ] verify — `reverse_kinematics.tick(&mut params, dt_seconds);`
  after the widget section, before `ui.end` — one line, commented as the
  solver's explicit frame-schedule slot.

Phase gate: `just check-all`, then human plays the game on hardware or WASM:
play converges the arm toward the target and the button shows stop while
running; tap-step advances once; hold-step repeats until lifted; dragging an
arm slider or changing targets cancels; camera sliders do not cancel. Solver
speed should feel the same on a fast WASM build and on slower hardware —
that is the real-time pacing working (the fps label makes this checkable).

## Out of scope

- Any change to the distance-label game mechanics in `main.rs`.
- The `todo000` shared-home question from `SINGLE_SOURCE_SPEC.md` about
  `arm_tip`/`compute_target_distance` duplication — leave that todo in
  place; the solver calling `super::compute_target_distance` neither fixes
  nor worsens it.
- Smarter solvers (gradient, jacobian, randomized restarts). Restore the old
  greedy search as-is first.
