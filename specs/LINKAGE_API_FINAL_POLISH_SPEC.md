<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Linkage API Final Polish

## Purpose

Polish the newly implemented linkage API until its production examples are concise, correct, and suitable for the “Nine Rules for Rust `const fn`” article.

The underlying design is now sound:

- `linkage_program!` resembles Device Envoy’s audio declaration macros;
- `DOF` and `MARKS` remain meaningful public constants;
- step capacity `N` is measured and hidden;
- `linkage_combine!` and `linkage_with_joint_spheres!` represent structural transformations;
- `linkage_extend!` preserves ordinary fluent DSL instructions;
- `linkage_view!` produces exact promoted backing without a public compaction step.

This specification addresses the remaining correctness, readability, visibility, documentation, and API-completeness issues. Do not add compatibility shims for the current intermediate API.

## Correct Armatron Before Polishing Syntax

The current Armatron migration changes operation order.

Previously:

```rust
SCENE_WITH_ARM
    .restore("scene origin")
    .combine(ARMATRON1)
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("close hand", 0.5, 0.0)
```

The current expression combines the second arm before restoring the scene origin. This can change the ghost arm’s starting pose and is not covered by the existing unit tests.

Restore the original semantic order:

1. Build the camera, grid, and displayed arm.
2. Restore `"scene origin"`.
3. Combine the second `Armatron1` program.
4. Append the red pen color and hand sphere.
5. Materialize the exact final view.

Use a meaningfully named derived program for the reusable scene:

```rust
linkage_program! {
    SceneWithArm {
        program: linkage_combine!(
            CameraControl::fixed(),
            Grid9x9::fixed(),
            linkage_with_joint_spheres!(Armatron1::fixed(), 0.15),
        ),
        dof: 9,
        marks: 3,
    }
}
```

Then construct the final program conceptually as:

```rust
const LINKAGE: LinkageView<15, 4> = linkage_view!(
    linkage_extend!(
        linkage_combine!(
            linkage_extend!(
                SceneWithArm::fixed();
                .restore("scene origin")
            ),
            Armatron1::fixed(),
        );
        .pen_color(Rgb888::CSS_RED)
        .sphere_param("close hand", 0.5, 0.0)
    )
);
```

Adjust syntax to the final variadic `linkage_combine!` implementation, but preserve this ordering exactly.

Add a focused test that would fail if the second arm were evaluated before the restore. Prefer comparing relevant poses or mark-relative geometry over testing only counts.

Replace:

```rust
const TARGET_PARAM_START: usize = 9;
```

with:

```rust
const TARGET_PARAM_START: usize = SceneWithArm::DOF;
```

Do not replace derived structural facts with magic numbers merely to simplify macro migration.

## Remove Redundant Type-Check Constants

Delete production lines such as:

```rust
const _: LinkageView<3, 1> = CameraControl::VIEW;
const _: LinkageView<0, 1> = Grid9x9::VIEW;
const _: LinkageView<6, 1> = Armatron1::VIEW;
```

Also remove the analogous lines from Ballet and Skeleton Clock.

They provide no application behavior and duplicate dimensions already supplied to and validated by `linkage_program!`.

If compile-time verification of generated view types is valuable, place it in focused macro tests rather than every application. One test should demonstrate that:

```rust
const _: LinkageView<EXPECTED_DOF, EXPECTED_MARKS> = Program::VIEW;
```

compiles for a representative generated program.

## Honor Visibility in `linkage_program!`

The macro currently captures:

```rust
$visibility:vis
```

but always emits:

```rust
pub struct $name;
```

Change both the `file:` and `program:` macro forms to emit:

```rust
$visibility struct $name;
```

Required behavior:

- no visibility token produces a private marker type;
- `pub(crate)` produces a crate-visible marker type;
- `pub(super)` and other restricted visibility forms work;
- `pub` produces a public marker type;
- attributes and documentation remain attached to the generated marker.

Associated items may remain `pub`; their effective visibility is bounded by the marker type’s visibility. Do not create separate private/public macro implementations.

Add compile tests for private, restricted, and public declarations. Include at least one downstream-style test proving that a private program cannot be accessed outside its module.

## Make `linkage_combine!` Variadic

Extend:

```rust
linkage_combine!(first, second)
```

