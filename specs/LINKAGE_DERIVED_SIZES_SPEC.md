<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Linkage Derived Sizes and Type-Erased Views

## Goal

Change the normal `LinkageFixed` construction API so application authors do not guess or manually maintain linkage capacities.

The implementation must:

- measure imported `.lb.rs` programs during const evaluation;
- allocate exact fixed storage from the measured facts;
- derive the step capacity of transformations that change the number of steps;
- keep the concrete `LinkageFixed<DOF, MARKS, N>` representation available internally;
- expose final programs as promoted `LinkageView`s so runtime callers do not need to name `N`;
- preserve stable Rust, `no_std`, no allocation, and `#![forbid(unsafe_code)]`.

This is the linkage equivalent of `Playable<SAMPLE_RATE>` hiding an audio buffer's total sample count: `LinkageView<DOF, MARKS>` preserves the constants meaningful to the caller while hiding storage length `N`.

## Terminology and Non-Goals

In the current API, `N` is the step-slot capacity of `LinkageFixed<DOF, MARKS, N>`. The value also stores `len`, the number of occupied step slots. This work removes manually maintained `N` values from normal application code; it does not remove `len()`, `step_count()`, fixed arrays, or const generics from the implementation.

`DOF` is semantically important because evaluation accepts `[f32; DOF]`. `MARKS` describes a meaningful program facility and lets iterators use fixed mark-state storage. Users may and should continue supplying `DOF` and `MARKS`; compile-time validation catches mistakes. Do not add machinery merely to hide either value.

Do not:

- add a build script or procedural macro;
- require nightly Rust or `generic_const_exprs`;
- change `.lb.rs` files to contain manually maintained metadata;
- parse Rust source text;
- add allocation to `linkage-blaze-core`;
- replace `LinkageFixed` with a recursive type-level list;
- remove `draw_item_3d_count()` or `len()`; both remain useful facts;
- erase `DOF` or `MARKS` from `LinkageView`;
- add compatibility aliases or retain redundant numeric macro forms.

## Evaluate a `?Sized` Step Tail First

Before adding or expanding the view machinery, perform a small stable-Rust feasibility spike for the same representation technique used by audio buffers.

The promising shape is conceptually:

```rust
pub struct LinkageStorage<
    const DOF: usize,
    const MARKS: usize,
    Steps: ?Sized = [Step],
> {
    params: [Param; DOF],
    param_len: usize,
    mark_names: [&'static str; MARKS],
    mark_len: usize,
    len: usize,
    steps: Steps,
}

pub type LinkageFixed<const DOF: usize, const MARKS: usize, const N: usize> =
    LinkageStorage<DOF, MARKS, [Step; N]>;

pub type LinkageView<'a, const DOF: usize, const MARKS: usize> =
    &'a LinkageStorage<DOF, MARKS, [Step]>;
```

Names may change to avoid colliding with the existing `Linkage` trait. The requirements are:

- the possibly unsized `steps` field is last;
- fixed construction and transformations operate on `[Step; N]`;
- runtime methods operate through `Steps: ?Sized + AsRef<[Step]>`, or another safe stable bound shared by arrays and slices;
- `.view()` performs the safe array-to-slice unsizing coercion;
- the view retains `DOF` and `MARKS` but erases `N`;
- the implementation uses no `unsafe`;
- `len` continues to select active steps if a low-level fixed value has spare capacity.

The spike must compile a const-constructed fixed value, coerce it through `.view()`, promote a temporary fixed value to a `'static` view, and run at least `len()`, `poses()`, and `draw_items_3d()`.

Adopt this representation if it removes the separate field-by-field `LinkageView` wrapper and its conversion boilerplate without making fixed transformations or trait implementations substantially harder. If it does, update the rest of this spec so `LinkageView` is a borrowed unsized-storage alias while preserving the public `Fixed`/`View` terminology and `.view()` conversion.

Do not adopt it merely for novelty. Keep the existing explicit `LinkageView` struct if custom-DST coercion creates awkward bounds, trait conflicts, worse diagnostics, or additional public complexity. Record the spike result in the implementing change rather than leaving both representations in the API.

## Current Problems

Normal application code currently contains declarations such as:

```rust
const CAMERA_CONTROL: LinkageFixed<3, 1, 8> =
    linkage_fixed!("../../assets/examples/armatron/camera_control.lb.rs");
const GRID_9X9: LinkageFixed<0, 1, 81> =
    linkage_fixed!("../../assets/examples/armatron/grid_9x9.lb.rs");
const CAMERA_AND_GRID: LinkageFixed<3, 2, 88> = CAMERA_CONTROL.combine(GRID_9X9);
```

