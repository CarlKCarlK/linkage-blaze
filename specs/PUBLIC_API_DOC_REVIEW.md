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
- Keep `LinkageBuf` and `linkage_buf!`. `Buf` distinguishes growable,
  allocator-backed storage from the fixed owner and borrowed view.
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

- Change the local documentation recipe to build the same all-features public
  surface configured for docs.rs rather than only `alloc`.
- Regenerate rustdoc and review the newly visible `bvh` and feature-gated
  `examples` module contents before changing the API.
- Add decisions and work items for any public items missing from this review.
  Do not begin structural implementation until this pass is complete.

### 2. Establish the final module structure and names

- Create the always-present `bvh` facade, move compile-time motion support and
  host-only support behind it, and establish contextual public names.
- Create the `render` module and move its public types without root aliases.
- Rename `Arg`/`VariableArg` to `StepArg`/`ParamArg` and update public variants.
- Keep `LinkageBuf` and `linkage_buf!` unchanged except for documentation.

### 3. Simplify evaluation and rendering APIs

- Add direct mark-pose lookup to `LinkageView`, integrate its failures into
  `Error`, remove `MarkError`, and return an opaque draw-item iterator.
- Inline `DrawItem3dExt::project` as an inherent method on `render::Item3d`.
- Remove the unused `DrawSurface`/`PixelSurface` API island and make
  `PenStyle` private.
- Remove the root `Point` and `Rgb565` re-exports while retaining `Rgb888`,
  `RgbColor`, and `WebColors` with Linkage Blaze-specific documentation.

### 4. Update every consumer

- Update the core crate, editor, ESP examples, RP examples, WASM examples,
  integration tests, `.lb.rs` macro paths, generated-source expectations, and
  doctests in one coordinated migration.

### 5. Rewrite public documentation for the final API

- Apply every summary and visibility work item in this review after names and
  module paths are stable.
- Add the compile-checked crate Quick Start and goal-oriented navigation.
- Update all top-level BVH wording to identify the Biovision Hierarchy
  motion-capture file format.
- Remove "Logo-style," robot-arm-only framing, unexplained degree-of-freedom
  terminology, duplicate summaries, and ignored examples serving as primary
  guidance.

### 6. Validate and review the generated result

- Run formatting, focused tests, doctests, and checks for each affected feature
  set while implementing.
- Run `cargo check-all` for Linkage Blaze before completion.
- Regenerate all-features rustdoc, inspect the crate index and moved modules,
  and verify that links and examples resolve.
- Compare the final public item list against this review and account for every
  addition, removal, rename, and relocation.

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

### `bvh_parse`

Decision: the compile-time BVH motion functionality is public API, but
`bvh_parse` is an implementation-oriented public module name and its one-line
description does not explain the API's normalization and compact storage.

Work items:

- Design one coherent public BVH namespace for the always-available,
  allocation-free motion API and the `bvh`-feature host APIs. Prefer a
  user-facing concept such as `bvh` or `bvh_motion` over the implementation
  action `bvh_parse`, resolving the existing `bvh` module rather than adding a
  second redundant API path.
- Review every currently public item in `bvh_parse`. Keep user-facing motion
  types and operations public, but reduce parser, normalization, and encoding
  implementation details to crate visibility unless users need them directly.
- Preserve a valid downstream expansion path for `bvh_motion!`. If a public
  macro helper must remain public solely for macro expansion, use the
  repository's documented `__` naming and `#[doc(hidden)]` macro-helper
  exception.
- Replace the module summary with documentation that identifies BVH as the
  Biovision Hierarchy motion-capture format and explains compile-time parsing,
  normalization, `u16` quantization, fixed-size storage, and `no_std` use.
- Explain how the always-available compile-time motion API differs from the
  optional host-side APIs enabled by the `bvh` feature.

### `examples`

Decision: keep `examples` public under its current name. The ESP, RP, and WASM
example crates consume this shared implementation across crate boundaries, and
the name accurately describes its role. Its current description is too
CYD-centric and does not explain its feature-gated contents or consumers.

Work items:

- Rewrite the module summary as platform-neutral example application logic
  shared by the ESP, RP, and WASM example crates.
- Explain that each example submodule is opt-in and name the corresponding
  feature: `examples-armatron`, `examples-ballet`, `examples-clock`, and
  `examples-skeleton-clock`.
- Clarify that the examples render through Device Envoy's CYD display and touch
  abstractions without requiring readers to understand the internal phrase
  "owned CYD parts."

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
  file through `linkage_file!`, `linkage_fixed!`, or `linkage_buf!`, with the
  callback mechanics presented as secondary implementation context.

### `linkage_buf!`

Decision: keep this `alloc`-gated macro public under its current name. It
matches [`LinkageBuf`], the growable, allocator-backed representation. The
current summary is accurate but does not distinguish that storage from the
fixed form.

Work item:

- State in the index summary that the macro requires `alloc` and includes a
  `.lb.rs` asset as a growable [`LinkageBuf`] expression.

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

Decision: keep this macro public under its current name. It matches
`LinkageFixed`, and the current summary clearly says that it includes a
`.lb.rs` file as a fixed linkage expression. No top-level API work item is
needed unless the later `LinkageFixed` review changes the type name or role.

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
