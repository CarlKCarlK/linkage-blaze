<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Make Linkage Composition View-In and View-Out

## Purpose

Apply the repository's `Fixed`/`View` policy to linkage composition:

- `Fixed` owns const-generic storage;
- `View` is the operational API;
- callers convert fixed storage to a view rather than using a trait that
  abstracts over both;
- public composition accepts views and returns a view;
- any exact fixed storage required by composition remains an internal
  const-evaluation detail.

The desired public use is:

```rust
const BALLET_LINKAGE: pirouette::View = linkage_combine!(
    ballet_style::view(),
    pirouette::view(),
);
```

It must not be:

```rust
const LINKAGE: LinkageView<132, 6> = linkage_view!(
    linkage_combine!(
        ballet_style::fixed(),
        pirouette::fixed(),
    )
);
```

This specification builds on `LINKAGE_FILE_MODULE_API_SPEC.md`. Assume named
linkage files expose module-relative `View` aliases and `view()` constructors.

Keep unrelated fluent construction and specialization syntax unchanged unless
a mechanical adjustment is required to make composition view-in/view-out.
Those APIs will be reviewed separately.

Do not add compatibility traits or aliases for the old composition API.

## Repository policy: `Fixed` and `View`

When a data family has `Fixed` and `View` forms, do not introduce a trait merely
so methods can accept both representations.

Use this model:

```text
Fixed --.view()--> View --operations--> View
```

`Fixed` is responsible for:

- owning arrays and other const-generic storage;
- const construction;
- exposing `.view()`;
- internal materialization when an operation must synthesize new backing data.

`View` is responsible for:

- inspection;
- evaluation and rendering;
- the public inputs to composition and other operational APIs;
- hiding storage capacity `N`.

If an operation must create new arrays, it may use or return an exact `Fixed`
inside a hidden helper. The public operation must promote or otherwise retain
that backing and return a `View`.

Do not expose an intermediate `Fixed` merely because the implementation needs
one.

## Remove the `Linkage` conversion trait

Remove the current public:

```rust
pub trait Linkage<const DOF: usize, const MARKS: usize>
```

It exists primarily to forward operations through `.view()`, which is exactly
the abstraction this repository avoids for `Fixed`/`View` pairs.

Requirements:

- keep inherent `.view()` methods on `LinkageFixed` and `LinkageBuf`;
- keep the necessary inspection and evaluation methods directly on
  `LinkageView`;
- replace generic bounds on `Linkage` with explicit `LinkageView` parameters;
- update callers to pass `.view()` explicitly when they hold storage;
- do not replace `Linkage` with another trait having the same purpose;
- do not retain a deprecated compatibility trait or re-export.

If a method currently exists only as a default method on `Linkage`, move or
retain the useful behavior on `LinkageView` before deleting the trait.

Do not remove unrelated domain traits that serve a different abstraction.

## `linkage_combine!` public contract

`linkage_combine!` must accept two or more const-evaluable linkage views:

```rust
linkage_combine!(
    first::view(),
    second::view(),
)
```

and:

```rust
linkage_combine!(
    first::view(),
    second::view(),
    third::view(),
)
```

The result must be a promoted/static `LinkageView` whose const generics are:

- `DOF`: the sum of input degrees of freedom;
- `MARKS`: the sum of input mark-slot counts.

Its active steps must be the same as the existing left-associative combination:

- retain the first input's implicit `Start`;
- skip each later input's implicit `Start`;
- preserve operation order;
- offset later parameter indexes correctly;
- offset later mark indexes correctly;
- preserve the final pose and pen state across input boundaries.

The result must not expose step capacity `N`.

The two-input and variadic forms must have the same public result category:
both return a view.

## Internal materialization

A flat combined linkage cannot generally borrow the input slices without either
introducing a segmented/composite evaluator or creating new backing storage.
Do not introduce a segmented view or audio-sequence-like evaluator in this
change.

Use the simpler existing semantics:

1. Read both input views.
2. Derive the exact output step count during const evaluation.
3. Copy parameters, marks, and steps into one exact internal `LinkageFixed`.
4. Apply the existing parameter and mark index offsets.
5. Promote or otherwise retain that exact backing.
6. Return its `LinkageView`.

An internal helper may return the exact `LinkageFixed` because a Rust method
cannot return a view borrowing a newly created local array. Keep that helper
private or macro-internal and give it an implementation-detail name such as
`__combine_fixed`.

The public `linkage_combine!` macro must hide both that helper and the final
promotion. Callers must not wrap its result in `linkage_view!`.

Do not allocate at runtime. Keep the core crate `no_std` and no-allocation.

## `LinkageView` composition implementation

Put the input-side combination logic on `LinkageView`, consistent with the
repository policy. Conceptually:

```rust
impl<const DOF: usize, const MARKS: usize>
    LinkageView<'_, DOF, MARKS>
{
    const fn __combine_fixed</* derived output dimensions */>(
        self,
        other: LinkageView<'_, DOF2, MARKS2>,
    ) -> LinkageFixed<DOF_OUT, MARKS_OUT, N_OUT> {
        /* copy and offset */
    }
}
```

The exact generic spelling may differ. It must remain usable from stable Rust
and from public macro expansions in downstream crates. Follow the repository's
`#[doc(hidden)]` macro-helper rules if a helper must technically be public for
macro hygiene.

