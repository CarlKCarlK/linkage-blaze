# Immediate-Mode UI Refactor Spec

Refactor the armatron example's control layer from retained-mode widget objects
(`ArmatronUi`) to a true immediate-mode GUI: a small generic widget module plus
a declarative layout description. The result will be showcased in a Medium
article, so clarity of the final code matters as much as correctness.

This spec is written to be executed by any capable agent (Claude, Codex, etc.),
one phase per session.

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
  - Never delete `TODO`/`todo` comments. Move them with the code they annotate;
    if one seems obsolete, append `(may no longer apply)`.
  - No `.unwrap()`/`.expect()` in app paths; propagate errors with `?`. Use
    `.unwrap_infallible()` (crate-local `InfallibleResultExt`) only for
    `Infallible` results.
  - Error enums: derived `From` for our own error types, explicit
    `.map_err(...)` only for the single generic device error (the
    `ballet::Error` pattern documented in `AGENTS.md`).
  - No builder pattern; direct `const fn` constructors.
  - No `mod.rs` files. Entry points at the top of the file.
  - Descriptive variable names matching type names; no single-letter names.
  - Keep the crate `no_std` and allocation-free.

## Baseline (before phase 1)

For article metrics, record these before starting:

- [ ] impl — Record baseline: `controls.rs` line count (667), `main.rs` line
      count (398), and "places touched to add one widget" (7: struct field,
      `new()`, `begin_frame()`, `handle_touch_down/up`, `control_at`/
      `ActiveControl`, `draw()`, getter on `ArmatronUi`). Save the numbers at
      the bottom of this file under "Metrics".
  - [ ] verify

## Target design (read fully before phase 1)

### Files

- `crates/linkage-blaze-example-core/src/ui.rs` — **new**, generic,
  armatron-agnostic immediate-mode widget module. Declared `pub mod ui;` in
  `lib.rs` with module docs (it is part of the showcase).
- `crates/linkage-blaze-example-core/src/armatron/controls.rs` — shrinks to a
  pure **layout description**: `static` layout specs and the param-slider
  column. No widget logic, no event handling, no drawing code.
- `crates/linkage-blaze-example-core/src/armatron/main.rs` — the game loop
  calls `ui.*` widgets directly; `ArmatronUi` and its helpers are deleted.

### Core principle: no widget state

Widgets do not store values. The app's `params: [f32; DOF]` array is the single
source of truth; a slider borrows its value for one call:
`ui.slider(&mut frame, &TILT_SLIDER, &mut params[TILT_PARAM_INDEX])`. This
removes bidirectional sync bugs (today, external writes to `params` are stomped
by `linkage_params()` every frame) and removes the value-carrying constructors.

The only persistent UI state is the `Ui` struct: the touch cursor, this frame's
event, and which slider (if any) has captured the current drag.

### Widget identity: `static` layout specs

The active-drag slider is identified by pointer equality (`core::ptr::eq`) on
its layout spec. For this to be sound, layout specs MUST be `static`, not
`const` — statics have a guaranteed unique, stable address; consts are inlined
per use and address stability is not guaranteed. This is a deliberate,
article-worthy design point; document it on `Ui::slider`.

### Draw order and the one-frame-lag rule