to accept two or more programs:

```rust
linkage_combine!(first, second, third)
```

and:

```rust
linkage_combine!(
    CameraControl::fixed(),
    Grid9x9::fixed(),
    linkage_with_joint_spheres!(Armatron1::fixed(), 0.15),
)
```

Semantics must be left-associative:

```rust
linkage_combine!(a, b, c)
```

is equivalent to:

```rust
linkage_combine!(linkage_combine!(a, b), c)
```

This preserves parameter order, mark offsets, step order, and the rule that each appended program’s implicit `Start` is skipped.

Implementation guidance:

- retain the two-expression arm as the primitive;
- implement the variadic arm recursively;
- accept a trailing comma;
- require at least two operands;
- provide a clear compile error for zero or one operand if practical;
- do not introduce a runtime collection or trait-object sequence.

Add equivalence tests comparing two-operand nesting with the variadic form, including programs with parameters and marks.

Use the variadic form in Armatron where it materially reduces nesting. Do not rewrite readable two-part combinations merely to demonstrate the feature.

## Finish Removing “Compaction” Terminology

Public compaction has been removed. Complete the conceptual cleanup.

Rename:

```rust
PIROUETTE_BODY_OPT
```

to a name that describes what it now is, such as:

```rust
PIROUETTE_BODY_VIEW
```

Rename tests such as:

```rust
pirouette_body_optimized_matches_original
pirouette_body_const_opt_matches_buf_opt
```

to describe fixed/view or fixed/buffer equivalence:

```rust
pirouette_body_exact_view_matches_fixed
pirouette_body_view_matches_buf
```

Update comments that incorrectly claim `linkage_view!` performs an optimization pass or produces fewer active steps. It shrinks backing storage to the already active step count and erases `N`; it does not further optimize the program.

Remove stale documentation such as:

```rust
Combined with no-op stripping (see [`compact`](Self::compact)) afterward
```

Rewrite it to describe the private specialization cleanup pipeline directly.

Search the entire workspace for:

```text
compact
compaction
optimized
_OPT
const_opt
buf_opt
```

Remove or revise references that describe the deleted public concept. Preserve “optimize” only where code genuinely performs specialization cleanup or another real optimization.

## Add Primary Documentation Examples

The new public macros currently have minimal descriptions and no strong primary examples.

Add `rust,no_run` examples for:

### `linkage_program!`

Show an audio-style file declaration:

```rust
linkage_program! {
    ClockLinkage {
        file: "clock.lb.rs",
        dof: 2,
        marks: 2,
    }
}

const CLOCK: LinkageView<2, 2> = ClockLinkage::VIEW;
```

Explain:

- `DOF` and `MARKS` are supplied and validated;
- `STEP_COUNT` is measured;
- `fixed()` exposes exact const storage;
- `VIEW` erases the measured `N`.

Also document the `program:` form or link directly to a shared example covering it.

### `linkage_combine!`

Show a variadic combination and state that programs continue from the preceding final pose while later implicit `Start` steps are skipped.

### `linkage_with_joint_spheres!`

Show the structural transform and explain that output capacity is derived from the program contents.

### `linkage_extend!`

Show an ordinary fluent extension:

```rust
linkage_extend!(program;
    .restore("origin")
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("radius", 0.0, 1.0)
)
```

State clearly that the first implementation does not permit `define_param` or creation of new marks.

### `linkage_view!`

Show specialization followed by exact view materialization. Explain that it:

- shrinks backing to active steps;
- promotes the final data;
- erases `N`;
- does not perform another optimization pass.

### `linkage_fixed!`

Decide whether it remains a primary public entry point or a lower-level implementation-oriented API.

If `linkage_program!` is canonical for named files, update `linkage_fixed!` documentation to direct most readers there. Avoid presenting two equally preferred declaration styles.

Every public macro must either contain its own compilable example or link directly to one primary example that invokes it.

## Decide Whether to Generate `Program::buf()`

The current implementation preserves growable loading through:

```rust
linkage_buf!("same-file.lb.rs", DOF, MARKS)
```

This satisfies the core requirement that one `.lb.rs` file works with fixed and growable storage, but the named program namespace exposes only `fixed()` and `VIEW`.

Perform a small implementation spike for:

```rust
let linkage = Pirouette::buf();
```

Requirements:

- the method exists only when `linkage-blaze-core` is built with `alloc`;
- feature selection must be based on the dependency crate, not an accidental downstream `#[cfg(feature = "alloc")]`;
- it returns `LinkageBuf<DOF, MARKS>`;
- it interprets the same file body;
- it produces behavior equivalent to `fixed()`;
- it does not require allocation in the default fixed-only build.

A suitable approach may use a hidden exported helper macro whose definition is selected by `linkage-blaze-core`’s own `alloc` configuration.

Adopt `Program::buf()` only if the implementation remains small, hygienic, and well documented. Otherwise:

- keep `linkage_buf!`;
- document it explicitly in the `linkage_program!` example;
- record that generated `buf()` was deferred;
- do not add a brittle feature workaround.

This item is optional for release; the decision and rationale are required.

## Record the `?Sized` Decision

Do not introduce a `?Sized` linkage representation in this polish change.

Record the decision:

- `LinkageFixed<DOF, MARKS, N>` remains the owned const-generic representation;
- `LinkageView<'a, DOF, MARKS>` remains the existing explicit borrowed wrapper;
- `.view()` remains the conversion;
- `linkage_view!` performs exact backing materialization and promotion before producing that wrapper;
- `N` is already erased from runtime callers without a custom dynamically sized type.

Rationale:

- the surface API now achieves the desired type erasure;
- a custom DST would be a substantial internal representation rewrite;
- no demonstrated usability or firmware-size benefit currently justifies that rewrite;
- it can be reconsidered independently if future measurements show a concrete benefit.

Put this decision in implementation notes or durable design documentation. Public API documentation need only explain the chosen `Fixed`/`View` model; it does not need to teach rejected custom-DST machinery.

## Production Example Quality

After migration, the four primary examples should read as follows conceptually:

- Clock: one `linkage_program!` declaration and one `Program::VIEW` use.
- Ballet: one named source program plus a short structural style combination.
- Skeleton Clock: named source/derived programs, fluent specialization, and `linkage_view!`.
- Armatron: three named source programs, one named scene composition, correct restore ordering, and no magic capacity or parameter-boundary numbers.

Remove imports made obsolete by deleting redundant constants or changing macro usage.

Do not retain demonstration-only declarations in production examples. Macro type checks belong in tests.

## Required Tests

Add or update tests covering:

1. Armatron’s restore occurs before the second arm program.
2. `TARGET_PARAM_START` derives from `SceneWithArm::DOF`.
3. No redundant anonymous type-check constants remain in production examples.
4. Private `linkage_program!` declarations remain private.
5. Restricted and public visibility are honored.
6. Attributes and documentation survive macro expansion.
7. Two-part `linkage_combine!` remains unchanged.
8. Three-or-more-part combination is left-associative and equivalent to nesting.
9. Variadic combination preserves parameter order, marks, and steps.
10. `linkage_view!` produces exact backing without changing active behavior.
11. Fixed/view/buffer pirouette equivalence tests use accurate names and comments.
12. Documentation examples compile.
13. Fixed-only builds do not acquire allocation.
14. If implemented, `Program::buf()` matches `Program::fixed()`.

## Verification

Run focused tests while implementing:

```text
cargo test -p linkage-blaze-core --features alloc
```

Build all affected example feature combinations, including Armatron, Ballet, Clock, and Skeleton Clock.

Then run the repository’s complete local CI:

```text
cargo check-all
```

`cargo check-all` must pass. This is a completion requirement, not an optional recommendation.

Do not report the work complete if `cargo check-all` fails, is interrupted, or is not run. Report the exact blocker instead.

## Completion Criteria

The polish is complete only when:

- Armatron’s original operation order and behavior are restored;
- `TARGET_PARAM_START` is derived;
- redundant `const _: LinkageView` lines are gone from production examples;
- `linkage_program!` honors visibility;
- variadic combination reduces meaningful nesting;
- all obsolete compaction names, comments, and links are removed;
- every new public macro has a primary compilable documentation path;
- the `Program::buf()` decision is implemented or explicitly deferred with rationale;
- the decision to retain the explicit `LinkageView` wrapper is recorded;
- focused tests pass;
- every affected example builds;
- `cargo check-all` passes.
