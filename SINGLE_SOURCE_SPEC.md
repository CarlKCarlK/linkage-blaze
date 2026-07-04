# Single-Source-of-Truth Follow-up Spec

Follow-up to `IMMEDIATE_MODE_UI_SPEC.md` (all four phases complete). Three
themes, each article-worthy on its own:

1. **`Vec3` owns its math.** Distance/length live on the type, not as free
   functions duplicated across crates.
2. **The widget label is the parameter key.** A slider's drawn label and the
   linkage `param_index` lookup come from one string, checked at compile time.
3. **Derived constants over magic numbers.** `TARGET_PARAM_START` and friends
   are computed from the linkage, not hand-counted.

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
  - Omit const-generic turbofish when the const's type annotation already
    states the value.
  - Rust getters do not use a `get_` prefix.

## Open question for the human (does not block phases 1–3)

`linkage-blaze-armatron-core/src/lib.rs` and
`linkage-blaze-example-core/src/armatron/main.rs` duplicate `arm_tip`,
`target_center`, `compute_target_distance`, `distance`, `VERSION_TEXT`, and
`TARGET_PARAM_START`. Phase 1 removes the `distance` duplication via `Vec3`
methods; the rest remains duplicated. If armatron-core's UI path is legacy
and scheduled for deletion, the duplication is acceptable and needs only a
`todo`; if both live on, the shared helpers should move to a common home.
Decision: ______________________

## Phase 1 — `Vec3` owns its math

All in `crates/linkage-blaze-core/src/math.rs` unless noted. `libm` is
already a dependency of this crate (`Mat3::yaw` etc.).

- [x] impl — Add `impl Sub for Vec3` (component-wise), matching the style of
      the existing `Add` impl.
  - [x] verify
- [x] impl — Add three methods on `Vec3`, each `#[must_use]` with a one-line
      doc comment:

      ```rust
      pub fn dot(self, rhs: Self) -> f32;
      pub fn length(self) -> f32;          // libm::sqrtf(self.dot(self))
      pub fn distance_to(self, other: Self) -> f32;  // (self - other).length()
      ```

  - [x] verify
- [x] impl — Add unit tests in the existing `math.rs` test module: `Sub`,
      `dot`, `length` on a 3-4-0 triangle (length 5), and `distance_to`
      symmetry (`a.distance_to(b) == b.distance_to(a)`).
  - [x] verify
- [x] impl — In `linkage-blaze-example-core/src/armatron/main.rs`: replace
      the `distance` free function with `Vec3::distance_to`; delete `distance`
      and `square`; inline `round_to_u32` as `libm::roundf(...) as u32` at its
      single call site in `target_distance_hundredths`. Move the attached
      `todo1` comments per `AGENTS.md` (they said these helpers seemed silly —
      append `(resolved by Vec3 methods)` context rather than deleting if any
      wording survives).
  - [x] verify
- [x] impl — In `linkage-blaze-armatron-core/src/lib.rs`: replace its
      `distance` free function (~line 1408) and any `square` helper with
      `Vec3::distance_to`.
  - [x] verify
- [x] impl — Add a `todo` at the top of the duplicated helper block in
      `main.rs` referencing the open question above (shared home for
      `arm_tip`/`target_center`/`compute_target_distance`/`VERSION_TEXT`),
      unless the human has already answered it.
  - [x] verify

**PHASE 1 GATE — stop here and test:**

- [x] `just check-all` passes (all targets, `-D warnings`).
- [x] `cargo test -p linkage-blaze-core` and armatron-core tests pass
      (the reverse-kinematics distance tests exercise the migrated code).
- [ ] Suggest a commit message; human commits.

## Phase 2 — The widget label is the parameter key

Kill the remaining label duplication that phase 3 of the previous spec fixed
only for the six arm sliders. Today `"z"`, `"zoom"`, and `"x/y view"` appear
BOTH as slider labels in `controls.rs` and as `TILT_PARAM_NAME` /
`DOLLY_PARAM_NAME` / `XY_VIEW_PARAM_NAME` in `main.rs`; if they drift, a
slider silently controls the wrong joint. Same fix as before: one source of
truth, and the index lookup reads from it.

- [x] impl — In `ui.rs`, add a getter on `Slider`:

      ```rust
      /// The label drawn beside the track. Apps may also use it as a
      /// stable key (e.g. a linkage parameter name).
      #[must_use]
      pub const fn label(&self) -> &'static str;
      ```

  - [x] verify
- [x] impl — In `main.rs`, derive the camera indices from the layout specs
      and delete the three `*_PARAM_NAME` consts:

      ```rust
      const TILT_PARAM_INDEX: usize = LINKAGE.param_index(TILT_SLIDER.label(), 0);
      const DOLLY_PARAM_INDEX: usize = LINKAGE.param_index(DOLLY_SLIDER.label(), 0);
      const XY_VIEW_PARAM_INDEX: usize = LINKAGE.param_index(XY_VIEW_SLIDER.label(), 0);
      ```

  - [x] verify