Widget calls both update state and draw, so call order is z-order. The loop
draws the 3D scene FIRST (using `params` as updated by last frame's UI), then
runs the UI widgets on top. Consequence: the scene reflects input with one
frame of latency (~50 ms at 20 fps). This is the standard immediate-mode
tradeoff (dear imgui works the same way); document it in the loop comment.
Labels and the touch cursor draw last, on top of everything.

### Button semantics

Buttons fire on touch-DOWN inside their rectangle (current behavior — right for
a resistive touchscreen; no press-cancel gesture). `Ui::button` returns
`Ok(true)` only on the frame the down event lands inside the rectangle.
Document this choice on `Ui::button`. A down event is consumed by the first
widget (in call order) whose touch rectangle contains it; `Ui` tracks a
`down_consumed: bool`, reset in `begin()`, so overlapping widgets can't both
fire.

### Drag capture

On touch-down inside a slider's touch rectangle, that slider becomes
`active_slider` and keeps receiving Move updates until touch-up, even if the
cursor leaves the rectangle (preserves current capture behavior). Touch-up
clears `active_slider` in `begin()`.

### `ui.rs` API (signatures are normative; bodies are the executor's job)

```rust
pub struct Ui {
    touch_cursor: Option<(f32, f32)>,
    touch_event: Option<TouchEvent>,
    active_slider: Option<&'static Slider>,
    down_consumed: bool,
}

impl Ui {
    pub fn new() -> Self;               // also derive/impl Default

    /// Start a frame: store the event, update the touch cursor,
    /// clear capture on Up, reset down_consumed.
    pub fn begin(&mut self, touch_event: Option<TouchEvent>);

    /// Update `value` from any captured drag, then draw track, label, knob.
    pub fn slider<D>(
        &mut self,
        target: &mut D,
        slider: &'static Slider,
        value: &mut f32,
    ) -> Result<(), UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>;

    /// Draw the button; return true iff it was clicked this frame.
    pub fn button<D>(
        &mut self,
        target: &mut D,
        button: &'static Button,
    ) -> Result<bool, UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>;

    /// Like `button`, but draws an `Icon` instead of a text label.
    pub fn icon_button<D>(
        &mut self,
        target: &mut D,
        icon_button: &'static IconButton,
    ) -> Result<bool, UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>;

    /// Format `args` into a stack buffer and draw at the label's position.
    pub fn label<D>(
        &self,
        target: &mut D,
        label: &'static Label,
        args: fmt::Arguments<'_>,
    ) -> Result<(), UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>;

    /// End the frame: draw the touch cursor (cyan filled circle, radius 5)
    /// on top of everything, if a touch is in progress.
    pub fn end<D>(&self, target: &mut D) -> Result<(), UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>;
}
```

Notes:

- `label` formats into a stack-local `heapless::String<LABEL_CAPACITY>` with
  `const LABEL_CAPACITY: usize = 24;` (longest current text is
  `"distance 99.99"` = 14). Overflow is a real `fmt::Error`, propagated —
  no swallowing.
- All methods return `UiError` uniformly (even those that can only fail on
  draw) so the game loop is a clean run of `?`.

### Layout spec types (in `ui.rs`, constructed by `const fn`)

```rust
#[derive(Clone, Copy)]
pub struct Slider {
    label: &'static str,
    touch_rectangle: Rectangle,
    track_start: Point,
    track_end: Point,
    orientation: SliderOrientation,   // private enum, Horizontal | Vertical
    range_start: f32,                 // value at track_start
    range_end: f32,                   // value at track_end
}

#[derive(Clone, Copy)]
pub struct Button {
    touch_rectangle: Rectangle,
    label: &'static str,
}

#[derive(Clone, Copy)]
pub struct IconButton {
    touch_rectangle: Rectangle,
    icon: Icon,
}

#[derive(Clone, Copy)]
pub enum Icon {
    Play,        // filled green triangle
    StepForward, // outlined box, green triangle + white bar
}

#[derive(Clone, Copy)]
pub struct Label {
    position: Point,
    color: Rgb888, // converted to Rgb565 at draw time (From is not const)
}
```

Constructors (all `const fn`): `Slider::horizontal(...)`,
`Slider::vertical(...)`, `Button::new(...)`, `IconButton::new(...)`,
`Label::new(...)`. In phase 2 the constructors take explicit touch AND track
rectangles copied verbatim from today's numbers; phase 3 replaces that with
derived geometry (see phase 3).

Icon drawing must be parameterized by the icon button's rectangle — scale the
current hardcoded triangle/bar geometry (which references the global RK
rectangles) into whatever rectangle the spec provides. The two RK buttons are
18x18, so pixel-identical output for them is the correctness check.

### Error handling

In `ui.rs`, mirroring the `ballet::Error` pattern from `AGENTS.md`:

```rust
#[derive(Debug, derive_more::From)]
pub enum UiError<D> {
    /// Formatting label text failed (buffer overflow).
    Text(fmt::Error),
    /// Drawing to the target failed.
    #[from(ignore)]
    Draw(D),
}
```

`armatron::Error` changes to:

```rust
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// A UI widget failed (text formatting; draw is Infallible here).
    Ui(UiError<Infallible>),
    /// Reading touch events or flushing a frame failed.
    #[from(ignore)]
    Cyd(F),
}
```

Update the enum's doc comment (it currently explains the `Text`/`Cyd`
coherence asymmetry; keep that explanation, adapted). Check the platform
crates (`linkage-blaze-classic` example, `linkage-blaze-armatron-wasm`,
`linkage-blaze-armatron-c6`) for anything matching on the removed `Text`
variant.

