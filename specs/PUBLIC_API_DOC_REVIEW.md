<!-- TODO0 consider deleting this spec once the public API and documentation review is implemented and released. -->

# Public API Documentation Review

Review the generated `linkage-blaze` rustdoc index in display order. For each
item, decide whether it belongs in the public API, whether its public name is
appropriate, and whether its description explains the item clearly to a new
reader. Do not retain compatibility aliases when implementing API changes.

Whenever a top-level API summary introduces BVH, identify it as the Biovision
Hierarchy motion-capture file format. Do not assume readers already know what
the abbreviation means.

## Resolved Design

The following decisions resolve provisional alternatives mentioned later in
this review:

- Provide one always-present public `bvh` module. Put allocation-free,
  compile-time motion support in it unconditionally and gate only host-side
  parsing and conversion APIs behind the `bvh` feature.
- Use contextual BVH names: `bvh::Motion`, `bvh::motion!`, and host-side names
  such as `bvh::Clip` rather than repeating the `Bvh` prefix inside the module.
- Keep `LinkageBuf`; `Buf` distinguishes growable, allocator-backed storage
  from the fixed owner and borrowed view. Make `linkage_file!` the sole
  user-facing macro for loading `.lb.rs` assets. Remove the redundant
  standalone `linkage_fixed!` and `linkage_buf!` macros after making
  `linkage_file!` independent of them; do not retain hidden compatibility
  aliases under those names.
- Use one `render` module for evaluated drawing geometry and projection:
  `render::Item3d`, `render::Stroke`, `render::Disk`, `render::Sphere`, and
  `render::Projection`.
- Move mark-pose lookup onto `LinkageView`, return the primary `Error`, remove
  `MarkError`, and make the draw-item iterator private behind `impl Iterator`.
- Rename `Arg` to `StepArg` and `VariableArg` to `ParamArg`. Keep `ParamArg` as a
  named type rather than repeating its parameter index and operation range in
  several variants.
- Keep `Pose`, `StyledPose`, and `PenState` public and top-level as core
  evaluation results.
- Remove compatibility aliases. Update all workspace consumers directly to the
  final API.

## Implementation Order

### 1. Review the complete documented feature surface

- **Complete.** Regenerated with `cargo doc -p linkage-blaze --lib --no-deps
  --all-features` and reviewed the generated `bvh`, `bvh_parse`, and
  feature-gated `examples` modules.
- The source currently has `bvh` feature-gated and `bvh_parse` always present,
  contrary to the resolved design; this is recorded as an implementation task,
  not treated as the final surface.
- The complete item-by-item findings are recorded in the Modules section below.
- No API implementation changes are part of Step 1.

### 2. Establish the final module structure and names — Complete

- **Complete.** Created the always-present `bvh` facade, moved compile-time motion support and
  host-only support behind it, and establish contextual public names.
- **Complete.** Created the `render` module and moved its public types without
  retaining the old draw-type names.
- **Complete.** Renamed `Arg`/`VariableArg` to `StepArg`/`ParamArg` and updated
  public variants.
- **Complete at the time.** Kept `LinkageBuf` and `linkage_buf!` unchanged
  except for documentation. Step 7 deliberately supersedes the macro part of
  this decision while retaining `LinkageBuf`.

Focused tests, all-features rustdoc, and `cargo check-all` completed the
Step 2-specific validation. The remaining Armatron golden-image failure is
pre-existing: the same test fails in a clean `HEAD` checkout with the same
diagnostic.

### 3. Simplify evaluation and rendering APIs — Complete

- Added direct mark-pose lookup to `LinkageView`, integrated missing and
  ambiguous-name failures into `Error`, removed `MarkError`, and returned an
  opaque draw-item iterator. Focused tests cover successful, missing, and
  ambiguous mark names.
- Moved `Item3d`, `Stroke`, `Disk`, `Sphere`, and `Projection` genuinely into
  the public `render` module, and made projection an inherent method on
  `render::Item3d`.
- Removed the unused `DrawSurface`/`PixelSurface` API island and made
  `PenStyle` private.
- Removed the root `Point` and `Rgb565` re-exports while retaining `Rgb888`,
  `RgbColor`, and `WebColors` with Linkage Blaze-specific documentation.
- Restored Projection and public-method documentation for the resulting API.

