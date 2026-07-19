<!-- todo0 consider deleting this spec once the review decisions are implemented and released. -->

# Armatron TODO review

Planning notes for the review TODOs in
[`armatron/main.rs`](../crates/linkage-blaze-core/src/examples/armatron/main.rs),
including which findings should also be applied to the files listed in
[`review-files_SPEC.md`](review-files_SPEC.md).

This spec is planning only. It does not authorize changes in the sibling
Device Envoy repository or generated ESP board files.

## Recommended scope

Do not apply every Armatron cleanup mechanically to all reviewed files. Most
findings are local to the Armatron core implementation. Use targeted
cross-file passes only for entry-point documentation and naming.

Only edit ESP Jinja templates. Do not edit their generated board files.

## Decisions and action items

### Parent-module visibility

Remove `pub(super)` from these items in `armatron/main.rs`:

- `LINKAGE`
- `ARM_TIP_LINKAGE`
- `ARM_PARAM_INDEXES`
- `compute_target_distance`, or its eventual replacement

Rust child modules can access private items declared by an ancestor module.
Here, `pub(super)` instead exposes the items to the surrounding `examples`
module, which is broader than required. Do not add a comment explaining the
visibility; express the intended boundary with private visibility.

Keep `pub(super)` on items declared inside `controls.rs` and
`reverse_kinematics.rs` when their parent module calls or imports those items.

Cross-file scope: audit only the Armatron module family. Do not remove
`pub(super)` or `pub(crate)` mechanically from the other review files.

### Compile-time arm parameter indexes

Keep `ARM_PARAM_INDEXES` as a const block. It resolves the slider labels to
linkage parameter indexes at compile time, and both the manual controls and
reverse-kinematics search use the result.

Add a concise comment explaining that purpose. Do not introduce a one-use
helper function merely to hide the const loop.

Cross-file scope: local to Armatron.

### FPS display

Delete the always-true `SHOW_FPS_TEXT` constant and display FPS whenever a
previous measurable frame time is available. The first frame will naturally
have no FPS value.

Delete or inline `next_fps_label`. Keep one named helper for the actual
elapsed-time, rounding, and divide-by-zero calculation.

Do not silently cap a measured rate at `99.0 fps`. Prefer an explicit overflow
presentation such as `99+ fps` when the numeric form will not fit the label.

Cross-file scope: local to Armatron. Ballet already displays FPS
unconditionally.

### `run` documentation

Shorten the Armatron `run` documentation. Retain these behavioral contracts:

- Platform setup must provide calibrated touch before calling `run`.
- The function returns when physical or on-screen input requests calibration.
- The scene is drawn before the immediate-mode widgets, so widget-driven scene
  changes become visible on the next frame.

Remove the numbered per-frame walkthrough. Replace “forever” with wording that
acknowledges the calibration exit.

Apply the same exit-aware wording to `clock::run` and
`skeleton_clock::run`, because they can return an `Exit`. Ballet returns
`Infallible`, so its “forever” wording is accurate. DNS Tester already
documents its exit behavior.

Cross-file scope: the four Linkage Blaze core examples and DNS Tester core for
consistency review. Platform callers do not need copied versions of the core
API documentation.

### Generic parameters and button names

Change the Armatron entry point toward this shape:

```rust,no_run
pub async fn run<CydDevice, ButtonDevice>(
    cyd: &mut CydDevice,
    button: &mut ButtonDevice,
) -> Result<Exit, Error<CydDevice::Error>>
where
    CydDevice: Cyd,
    ButtonDevice: Button,
```

Also replace the generic `Error<F>` parameter with a descriptive name such as
`Error<CydError>`.

For platform callers, use the variable name that matches the concrete type and
context:

- Prefer `button` for RP `ButtonRp` values when there is only one button.
- Prefer `button` for the WASM capability field currently named
  `button_watch`.
- Keep `button_watch` in ESP templates when the concrete type is
  `ButtonWatch`.

Review the other shared core entry points for descriptive generic and error
type parameters. Do not rename concrete `ButtonWatch` values merely to make
every platform file textually identical.

Cross-file scope: Armatron core, its RP/WASM callers, and a targeted shared
entry-point naming audit. The DNS Tester core signature is a useful model.

### Generic CYD error conversion