### Target game loop (normative shape, not verbatim)

```rust
let mut ui = Ui::new();
let mut previous_tick = None;
let mut frame = display.full_frame_mut();

loop {
    frame.fill(BACKGROUND_565);

    // Scene first, UI on top: params here were updated by last frame's
    // widgets (standard immediate-mode one-frame latency).
    for draw_item_3d in LINKAGE.draw_items_3d(&params) {
        draw_item_3d.project(&PROJECTION).draw(&mut frame);
    }

    ui.begin(touch.read().map_err(Error::Cyd)?);

    ui.slider(&mut frame, &TILT_SLIDER, &mut params[TILT_PARAM_INDEX])?;
    ui.slider(&mut frame, &DOLLY_SLIDER, &mut params[DOLLY_PARAM_INDEX])?;
    ui.slider(&mut frame, &XY_VIEW_SLIDER, &mut params[XY_VIEW_PARAM_INDEX])?;
    for (param_slider, param_index) in PARAM_SLIDERS.iter().zip(ARM_PARAM_INDEXES) {
        ui.slider(&mut frame, param_slider, &mut params[param_index])?;
    }

    if ui.button(&mut frame, &PREVIOUS_TARGET_BUTTON)? {
        target_seed = target_seed.wrapping_sub(1);
        randomize_target(target_seed, &mut params);
    }
    if ui.button(&mut frame, &NEXT_TARGET_BUTTON)? {
        target_seed = target_seed.wrapping_add(1);
        randomize_target(target_seed, &mut params);
    }

    // Clicks not yet wired to actions (matches current behavior).
    ui.icon_button(&mut frame, &RK_RUN_BUTTON)?;
    ui.icon_button(&mut frame, &RK_STEP_BUTTON)?;
    ui.button(&mut frame, &CALIBRATE_BUTTON)?;

    ui.label(&mut frame, &TARGET_LABEL, format_args!("target #{target_seed}"))?;
    // ... distance label, fps label (only when computable), version label ...

    ui.end(&mut frame)?; // touch cursor on top

    frame.flush().await.map_err(Error::Cyd)?;
}
```

`randomize_target(seed, &mut params)` replaces `update_target` (the click
bools now live in the loop). The seeded-RNG body is unchanged, and the initial
randomization before the loop calls the same function. Keep
`display_fps_since` and the distance-computation helpers as-is; the
`update_fps_text` / `update_text_info` wrappers dissolve into loop-side
`ui.label` calls.

## Phase 1 — Build `ui.rs` alongside the existing code

Nothing in armatron changes in this phase. The new module must be complete,
documented, and compiling on all targets, but is not yet called.

- [ ] impl — Create `crates/linkage-blaze-example-core/src/ui.rs` with the
      types, constructors, and `Ui` methods specified above. Include module
      docs stating the pattern (immediate mode, no widget state, statics as
      identity, on-down click semantics, one-frame lag).
  - [ ] verify