Focused tests, all-features rustdoc, and `cargo check-all` pass. The updated
Armatron golden-image fixture is preserved.

### 4. Update every consumer — Complete

- Audited and updated the core crate, editor, ESP/RP/WASM examples, integration
  tests, `.lb.rs` macro consumers, generated-source expectations, and doctests
  for the finalized Step 2–3 API. The host BVH implementation now uses the
  contextual `Clip`, `Joint`, `Channel`, `Parameter`, and `ParameterLayout`
  names directly, and all rendering consumers use `render::*` paths.
- Searched the repository for the removed names and paths, including old BVH,
  argument, rendering, mark-error, surface, and root rendering names. No
  obsolete consumer references remain outside this historical review spec.

Formatting, all-features rustdoc, and `cargo check-all` pass.

### 5. Rewrite public documentation for the final API — Complete

- Applied the remaining public summary, field, variant, method, module, macro,
  and re-export documentation work after the API paths stabilized.
- Added a compile-checked crate Quick Start and goal-oriented links for asset
  files, platform examples, the live gallery, and Biovision Hierarchy data.
- Distinguished always-available `bvh::Motion` from feature-gated host parsing
  and conversion APIs, and expanded BVH as the Biovision Hierarchy
  motion-capture file format in public entry-point documentation.
- Removed robot-arm-only and “Logo-style” framing, clarified linkage-parameter
  terminology and CYD integration, and removed duplicate summaries.
- Made implementation-only example surfaces private where the consumer audit
  found no external API requirement: `examples::ui` and
  `armatron::reverse_kinematics`; the internal `StatusTextError` wrapper was
  removed. Public example modules and platform entry points remain available.
- Converted public examples to `rust,no_run` doctests where an executable
  example is appropriate. External `.lb.rs` and BVH asset inclusions are shown
  as explicitly labeled `text` excerpts because their call-site assets cannot
  be supplied to rustdoc.

All-features doctests and rustdoc pass, and `cargo check-all` passes.

### 6. Validate and review the generated result — Complete

- Formatting, focused all-features library tests, all-features doctests, and
  all-features rustdoc pass. The focused library suite reports 164 passing
  tests, and the doctest suite reports 22 passing tests.
- `cargo check-all` passes across core, embedded examples, editor, RP, ESP,
  and WASM targets, including the generated board examples and wasm-pack
  output.
- Inspected the generated crate index and the `bvh`, `render`, and `examples`
  module pages. The Quick Start and contextual links resolve, the finalized
  BVH and render items appear in their moved modules, and implementation-only
  example modules are absent from the public index.
- Compared the generated public item list with the resolved design: the
  always-available `bvh::Motion`, feature-gated host BVH items, `bvh::motion!`,
  render types, renamed argument types, and finalized root APIs are accounted
  for; removed names, root rendering paths, compatibility paths, and private
  helpers are absent.

### 7. Make `linkage_file!` the only public asset-loading macro

This follow-up deliberately amends the earlier decision to expose three ways
to load the same `.lb.rs` asset. `linkage_file!` already infers dimensions and
provides fixed, view, and feature-gated growable access, so it is the sole
documented and exported asset-loading macro.

Implementation work:

- Keep `LinkageFixed`, `LinkageView`, and `LinkageBuf` public. This change is
  about the redundant loading macros, not the storage types.
- Remove the exported `linkage_fixed!` and `linkage_buf!` macros. Do not leave
  deprecated, compatibility, or `#[doc(hidden)]` aliases under those names.
- Refactor `linkage_file!` so its generated `fixed()` and feature-gated `buf()`
  no longer expand either removed macro. Reuse its measured fixed value where
  practical and keep any exported expansion helper clearly implementation-only
  with an `__` prefix, `#[doc(hidden)]`, and a comment explaining why downstream
  macro expansion requires public visibility.
- Keep `linkage_file!` as the public entry point. Its generated module must
  continue to expose inferred `DOF`, `MARKS`, and `STEP_COUNT`; `Fixed`, `View`,
  and, with `alloc`, `Buf`; and `fixed()`, `view()`, and, with `alloc`, `buf()`.
- Replace direct tests of `linkage_fixed!` and `linkage_buf!` with tests through
  a `linkage_file!` declaration. Cover fixed, view, and growable access and the
  inferred dimensions without weakening existing compile-time checks.