The programmer must copy or guess `N`, then update downstream capacity arithmetic after editing either input. Supplying `DOF` and `MARKS` is acceptable because they describe the program rather than its backing storage.

Other transformations expose the same problem:

```rust
const WITH_JOINTS: LinkageFixed<6, 1, 45> = ARM.with_joint_spheres(0.15);
const OPTIMIZED: LinkageFixed<3, 6, 385> = SOURCE.compact();
```

Some call sites additionally state output step capacities as turbofish arguments:

```rust
source.compact::<385>();
```

The runtime API already contains the right final abstraction: `LinkageView<DOF, MARKS>` erases `N` while retaining the two meaningful constants. The missing pieces are automatic step counting and ergonomic capacity-changing transformations.

## Desired Application API

Exact spelling may be adjusted only when Rust macro parsing requires it. Preserve these semantics and keep the final syntax comparably compact.

### Imported fixed programs

Change the expression macro to accept the two meaningful dimensions and discover only its step capacity:

```rust
let camera_control =
    linkage_fixed!("../../assets/examples/armatron/camera_control.lb.rs", 3, 1);
```

The arguments are `DOF` and `MARKS`. The returned concrete type is `LinkageFixed<3, 1, N>`, where the macro measures `N` from the file.

Remove the existing form that accepts `N`:

```rust
linkage_fixed!(path, dof, marks, n)
```

The canonical expression form is `linkage_fixed!(path, dof, marks)`. Materialization validates the supplied `DOF` and `MARKS` at compile time.

### Named fixed intermediate programs

Rust requires a type on a `const` item, even when an expression's type is inferable. Provide an item macro that generates the exact concrete type:

```rust
linkage_fixed_const! {
    const CAMERA_CONTROL =
        linkage_fixed!("../../assets/examples/armatron/camera_control.lb.rs", 3, 1);
}
```

The item macro must accept:

- ordinary attributes, including documentation;
- `pub`, restricted visibility, or private visibility;
- a constant name;
- a const-evaluable expression returning an exactly sized `LinkageFixed`;
- an optional trailing comma or semicolon, whichever grammar is chosen and documented.

It must expand to a named `const`, not a `static`.

### Derived transformations

Provide canonical macros for transformations whose output const arguments cannot be expressed by stable Rust method return types:

```rust
linkage_fixed_const! {
    const CAMERA_AND_GRID = linkage_combine!(CAMERA_CONTROL, GRID_9X9);
}

linkage_fixed_const! {
    const ARM_WITH_JOINTS = linkage_with_joint_spheres!(ARM, 0.15);
}

linkage_fixed_const! {
    const OPTIMIZED = linkage_compact!(SPECIALIZED);
}
```

These macros derive step capacity. `linkage_combine!` may also derive output `DOF` and `MARKS` by adding the inputs because that arithmetic is direct and already validated by `combine`; this is a convenience, not a requirement to erase those values from the final view.

Keep the fluent fixed-size methods as implementation primitives only if exported macros require them across crate boundaries. If so, mark them as public macro helpers using the repository's `#[doc(hidden)]` exception, prefix non-user-facing helper names with `__`, and explain why they must be public. Do not leave two documented, equally recommended ways to perform the same operation.

### Final runtime view

Use the existing view type at the runtime boundary:

```rust
const LINKAGE: LinkageView<3, 6> = linkage_compact!(SPECIALIZED).view();
```

Use the repository's actual `LinkageView` generic order. The macro must hide the backing `N`; the user-facing declaration must not contain `LinkageFixed`, `N`, a backing `static`, or a separately named backing const.

`DOF` and `MARKS` remain visible, just as an audio `Playable<SAMPLE_RATE>` retains its meaningful sample rate while hiding its total sample count. Const promotion supplies backing storage for the view. Use a named `static` only when address identity is part of program behavior; linkage views need lifetime, not pointer identity.

## Compile-Time Measurement Design

### Preserve the `.lb.rs` format

An `.lb.rs` file must continue to contain only the existing `linkage![...]` fluent chain. Do not add a comment or header that must agree with the body. Existing generated and hand-edited files must gain automatic sizing without storing redundant counts.

### Make `linkage!` dispatch the complete chain

Currently `linkage!` calls a locally defined `__linkage_blaze_start!()` helper and then appends the chain. Change the private dispatch contract so the including macro receives the complete chain and may evaluate it more than once.

Conceptually:

```rust
macro_rules! linkage {
    ($($chain:tt)*) => {
        __linkage_blaze_build!($($chain)*)
    };
}
```