- [x] impl — Move `ARM_PARAM_NAMES` from `main.rs` into `controls.rs`, next
      to the `PARAM_SLIDERS` column it feeds, and export it `pub(super)`.
      This fixes the odd dependency direction where the layout module reaches
      back into `main.rs` via `use super::ARM_PARAM_NAMES`.
  - [x] verify
- [x] impl — Collapse the six hand-written `ARM_PARAM_INDEXES` entries into a
      const block using the same `while`-loop trick `Slider::column` already
      uses (`param_index` is `const fn`). Read the labels from
      `PARAM_SLIDERS[slider_index].label()` so the drawn label and the index
      key are one value even if the array and the column ever diverge:

      ```rust
      const ARM_PARAM_INDEXES: [usize; PARAM_SLIDER_COUNT] = {
          let mut indexes = [0; PARAM_SLIDER_COUNT];
          let mut slider_index = 0;
          while slider_index < PARAM_SLIDER_COUNT {
              indexes[slider_index] =
                  LINKAGE.param_index(PARAM_SLIDERS[slider_index].label(), 0);
              slider_index += 1;
          }
          indexes
      };
      ```

  - [x] verify

**PHASE 2 GATE — stop here and test:**

- [x] `just check-all` passes.
- [ ] `just run-armatron-wasm`: every slider still moves its own joint —
      especially z, zoom, and x/y view (the three whose keys changed source).
- [ ] Suggest a commit message; human commits.

## Phase 3 — Derived constants, naming, and the `start`/`last` rename

- [ ] impl — Derive `TARGET_PARAM_START` instead of hardcoding 9: the ghost
      arm's params begin exactly where the pre-ghost linkage's end, so
      `const TARGET_PARAM_START: usize = <pre-ghost linkage>.view().dof();`
      (`dof` is `const fn`). This answers the existing
      `todo00 how to we feel about "TARGET_PARAM_START"` — append
      `(now derived from the linkage)` to it rather than deleting.
  - [ ] verify
- [ ] impl — Replace the two hardcoded `9`s in `arm_tip` (array size and
      slice bound) with the same derived constant. The `[f32; 9]` array
      length can use `TARGET_PARAM_START` directly since the arm params are
      exactly the pre-ghost params.
  - [ ] verify
- [ ] impl — Rename `LINKAGE0` to a self-describing name (suggested:
      `SCENE_WITH_ARM` — camera + grid + jointed arm; executor may pick
      better). Keep all intermediate consts: they carry the type annotations
      that let turbofish be omitted per `AGENTS.md`.
  - [ ] verify
- [ ] impl — In `ui.rs`, rename `Slider`'s `range_start`/`range_end` fields
      and the matching constructor parameters to `start`/`last`. Rationale:
      "range" is redundant on a slider, and `last` matches the new
      `core::range::RangeInclusive { start, last }` vocabulary for an
      inclusive endpoint (the knob can rest exactly on `last`). Deliberate
      trade, note in the field docs: `start: f32` sits beside
      `track_start: Point` — the doc comments must say "value at
      `track_start`" / "value at `track_end`" to keep the pairing obvious.
      Update `Slider::column`'s hardcoded `0.0, 1.0` call and the module
      docs/doctest if they mention the old names.
  - [ ] verify
- [ ] impl — Trim the unused hidden imports (`Rgb888`, `Rectangle`,
      `prelude`) from the `ui.rs` module doctest — it is article material and
      should carry no dead lines.
  - [ ] verify
- [ ] impl — Add a one-line comment on the clamp in
      `target_distance_hundredths` stating it is a display bound (the label
      is at most `"distance 99.99"`), so it does not read as the silent
      clamping `AGENTS.md` forbids.
  - [ ] verify

**PHASE 3 GATE — stop here and test:**

- [ ] `just check-all` passes.
- [ ] `just run-armatron-wasm`: full spot-check — target prev/next and
      distance text (derived `TARGET_PARAM_START`), all sliders (renamed
      fields), knob resting positions at both extremes of the z slider (its
      inverted `start: 1.0, last: 0.0` range is the regression canary).
- [ ] If hardware is available: `just run-armatron-classic` boots and touch
      works after calibration.
- [ ] Suggest a commit message; human commits.

## Metrics (for the article)

| Measure | Before | After |
| --- | --- | --- |
| Free math helper functions in `armatron/main.rs` | 3 (`distance`, `square`, `round_to_u32`) | 0 |
| Places each slider label string appears | 2 | 1 |
| Hand-counted param indices/offsets | 9 (3 names + 6 entries + literal `9`) | 0 |