- Update the crate documentation, README, doctests, examples, generated
  expectations, and all workspace consumers so they teach and use
  `linkage_file!`. Documentation for `linkage!` should say that `.lb.rs` files
  are loaded through `linkage_file!` only.
- Search the complete workspace for both removed macro names. After updating
  this spec's historical findings, neither name should remain in source,
  documentation, tests, templates, or generated expectations.

Validation:

- Run `cargo fmt --all`.
- Run focused tests covering `linkage_file!` with and without `alloc`, then the
  all-features library tests and doctests.
- Regenerate all-features rustdoc and inspect the crate index and macro list.
  `linkage_file!` and `linkage!` must remain visible; `linkage_fixed!` and
  `linkage_buf!` must be absent.
- Run `cargo check-all` and `git diff --check`. Do not suppress existing or new
  warnings.
- Modify only this repository and do not commit. Mark Step 7 complete only when
  all validation passes.

## Crate Landing Page

Decision: the crate's `index.html` does not currently provide a clear starting
path. The long `armatron1.lb.rs` fragment is marked `ignore`, does not include
its required context, and does not show how to evaluate or render the linkage.
The gallery proves what the project can do, but it does not teach the first API
step.

Work items:

- Add a short Quick Start near the beginning of the crate documentation, before
  the extended gallery and example material.
- Make the Quick Start a compilable `rust,no_run` doctest. It should construct a
  small `const` [`LinkageFixed`], obtain its [`LinkageView`], supply one animated
  parameter value, and evaluate a visible result such as the final [`Pose`].
- Keep the example small enough to reveal the core flow at a glance: define a
  linkage, choose parameter values, then evaluate it. Hide necessary
  boilerplate with rustdoc's `#` lines.
- Link directly from the Quick Start prose to the primary types and methods it
  uses. The example on [`LinkageView`] is a useful starting point, but the crate
  landing page must be understandable without first discovering that type.
- Follow the Quick Start with a compact set of goal-oriented links:
  `linkage_file!` for `.lb.rs` assets, the platform examples for rendering on
  ESP/RP/WASM, and the `bvh` module for Biovision Hierarchy motion-capture data.
- Either turn the current ignored Armatron fragment into a compilable example
  with its required macro context or present it explicitly as an asset-file
  excerpt after the Quick Start. Do not let an ignored, context-free fragment
  serve as the primary usage example.
- Ensure the landing page has one obvious link to the live interactive gallery
  and one obvious link to complete source examples, while keeping those links
  secondary to the compile-checked API introduction.

## Modules

### Step 1 findings: `bvh` and `bvh_parse`

The final public namespace is one always-present `linkage_blaze::bvh` module.
It contains allocation-free compile-time motion support unconditionally;
host-side parsing and conversion APIs are gated by `bvh`. `bvh_parse` is not a
final public module: its declarations are moved/re-exported into `bvh` or made
private.

#### Current `bvh` public items

| Current item | Final path/name | Decision and documentation work |
| --- | --- | --- |
| `BvhClip` and fields `joints`, `samples`, `sample_time` | `bvh::Clip` | Keep public. Describe a parsed Biovision Hierarchy clip containing its joint hierarchy, motion samples, and frame interval. Keep `channel_count` private unless implementation proves otherwise. |
| `BvhJoint` and all fields | `bvh::Joint` | Keep public. Explain parent index, model-space offset, ordered channels, and end-site representation. |
| `BvhChannel` and variants | `bvh::Channel` | Keep public. Describe position versus rotation channels and document every variant in reader-facing language. |
| `MotionSample` and `values` | `bvh::Sample` | Keep public. Describe one raw frame in channel order and state the physical units. |
| `BvhParameterLayout`, `parameters`, `len`, `is_empty` | `bvh::ParameterLayout` | Keep public. Explain the discovered mapping and document both query methods. |
| `BvhParameter` and all fields | `bvh::Parameter` | Keep public. Explain normalized linkage parameter name/index and source joint/channel. |
| `discover_bvh_parameters` | `bvh::discover_parameters` | Keep public. Say that it maps ordered BVH channels to linkage parameters and retains source mapping. |
| `build_bvh_linkage_buf` | `bvh::build_linkage_buf` | Keep public under host/`alloc` support. Explain capacities, defaults, named joint marks, and insufficient-capacity errors. |
| `bvh_sample_params` | `bvh::sample_params` | Keep public. Describe conversion of one raw sample into normalized linkage values. |
| `bvh_to_lb_rs` | `bvh::to_lb_rs` | Keep public as a host-side `.lb.rs` code-generation utility; explain that direct `Clip`/`Motion` use does not require it. |
| `parse_bvh` | `bvh::parse` | Keep public under the host feature. Say that it parses hierarchy and motion sections and returns `Clip`, with malformed-input diagnostics. |
| `Error` | `bvh::Error` | Keep public for host-side parsing, mapping, and conversion; document retained source diagnostics. |