`linkage_fixed!` defines `__linkage_blaze_build!` for measured fixed construction. `linkage_buf!` defines it for ordinary growable construction. Keep the helper hygienic and local to the expansion, following the approach already used by the repository.

### Add a const measurement builder

Add a private `LinkageStepCount` value with const-evaluable fluent methods matching every operation permitted inside `linkage![...]`.

It records only:

- `step_count`: one implicit `Start` plus one for every DSL call that emits a `Step`.

`define_param` does not emit a step and therefore leaves the count unchanged. Every fixed or parameterized motion, drawing, pen, mark, and restore operation increments the count once. The counter ignores argument values and names. The real materialization pass remains responsible for semantic validation, ranges, parameter lookup, mark lookup, and restore ordering.

Do not hand-maintain two unrelated lists of fluent operations. Extend the existing DSL method-generation macros, or introduce a shared operation inventory from which both `LinkageFixed` and `LinkageStepCount` methods are generated. Adding a future DSL operation must either update both implementations from one macro entry or fail a compile-time coverage test.

### Materialize once with supplied semantic dimensions

`linkage_fixed!(path, DOF, MARKS)` uses two stages:

1. Run the chain on `LinkageStepCount` to discover exact `N`.
2. Materialize and validate the chain as `LinkageFixed<DOF, MARKS, N>`.

After materialization, assert that `param_count() == DOF` and `mark_count() == MARKS`. A supplied value that is too small already fails while filling storage; a supplied value that is too large must also fail rather than leaving empty semantic slots.

Both stages occur during compilation. Only the final fixed value may be retained in the binary.

### Validation

The materialization pass must preserve all current compile-time failures, including:

- too many or too few parameters for a declared representation;
- undefined parameter names;
- ambiguous name-based freezing;
- invalid normalized defaults and fixed values;
- restoring an undefined or not-yet-defined mark;
- duplicate retained parameter indexes;
- output transformations that disagree with their derived counts.

Automatic sizing removes capacity mistakes; it must not weaken semantic checks.

## Derived Transformation Design

Stable Rust cannot generally write a method return type containing expressions such as `N + N2` or a value discovered by running an optimizer. Keep the low-level const-generic implementation and put the derivation in exported declarative macros.

Macros may use `_` for inferable input const arguments and supply only derived output arguments where the underlying generic order permits it. A stable-Rust feasibility check has confirmed that inferred const generic placeholders such as `::<_, 3>` compile; add an in-repository test rather than relying only on this note.

### Combine

`linkage_combine!(first, second)` derives:

- output parameters: `first.param_count() + second.param_count()`;
- output marks: `first.mark_count() + second.mark_count()`;
- output steps: `first.step_count() + second.step_count() - 1`, because the second implicit `Start` is skipped.

It calls the existing const combination logic and returns an exactly sized `LinkageFixed`.

The macro may evaluate const expressions more than once. Document that operands must be const-evaluable linkage expressions. Do not allow runtime side effects or runtime-only values.

### Joint spheres

Add a hidden const helper that counts how many output steps `with_joint_spheres` will emit. `linkage_with_joint_spheres!` uses that count as `N_OUT`, calls the existing transformation, and returns exact storage.

The count must follow the transformation itself: every move-like step that receives endpoint spheres contributes the same number of steps counted by the materializer. Test fixed and parameterized `Move`, `Left`, and `Up`.

### Compaction

Add a hidden const helper that runs the same no-op stripping and adjacent-fixed-step merging pipeline as `compact`, then returns the exact remaining step count.

`linkage_compact!` uses the discovered count as `OUT_N`. Do not duplicate a simplified counting algorithm that can drift from the optimizer.

### Parameter specialization

Deriving specialization `DOF` values is outside this change. Users may continue supplying meaningful output `DOF` values where stable Rust cannot infer them. Preserve the existing compile-time validation for missing or ambiguous names, bad indexes, duplicate retained indexes, and mismatched output dimensions.

Do not introduce freeze or retain macros merely to hide `DOF`.

## Declaration Macro Implementation Notes

Rust requires a type annotation on a named `const`, so expression inference alone cannot solve named intermediate programs. `linkage_fixed_const!` should derive its generated type from the expression:

```rust
LinkageFixed<
    { EXPRESSION.param_count() },
    { EXPRESSION.mark_count() },
    { EXPRESSION.step_count() },
>
```

The initializer is the same expression. Repeated const evaluation is acceptable; repeated runtime evaluation is not.

Macro expressions may be evaluated several times by the compiler. Mention this in macro documentation, but do not expose temporary measurement constants in the public API or generated documentation.

## Migration

Migrate production examples before broad test cleanup so the desired API is exercised by real code.

### Armatron

Update `crates/linkage-blaze-core/src/examples/armatron/main.rs` so:

- imported camera, grid, and arm linkages use automatically measured declarations;
- `with_joint_spheres` derives its output size;
- every `combine` derives all output sizes;
- the final program is a promoted `LinkageView`;
- `ARM_TIP_LINKAGE` follows the same pattern;
- no linkage step capacity appears as a numeric literal.

Preserve the current reuse of `CAMERA_CONTROL`, `ARMATRON1`, and `SCENE_WITH_ARM`. Do not recompute a logically shared program at runtime.

### Skeleton Clock

Update `crates/linkage-blaze-core/src/examples/skeleton_clock.rs` so:

- the imported pirouette linkage does not state `600`;
- style composition derives its output;
- freezing and retaining may continue to state meaningful `DOF` values when inference requires them;
- compaction discovers its exact final count instead of stating `385`;
- the final runtime value remains a view;
- the runtime `heapless::Vec` continues to use `{ LINKAGE.draw_item_3d_count() }`.

The last item is an example of deriving runtime capacity from compile-time content and is intentionally retained.

### Ballet and Clock

Remove manual linkage capacities and explicit `combine` output turbofish arguments. Use promoted final views.

### Tests and documentation

Update:

- `crates/linkage-blaze-core/tests/both_storage_types.rs`;
- `crates/linkage-blaze-core/tests/pirouette_specialization.rs`;
- compile-pass and compile-fail fixtures;
- doctests in `crates/linkage-blaze-core/src/lib.rs`;
- utility tests that generate `.lb.rs` source;
- README or crate documentation showing linkage construction.

Low-level tests may explicitly instantiate `LinkageFixed<DOF, MARKS, N>` when the capacity boundary itself is what the test exercises. Application-style tests and documentation must use the derived API.

Do not preserve the old numeric `linkage_fixed!` form as a compatibility shim. Update all in-workspace users.

## Required Tests

Add focused tests for:

1. An imported `.lb.rs` file uses supplied `DOF` and `MARKS` and discovers exact `N` without an expected type.
2. Supplied `DOF` and `MARKS` values that are too small fail during materialization.
3. Supplied `DOF` and `MARKS` values that are too large fail the final equality assertions.
4. An invalid restore still fails during const evaluation.
5. `linkage_combine!` derives exact parameter, mark, and step counts and skips the second `Start`.
6. Combining programs that use the same mark text preserves the existing independent-slot behavior.
7. `linkage_with_joint_spheres!` derives the exact count for every move-like step variant.
8. `linkage_compact!` exactly matches the optimizer output for no-ops, adjacent merges, and sums that merge to zero.
9. Existing freeze and retain APIs preserve their validation behavior.
10. `linkage_fixed_const!` works with private and public constants plus documentation attributes.
11. An ordinary `const LINKAGE: LinkageView<DOF, MARKS> = EXPRESSION.view()` produces a `'static` promoted view with no named backing const or static.
12. A final view evaluates poses and draw items identically to the pre-migration program.
13. No allocation is required with default features.
14. The `alloc`-backed `linkage_buf!` path still accepts the same `.lb.rs` files.

Retain compile-fail coverage for semantic mistakes. Update expected diagnostics only when the new macro expansion necessarily changes their presentation.

## Documentation Requirements

Document the distinction plainly:

- `LinkageFixed` owns exact const-generic storage used during compile-time construction and transformation.
- `LinkageView` borrows the active arrays and erases step capacity for runtime use.
- measurement determines storage; the caller does not guess it;
- promotion is appropriate because the view needs static backing but not address identity.

Use `rust,no_run` for doctests. Each new public macro must have one primary compilable example or link directly to a shared example that invokes it.

Do not describe intermediate measured values as occupying firmware space. Only the retained final representation matters, subject to ordinary compiler optimization and release settings.

## Verification

During implementation:

1. Run focused `cargo test -p linkage-blaze-core`.
2. Run the compile-pass/compile-fail test harness.
3. Build every affected example feature.
4. Run `cargo fmt`.
5. Run workspace `cargo check-all` before handoff.

If the change affects generated Device Envoy examples or cross-repository interfaces, read the sibling Device Envoy `AGENTS.md`, update those users, and run its `cargo check-all` as required by the workspace instructions.

## Completion Criteria

The migration is complete when:

- production application code contains no guessed or manually copied linkage step capacities;
- capacity-changing transformations contain no manually supplied output `N`;
- `.lb.rs` imports measure themselves without stored metadata;
- final runtime programs are declared as promoted views;
- the fixed representation remains exact and allocation-free;
- all current semantic validation remains active at compile time;
- all focused tests and both required local CI suites pass where applicable.