Keep `.map_err(Error::Cyd)?` on both `touch.read()` and `frame.flush()`.

These conversions are intentional. A blanket `From<CydError>` implementation
would overlap with the concrete UI and linkage conversions under Rust's
coherence rules. Keep the explanation on the `Error` type rather than adding
comments to each call site.

Cross-file scope: do not bulk-remove `map_err`. Ballet, Clock, Skeleton Clock,
and DNS Tester have similar intentional generic or foreign error boundaries
that must be assessed individually.

### Scene and UI ordering comment

Reduce the local comment before scene drawing to one line, for example:

```rust,no_run
// Draw the scene before widgets so the UI appears on top.
```

Keep the one-frame latency explanation in the public `run` documentation or
the UI module documentation, not in all three locations.

Cross-file scope: local to Armatron.

### UI frame lifecycle names

`Ui::begin` is stateful. It stores the current touch event and cursor, resets
per-frame touch consumption, and preserves active slider or hold-button capture
across frames.

Rename the frame lifecycle methods for clarity:

- `Ui::begin` to `Ui::begin_frame`
- `Ui::end` to `Ui::end_frame`

Keep their behavioral explanations on the method documentation. Update the UI
doctest and Armatron call sites.

Cross-file scope: `examples/ui.rs`, its doctest, and Armatron. No broad platform
pass is needed.

### Private helper cleanup

Inline only helpers that do not carry a useful abstraction:

- Replace `previous_target_seed` with `target_seed.wrapping_sub(1)`.
- Replace `next_target_seed` with `target_seed.wrapping_add(1)`.
- Delete tests that only retest `wrapping_sub` and `wrapping_add`.
- Delete or inline `next_fps_label` after removing `SHOW_FPS_TEXT`.
- Collapse `arm_tip`, `target_center`, `compute_target_distance`, and
  `target_distance` into one shared private `target_distance(params)` function
  used by the main loop and reverse-kinematics module.

Keep helpers that contain meaningful or reused behavior:

- Keep `randomize_target_from_seed`; it is cohesive and used in multiple
  places.
- Keep one FPS calculation helper for timing, rounding, and zero-duration
  handling.
- Keep `target_distance_hundredths` if it makes the label path easier to read;
  otherwise inline that conversion immediately before formatting.

Do not silently clamp target distance to `99.99`. Assert that the result is
finite and within the supported display range, or represent the overflow
explicitly in the label.

Cross-file scope: local to Armatron. A focused scan found no equivalent
semantic clamp in the other listed files; the `min` operations in DNS Tester
and Device Envoy memory code are structural bounds calculations.

### Finish the in-file review

After the decisions above are implemented, remove the “continue reviewing
here” marker.

While editing the file, also make these local consistency changes:

- Move the `Error` type immediately after `run` and before the test module.
- Import `Infallible` and use it unqualified.
- Convert straightforward successful calibration tests from `.expect(...)` to
  test functions returning `Result` and using `?`.
- Keep the reverse-kinematics scheduling logic and the typed `Exit` enum.

## Open questions for the implementer

Resolve these before or during implementation:

1. For a distance outside the label's supported range, should the program fail
   fast with an assertion, or should the UI show an explicit overflow value?
   The repository guidance favors failing fast over silent clamping, but an
   overflow label may be better for a physical demo.
2. Is `99+ fps` the desired high-rate presentation, or should the layout be
   widened to show three-digit FPS values?
3. Should the public UI API use symmetrical `begin_frame`/`end_frame` names, or
   should `end` instead be renamed for its current narrow behavior, such as
   `draw_touch_cursor`? The latter is more literal but exposes the current
   rendering detail.

## Suggested implementation sequence

1. Clean up Armatron visibility, names, comments, and helper structure.
2. Rename the UI frame lifecycle methods and update their doctest and caller.
3. Update the affected Armatron RP/WASM caller variable names while preserving
   type-appropriate names in ESP templates.
4. Review the core `run` documentation for Armatron, Ballet, Clock, Skeleton
   Clock, and DNS Tester, changing only inaccurate or inconsistent wording.
5. Run formatting and local CI.

## Verification

Run:

```text
just check-all
```

If implementation changes any file in the sibling Device Envoy repository,
also run its documented `cargo check-all` command from that repository. Do not
run the Device Envoy suite merely because its files were read during planning.