The `bvh` summary must identify BVH as the **Biovision Hierarchy motion-capture
file format**, distinguish compile-time `Motion` from host-side `Clip` parsing,
and show the supported conversion path to a linkage.

#### Current `bvh_parse` public items

| Current item | Final decision |
| --- | --- |
| `bvh_motion!` | Keep as `bvh::motion!`; expose only a doc-hidden `__bvh_motion!` root helper required for downstream expansion. Remove the old public path. |
| `BvhMotion`, associated `DOF` and `SAMPLE_COUNT` | Keep as `bvh::Motion`, preserving const-generic dimensions, compile-time parsing, normalized `[0, 1]` values, and quantized `u16` storage. Keep the associated constants if they remain useful for generic downstream code; describe them as linkage-parameter and sample counts. |
| `BvhMotionSamples` | Make private and return it through `Motion::samples` as `impl Iterator<Item = [f32; DOF]>`. |
| `PARAM_CENTER_U16`, `norm_to_u16`, `u16_to_norm` | Make private representation helpers unless downstream custom quantization is demonstrated. |
| `parse_bvh_motion_section`, `parse_and_normalize_bvh_motion`, `normalize_bvh_motion`, `BvhNormalizePolicy` and its fields/`LINKAGE_BLAZE` constant | Make private. `Motion::from_bvh_bytes` is the supported entry point; do not expose the current fixed policy as a separate API without a concrete use case. |
| `parse_f32`, `parse_uint`, `scale_pow10`, `scale_pow10_f64`, `count_bvh_channels`, `parse_bvh_channel_is_position`, `skip_token`, `find_after`, `bytes_match`, `skip_whitespace`, `skip_inline_whitespace`, `skip_to_next_line` | Make private parser helpers; their current rustdoc is an implementation leak. |

`Motion::new`, `from_bvh_bytes`, `dof`, `sample_count`, `sample`, `samples`,
and `sample_into` remain public under `bvh::Motion`. Their docs must say
“linkage parameters” rather than rely on unexplained “degrees of freedom,” and
must state sample-index behavior and normalized-value units. The module docs
must explain compile-time parsing, normalization, fixed-size storage, and
`no_std` use, then contrast the optional host-side APIs.

### Step 1 findings: `examples`

Keep `linkage_blaze::examples` public. It is shared implementation consumed by
the ESP, RP, and WASM example crates, not a general-purpose application
framework. Its summary must be platform-neutral, identify Device Envoy CYD
display/touch traits as the integration boundary, and name the feature gates:
`examples-armatron`, `examples-ballet`, `examples-clock`, and
`examples-skeleton-clock`.