- [ ] impl — Add `pub mod ui;` to `lib.rs`. Public (not `pub(crate)`): the
      module is intended as a reusable showcase library, and public visibility
      avoids dead-code warnings while it is not yet wired up.
  - [ ] verify
- [ ] impl — Port slider geometry/value math from today's `SliderControl`
      (`set_value_from_touch` clamping, `knob_center` rounding via
      `libm::roundf`) so knob positions are pixel-identical.
  - [ ] verify
- [ ] impl — Port `TextButton` drawing (slate-gray 1px stroke rectangle,
      white centered `FONT_6X10` label via `centered_text_position` logic) and
      `Icon` drawing (parameterized by rectangle, per "Layout spec types").
  - [ ] verify
- [ ] impl — Define `UiError<D>` as specified, with the coherence-asymmetry
      doc comment.
  - [ ] verify
- [ ] impl — Add one compilable `rust,no_run` doctest on `Ui` showing a
      minimal begin/slider/end frame (hide boilerplate with `#` lines per
      `AGENTS.md`).
  - [ ] verify

**PHASE 1 GATE — stop here and test:**

- [ ] `just check-all` passes (all targets, `-D warnings`).
- [ ] Human review of the `ui.rs` API before phase 2 builds on it.
- [ ] Suggest a commit message; human commits.

## Phase 2 — Rewrite armatron on top of `ui.rs`

The big cut-over. App behavior must be preserved except for the documented
one-frame scene latency.

- [ ] impl — Rewrite `armatron/controls.rs` as a layout description only:
      `static` specs (`TILT_SLIDER`, `DOLLY_SLIDER`, `XY_VIEW_SLIDER`,
      `PARAM_SLIDERS: [Slider; PARAM_SLIDER_COUNT]`,
      `PREVIOUS_TARGET_BUTTON`, `NEXT_TARGET_BUTTON`, `RK_RUN_BUTTON`,
      `RK_STEP_BUTTON`, `CALIBRATE_BUTTON`, `TARGET_LABEL`, `DISTANCE_LABEL`,
      `FPS_LABEL`, `VERSION_LABEL`) plus `PARAM_SLIDER_COUNT` and the
      `rectangle`/`point` const helpers. Copy all geometry numbers verbatim
      from the current constants, including each slider's existing touch
      rectangle. Keep the section comments ("Target selector strip…",
      "Left-side camera controls…", etc.) — they are part of the layout
      language's readability. `PARAM_SLIDERS` may be written as six literal
      entries in this phase (the column constructor is phase 3).
  - [ ] verify
- [ ] impl — Delete `ArmatronUi`, `ActiveControl`, `TextBox`, `TextButton`,
      `ShapeButton`, `ShapeButtonKind`, `SliderLayout`, `SliderControl`,
      `SliderOrientation`, and the RK draw functions from `controls.rs` (their
      logic now lives in `ui.rs`). Relocate any `todo` comments to the
      corresponding new code; never drop one.
  - [ ] verify
- [ ] impl — Rewrite the loop in `armatron/main.rs` per the target shape
      above: scene first, then widgets, labels, `ui.end`, flush. Delete
      `linkage_params` (sliders now write `params` directly). Replace
      `update_target` with `randomize_target(seed, &mut params)` used both
      before the loop and on button clicks. Keep the loop at the top of the
      file per `AGENTS.md` entry-point placement.
  - [ ] verify
- [ ] impl — Change `armatron::Error` to the `Ui(UiError<Infallible>)` form
      specified above; fix up platform crates if they reference the old
      `Text` variant.
  - [ ] verify
- [ ] impl — FPS label: compute via `display_fps_since` and draw only when a
      previous tick exists (first frame draws nothing — acceptable parity).
      Distance/target labels: same formatting as today
      (`"target #{seed}"`, `"distance {:02}.{:02}"`). Version label: pass
      `format_args!("{VERSION_TEXT}")`.
  - [ ] verify