Do not put the public combination operation on both `LinkageFixed` and
`LinkageView`. There should be one canonical operational path through
`LinkageView`.

## Type spelling

For linkage-file modules, prefer the generated view alias:

```rust
const BALLET_LINKAGE: pirouette::View = /* ... */;
```

Do not repeat discovered values:

```rust
LinkageView<132, 6>
```

and do not replace them with a more verbose repetition:

```rust
LinkageView<{ pirouette::DOF }, { pirouette::MARKS }>
```

The module-relative `View` alias exists to hide those details while retaining
them in the real Rust type.

When composition changes `DOF` or `MARKS`, use the existing named-program
facility or another already-approved module-relative alias. Do not redesign
`linkage_program!` in this task.

## Production call-site migration

Migrate every `linkage_combine!` call to pass views and consume a view.

Named file inputs must use:

```rust
camera_control::view()
grid9x9::view()
pirouette::view()
```

not:

```rust
camera_control::fixed()
grid9x9::fixed()
pirouette::fixed()
```

Remove outer wrappers such as:

```rust
linkage_view!(linkage_combine!(...))
```

because `linkage_combine!` itself now returns the final view.

For an input produced by an unrelated transformation that still returns fixed
storage, convert it to a view before composition. Keep that conversion as local
and unobtrusive as stable const evaluation permits. Do not redesign the
unrelated transformation in this change.

Preserve exact operation ordering in Armatron:

1. camera;
2. grid;
3. displayed arm with joint spheres;
4. restore the scene origin;
5. append the ghost arm;
6. apply ghost-arm styling.

Do not repeat the earlier ordering regression while changing storage forms.

## Ballet target example

The final Ballet composition should communicate:

```rust
const BALLET_LINKAGE: pirouette::View = linkage_combine!(
    /* style view */,
    pirouette::view(),
);
```

This task does not need to settle the final inline-style construction syntax.
However:

- `pirouette::fixed()` must disappear from the combination;
- the result type must be `pirouette::View`;
- the result must not call `.view()` or be wrapped in `linkage_view!`;
- do not introduce new manual capacity arithmetic to construct the style.

If the existing style expression prevents a clean final production spelling,
use the least invasive existing view-producing mechanism and leave a normal
`TODO` describing only the inline-style syntax question. Do not compromise the
view-in/view-out composition contract.

## Documentation

Update `linkage_combine!` documentation so its primary example uses two views
and returns a view:

```rust
const COMBINED: combined_program::View = linkage_combine!(
    first_program::view(),
    second_program::view(),
);
```

The documentation must explain:

- inputs are views;
- the result is a view;
- exact fixed backing is generated and promoted internally during const
  evaluation;
- later inputs continue from the preceding final pose;
- later parameter and mark indexes are offset;
- no runtime allocation is performed.

Do not teach `LinkageFixed` or capacity `N` in the primary composition example.

Update `LinkageView` documentation to identify it as the canonical operational
API. Remove documentation that recommends using the deleted `Linkage` trait.

## Tests

Add or adapt focused tests for:

1. Two views combine into a const `LinkageView`.
2. Variadic view composition is left-associative.
3. The public result needs no outer `.view()` or `linkage_view!`.
4. Input step capacity does not appear in the result type.
5. Parameter arrays concatenate in the expected order.
6. Parameter indexes in later steps are offset correctly.
7. Mark names and mark indexes in later inputs are offset correctly.
8. Later programs continue from the preceding final pose.
9. The second and later implicit `Start` steps are skipped.
10. A zero-DOF/zero-mark style view combined with `pirouette::View` produces
    the exact `pirouette::View` type.
11. The no-allocation core configuration supports const composition.
12. `LinkageFixed::view()` and `LinkageBuf::view()` remain available without a
    conversion trait.
13. No public `Linkage` trait remains.
14. Clock, Ballet, Skeleton Clock, and Armatron rendered behavior remains
    unchanged.

Include compile-time type assertions in focused tests, not as redundant
production constants.

## Scope exclusions

Do not use this task to redesign:

- inline linkage-style construction;
- `linkage_program!`;
- parameter freezing or retention syntax;
- `linkage_extend!`;
- `linkage_with_joint_spheres!`, beyond any mechanical view conversion needed
  at a composition boundary;
- file metadata derivation;
- static versus const identity;
- a segmented or dynamically dispatched linkage representation.

Record clear follow-up observations without implementing them.

## Required verification

Run formatting and focused core tests while iterating. Before considering the
implementation complete, run from the Linkage Blaze repository root:

```text
cargo check-all
```

It must exit successfully. Do not report it as passing while it is still
running.

## Acceptance criteria

- `LinkageView` is the canonical operational linkage API.
- The `Linkage` conversion trait is removed rather than replaced.
- `linkage_combine!` accepts two or more views.
- `linkage_combine!` returns a promoted/static view.
- Exact fixed backing and capacity calculations are hidden internally.
- Ordinary composition contains neither `.fixed()` inputs nor an outer
  `linkage_view!`.
- Named file result types use module-relative `View` aliases.
- Combination behavior, parameter offsets, mark offsets, and operation order
  are unchanged.
- The core remains stable-Rust, `no_std`, and no-allocation.
- `cargo check-all` exits successfully.