| Current module and public items | Final decision |
| --- | --- |
| `examples::armatron::{BACKGROUND_COLOR, FOREGROUND_COLOR, DOF, run, Error, Exit}` | Keep public. Document colors with approximate names, explain `DOF` as the linkage parameter count, and document run termination plus device/UI errors. |
| `examples::armatron::reverse_kinematics` | Make private. It is an internal Armatron subsystem and has no external consumer. |
| `examples::ballet::{ORIENTATION, TOP_FONT, BACKGROUND_COLOR, FOREGROUND_COLOR, run, StatusTextError, Error}` | Keep constants, `run`, and `Error` public. Make `StatusTextError` private or an internal error detail; the standalone wrapper is not a user-facing concept. Rewrite docs around the motion-captured renderer and flush errors. |
| `examples::clock::{BACKGROUND_COLOR, FOREGROUND_COLOR, ORIENTATION, WIFI_STATUS_FONT, WIFI_STATUS_RECTANGLE, MAX_FRAME_PIXEL_COUNT, run, splash, Exit, Error}` | Keep public because platform launchers consume them. Document each layout/configuration constant and the run/splash lifecycle. |
| `examples::skeleton_clock::{BACKGROUND_COLOR, FOREGROUND_COLOR, ORIENTATION, TOP_FONT, WIFI_STATUS_RECTANGLE, FIGURE_TILE_GRID, run, splash, Exit, Error, MarkLookupError}` | Keep configuration, `run`, `splash`, `Exit`, and `Error` public. Remove `MarkLookupError` when mark lookup moves to `LinkageView`/the primary `Error`; it is an implementation wrapper. |
| `examples::ui::{UiState, UiFrame, Slider, Button, IconButton, Icon, HoldButtonState, Label, Error}` | Make `examples::ui` and these widget types private: source search finds no external consumer and they support only Armatron. If a downstream use is found during implementation, revisit this one decision before changing visibility. |

For retained items, replace vague or duplicate descriptions and remove “owned
CYD parts” as unexplained terminology. Prefer private visibility over
`#[doc(hidden)]`; do not add compatibility aliases. The `ui` visibility and
`StatusTextError` visibility are the only genuinely consumer-dependent
decisions found in this Step 1 pass.

The public methods currently shown beneath `examples::ui` are included in the
same private-API decision: `UiState::new`; `UiFrame::{new, slider, button,
icon_button, hold_button, label, draw_touch_cursor}`; `Slider::{horizontal,
vertical, column, label}`; `Button::{new, touch_rectangle}`;
`IconButton::new`; and `Label::new`. The public enum variants
`Icon::{Play, Stop, StepForward}` and
`HoldButtonState::{Idle, Pressed, Held}` are likewise implementation details
unless the module's visibility is reopened.

## Macros

### `bvh_motion!`

Decision: keep the compile-time inclusion macro public, but move its canonical
public path into the redesigned BVH module. Under the repository's contextual
naming convention, prefer `bvh::motion!` over either the root-level
`bvh_motion!` or the redundant `bvh::bvh_motion!`.

Stable `macro_rules!` macros exported for downstream crates originate in the
crate root, but the desired public path is still possible: export a doc-hidden
root helper such as `__bvh_motion!` and publicly re-export it as `motion!` from
the `bvh` module. Do not retain `bvh_motion!` as a compatibility alias.

Work items:

- Move the canonical macro path to `linkage_blaze::bvh::motion!` as part of the
  BVH namespace redesign.
- Rename the exported root implementation macro with the `__` prefix, hide it
  from public documentation, and explain that it must be public for downstream
  macro expansion.
- Update all internal, workspace, test, and documentation call sites to the new
  path.
- Rewrite the index summary to say that the macro embeds, parses, and
  normalizes a BVH (Biovision Hierarchy) motion-capture file at compile time.
- Document the resulting fixed-size, quantized motion representation and link
  directly to its primary type and example.

### `linkage!`

Decision: keep this macro public under its current name. Authors see and use it
inside `.lb.rs` asset files, and generated `.lb.rs` source emits it. The current
"callback macro" summary describes its implementation role instead of its
purpose for readers.

Work items:

- Rewrite the index summary to say that `linkage!` defines the linkage
  expression inside a `.lb.rs` asset file.
- Keep the detailed documentation explaining that callers normally load the
  file through `linkage_file!`, with the callback mechanics presented as
  secondary implementation context.

### `linkage_buf!`

Amended decision: remove this standalone macro from the public API. Keep
[`LinkageBuf`] public and expose growable loading through the feature-gated
`buf()` generated by [`linkage_file!`]. The separate expression macro adds a
second route to the same asset without a demonstrated consumer need.

Work item:

- Make the hidden `linkage_file!` expansion helper construct [`LinkageBuf`]
  directly, then remove `linkage_buf!` and its direct tests and documentation.

### `linkage_file!`

Decision: keep this macro public under its current name. It is the primary API
for declaring a named linkage asset, and its name reflects the external
`.lb.rs` file it consumes. The summary is basically accurate but does not tell
readers why they would choose it.

Work item:

- Rewrite the index summary to identify this as the primary way to declare a
  named `.lb.rs` linkage asset and mention that it generates a Rust module with
  fixed, view, and optional owned access.

### `linkage_fixed!`

Amended decision: remove this standalone macro from the public API. Keep
[`LinkageFixed`] public and expose fixed loading through the `fixed()` generated
by [`linkage_file!`]. The named declaration infers all dimensions and is clearer
than requiring callers to repeat parameter and mark counts.

Work item:

- Make `linkage_file!` produce `fixed()` without expanding `linkage_fixed!`,
  then remove `linkage_fixed!` and its direct tests and documentation.

## Structs

### Core linkage representations

#### `LinkageFixed`

Decision: keep this public and top-level under its current name. It is the
primary allocation-free, compile-time representation and follows the
repository's `Fixed`/`View` convention.

Work item:

- Replace "expression/storage type" in the index summary with a direct
  description such as "An allocation-free linkage with compile-time-fixed
  capacities." Link to the crate Quick Start.

#### `LinkageView`

Decision: keep this public and top-level under its current name. It is the
canonical evaluation interface and erases only the fixed step capacity while
preserving parameter and mark dimensions.

Work item:

- Replace "canonical operational API" with reader-facing language: a borrowed
  linkage view used to evaluate, compose, and render a linkage. Mention that
  [`LinkageFixed::view`] and the owned form produce it.

#### `LinkageBuf`

Decision: keep the `alloc`-gated growable representation public and top-level
under its current name. `Buf` has familiar standard-library precedent for an
owned, growable buffer and distinguishes this type from both the fixed-capacity
owner and the borrowed view. `LinkageOwned` would be less precise because
[`LinkageFixed`] also owns its storage.

Work item:

- Replace "expression/storage type" with a description of the `alloc`-enabled,
  growable linkage buffer used for runtime parsing and construction, and link
  directly to conversion from [`LinkageFixed`].

### Parameters and evaluated state

#### `Param`

Decision: keep this public and top-level under its current name because
[`LinkageView::param`] and [`LinkageView::params`] expose it.

Work item:

- Change the summary from "runtime linkage parameter" to "A linkage parameter
  definition with a display name and normalized default value." The runtime
  value is supplied separately during evaluation.

#### `VariableArg`

Decision: it is currently public because public [`Arg`] and [`Step`] variants
contain it, but its name and standalone top-level presence should be reviewed
with those enums. It represents a parameter reference plus an operation range,
not a general variable argument.

Work items:

- During the enum review, decide whether to inline these fields into the public
  variants or rename the type to the more specific `ParamArg`.
- If it remains a type, keep it top-level beside [`Arg`] and describe the
  normalized parameter reference and operation-value range without relying on
  unexplained "degree-of-freedom" terminology.

#### `Pose`

Decision: keep this public and top-level under its current name. It is a core
evaluation result used throughout the API.

Work item:

- Expand the summary to say that a pose contains a 3D position and local-frame
  orientation after evaluating a linkage step.

#### `StyledPose`

Decision: keep this public because [`LinkageView::styled_poses`] yields it.
Keep it top-level beside [`Pose`] unless the later rendering review introduces
a cohesive public rendering module for all pen-aware evaluation output.

Work item:

- Replace "Full pose plus Logo-style pen state" with a direct description of a
  [`Pose`] plus the pen state, color, and width active after a linkage step.

#### `PenStyle`

Decision: remove this from the public API. No public method accepts or returns
`PenStyle`; [`StyledPose`] exposes its useful values individually.

Work item:

- Make `PenStyle` private and keep only the visibility needed by internal
  evaluation. Do not hide the public type with a documentation attribute.

### Drawing output

#### `DiskItem`, `SphereItem`, and `StrokeSegment`

Decision: keep these public because they are payloads of public [`DrawItem3d`]
variants. They should not occupy the crate root independently of the enum that
gives them meaning.

Work items:

- Design a small drawing-output namespace containing [`DrawItem3d`],
  `DiskItem`, `SphereItem`, `StrokeSegment`, and the draw-item iterator. Use
  contextual names inside that module where they improve clarity, without
  retaining duplicate root re-exports.
- Make the three summaries parallel: a stroke, disk, or sphere emitted while
  evaluating a linkage, including the geometry and drawing style each carries.