- [ ] impl — Fix the `armatron()` doc comment: the loop redraws
      unconditionally every frame ("If the frame changed" is wrong today);
      document the scene-first/one-frame-lag ordering.
  - [ ] verify

**PHASE 2 GATE — stop here and test (the critical one):**

- [ ] `just check-all` passes.
- [ ] `just run-armatron-wasm` manual checklist:
  - [ ] Each of the 6 param sliders drags; the arm follows (next frame).
  - [ ] z (tilt), zoom (dolly), and x/y view sliders drag; view follows.
  - [ ] Drag started inside a slider continues when the cursor leaves it;
        releases cleanly on up.
  - [ ] prev/next change `target #N` text and move the red target; distance
        text updates.
  - [ ] RK play/step icons and `cal` button render as before; pressing them
        does nothing (unchanged).
  - [ ] FPS text updates; version text shows; cyan touch cursor tracks and
        draws on top.
- [ ] If hardware is available: `just run-armatron-classic` boots and touch
      works after calibration.
- [ ] Suggest a commit message; human commits.

## Phase 3 — Layout mini-language

Make `controls.rs` read as a layout description with no redundant numbers.

- [ ] impl — Derive slider touch rectangles instead of hand-writing them:
      `Slider::vertical(label, track_x, track_y, track_length, range_start,
      range_end)` and the horizontal twin compute the touch rectangle as the
      track expanded by `SLIDER_TOUCH_PAD` (14 px) perpendicular to the track
      and by the existing along-axis extents. Small (≤2 px) hit-area
      differences from today are acceptable; knob/track geometry must not
      change. If a slider genuinely needs a custom touch area, allow an
      explicit `with_touch_rectangle` variant rather than contorting the
      general rule.
  - [ ] verify
- [ ] impl — `Slider::column(x, top, step_y, labels: [&'static str; N])`
      (`const fn`, e.g. via a `while` loop over a `Copy` placeholder array)
      replaces the six literal `PARAM_SLIDERS` entries and the old
      `param_slider()` runtime math. If const-fn limitations make this ugly,
      fall back to literal entries and note why in a comment.
  - [ ] verify
- [ ] impl — Move slider label placement into the constructors as a rule
      (horizontal: above track start; vertical: current offset), replacing the
      scattered magic offsets (`y - 15`, `Point::new(x - 5, 5)`).
  - [ ] verify

**PHASE 3 GATE — stop here and test:**

- [ ] `just check-all` passes.
- [ ] `just run-armatron-wasm`: spot-check every control still hits and draws
      in the same place (compare against a phase-2 screenshot).
- [ ] Suggest a commit message; human commits.

## Phase 4 — Polish and article prep

- [ ] impl — `centered_text_position` derives glyph width from
      `FONT_6X10.character_size` instead of the hardcoded 6.
  - [ ] verify
- [ ] impl — Sweep `ui.rs` + `controls.rs` + `main.rs` doc comments for
      accuracy; ensure `ui` module docs read as the article's design section
      (they can be its first draft).
  - [ ] verify
- [ ] impl — Review surviving `todo` comments: relocate any still attached to
      moved code; append `(may no longer apply)` where the refactor plausibly
      resolved them. Do not delete any.
  - [ ] verify
- [ ] impl — Record final metrics below: line counts of `ui.rs`,
      `controls.rs`, `main.rs`, and "places touched to add one widget"
      (target: 2 — one `static` spec, one loop call).
  - [ ] verify

**PHASE 4 GATE:**

- [ ] `just check-all` passes.
- [ ] Final `just run-armatron-wasm` walkthrough of the phase-2 checklist.
- [ ] Suggest a commit message; human commits.

## Metrics (for the article)

| Measure | Before | After |
| --- | --- | --- |
| `controls.rs` lines | 667 | |
| `main.rs` lines | 398 | |
| `ui.rs` lines | — | |
| Places touched to add one widget | 7 | |