#### `DrawItem3dIter`

Decision: the iterator must currently be public because
[`LinkageView::draw_items_3d`] names it and its
[`DrawItem3dIter::pose_by_mark_name`] method exposes additional behavior. It
does not belong at the crate root, and the generated summary currently contains
two duplicated descriptions.

Work items:

- Move mark-pose lookup to a direct [`LinkageView`] operation if practical, so
  `draw_items_3d` can return an opaque iterator and `DrawItem3dIter` can become
  private.
- If the concrete iterator remains public, place it in the drawing-output
  namespace and choose a concise contextual name.
- Remove the duplicate summary and describe what the iterator yields, how it is
  obtained, and whether mark lookup requires exhausting it.

### Geometry and rendering

#### `Mat3` and `Vec3`

Decision: keep these public and top-level. [`Pose`] exposes them directly, and
they are small core geometry values rather than optional rendering adapters.
Their concise names are conventional and unambiguous in this crate.

Work items:

- Describe `Mat3` first as the local-frame orientation matrix used by a
  [`Pose`], with row-major storage and axis-column details following afterward.
- Describe `Vec3` as a 3D position or direction vector stored as `[x, y, z]`.

#### `Projection`

Decision: keep this public, but move it out of the crate root with the drawing
and rendering APIs. It is needed by the shared platform examples, but it is not
part of defining or evaluating a linkage.

Work items:

- Place `Projection` in the chosen rendering namespace without a duplicate root
  re-export.
- Rewrite the summary in plain language: a camera projection that maps Linkage
  Blaze 3D coordinates to 2D pixel coordinates, with orthographic and
  perspective constructors.

#### `PixelSurface`

Decision: remove this from the public API unless a real external rendering
entry point is found during implementation. It and [`DrawSurface`] form an
otherwise unused API island; no repository consumer uses either one.

Work item:

- Delete or inline `PixelSurface` and [`DrawSurface`] rather than preserving an
  unused abstraction. If an external use case is established, place the pair
  in the rendering namespace and document the complete call path that consumes
  them.

### Embedded Graphics re-exports

#### `Rgb888`

Decision: keep this public and top-level. It is the color type used directly by
the linkage DSL, drawing outputs, and generated `.lb.rs` assets.

Work item:

- Give the re-export a Linkage Blaze-specific summary identifying it as the RGB
  color type used by pen and shape operations.

#### `Point` and `Rgb565`

Decision: remove these convenience re-exports from the crate root. `Point` is
only needed by the optional projection API, and `Rgb565` is not part of a
Linkage Blaze public signature. Their generic upstream summaries make them look
like core crate types.

Work items:

- Use `embedded_graphics::Point` from the rendering namespace and examples
  instead of re-exporting `Point` at the crate root.
- Import `Rgb565` directly from `embedded_graphics` wherever examples or tests
  need it.

## Enums

Do not describe the public drawing model as "Logo-style." Normal API readers
only need the direct concepts: movement, pen state, color, width, and emitted
geometry. Remove that phrase from enum summaries and from related public struct
and method documentation.

### `Step`

Decision: keep this public and top-level. It is the core instruction type stored
by every linkage. The current "robot arm linkage" summary incorrectly narrows
Linkage Blaze to one example application.

Work items:

- Describe `Step` as one movement, drawing, shape, mark, or restore operation in
  a linkage definition.
- Keep the coordinate-system explanation, but link to the crate-level
  coordinate-system section instead of repeating unexplained conventions where
  practical.
- Review every variant description for reader-facing terminology. Replace
  "degree-of-freedom parameter" with "linkage parameter" unless the distinction
  is necessary and explained.

### `Arg`

Decision: keep the fixed-or-parameter-driven concept public because public
[`Step`] variants expose it, but reconsider both its generic top-level name and
its representation together with `VariableArg`.

Work items:

- Prefer a self-explanatory root name such as `StepArg`, or place a contextual
  `Arg` in a step-focused module if that module remains coherent after the full
  review. Do not retain both paths.
- Describe it as either a fixed operation value or a value interpolated from a
  normalized linkage parameter.
- Add descriptions for the `Fixed` and parameter-driven variants, including
  the units used by rotation and translation steps.
- Decide whether the parameter-driven fields should be inlined into this enum
  or represented by the proposed `ParamArg` type from the struct review.

### `DrawItem3d`

Decision: keep this public because renderers and the browser editor consume the
evaluated stroke, disk, and sphere variants. Move it out of the crate root with
its payload structs and iterator.

Work items:

- Place it in the drawing-output namespace chosen during the struct review,
  using a concise contextual name such as `draw::Item3d` if that improves the
  complete path.
- Describe it as 3D geometry emitted while evaluating a linkage, then list the
  supported stroke, disk, and sphere forms.
- Link each variant to its payload documentation and ensure each payload's
  geometry, color, and width or radius are clear.

### `Error`

Decision: keep one primary top-level `Error` for core linkage operations. Its
current summary is accurate only for evaluation and will need to broaden if
mark lookup is moved onto [`LinkageView`].

Work items:

- Preserve the useful parameter index and value in `InvalidParameter` and keep
  `EmptyLinkage` diagnostic.
- If mark lookup becomes a [`LinkageView`] operation, add clear not-found and
  ambiguous-mark variants to this `Error` rather than maintaining a second
  top-level error enum.
- Update the summary to cover the actual operation family after the final
  variants are known, and document every variant's trigger and retained
  diagnostic information.

### `MarkError`

Decision: remove this top-level enum if the planned mark-lookup redesign is
implemented. It exists only for one method on `DrawItem3dIter`, and both the
method placement and separate error type fragment the primary evaluation API.

Work item:

- Move mark lookup onto [`LinkageView`] and represent not-found and ambiguous
  names in the primary [`Error`]. If implementation evidence requires a
  separate error, place it beside the lookup API rather than at the crate root
  and give both variants full descriptions.

### `PenState`

Decision: keep this public because [`StyledPose::pen`] returns it. Keep it
top-level beside [`StyledPose`] unless all pen-aware evaluation output moves
into the drawing namespace.

Work items:

- Replace "Logo-style pen state" with a direct summary such as "Whether linkage
  movement currently emits strokes."
- Document `Up` as movement without stroke emission and `Down` as movement with
  stroke emission.

## Traits

### `DrawItem3dExt`

Decision: remove this public trait. It is implemented only for [`DrawItem3d`]
and exists solely to add one `project` method, so it is not a meaningful
extension point. Its top-level name and CYD/Device Envoy-oriented summary also
expose rendering integration before readers understand the core linkage API.

Work items:

- Inline `project` as an inherent method on the renamed/moved 3D draw-item enum.
- Place the method and [`Projection`] in the rendering namespace selected by
  the struct and enum reviews.
- Describe the method as projecting one Linkage Blaze 3D draw item into a
  Device Envoy 2D draw item, with a direct link to that output type and a small
  rendering example.
- Update the shared examples to call the inherent method without importing an
  extension trait.

### `DrawSurface`

Decision: remove this public trait together with `PixelSurface`. No repository
API consumes a `DrawSurface`, and the only implementation is the otherwise
unused `PixelSurface`; the pair is an unconnected abstraction rather than a
usable rendering entry point.

Work item:

- Delete or inline `DrawSurface` and `PixelSurface`. Do not retain public or
  doc-hidden compatibility artifacts. If implementation discovers a necessary
  rendering path, design and document that complete path instead of preserving
  the current isolated trait.

### `RgbColor`

Decision: keep this Embedded Graphics trait publicly re-exported at the crate
root beside `Rgb888`. Consumers of linkage draw items use it to read RGB
channels and access basic RGB constants without needing a separate import path.
The upstream summary "RGB color" does not explain why it appears here.

Work item:

- Add a Linkage Blaze-specific re-export summary identifying `RgbColor` as the
  channel-access and basic-color trait for the public `Rgb888` pen and shape
  color type.

### `WebColors`

Decision: keep this Embedded Graphics trait publicly re-exported at the crate
root beside `Rgb888`. Its CSS named-color constants are part of the documented
`.lb.rs` asset syntax and are imported by `linkage_file!` expansions.

Work items:

- Add a Linkage Blaze-specific re-export summary identifying `WebColors` as the
  source of `Rgb888::CSS_*` constants used by linkage color operations.
- Ensure public `.lb.rs` examples show the required trait import when they use a
  CSS named color, while macro-generated internal imports remain hidden as
  boilerplate.
