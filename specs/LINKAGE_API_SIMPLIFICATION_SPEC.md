<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Linkage API Simplification and Repository-Wide Review

## Decision

Replace the current construction and composition API everywhere with one
ownership-led model:

- `LinkageFixed<DOF, MARKS, N>` owns fixed-capacity storage and supports const
  construction.
- `LinkageBuf<DOF, MARKS>` owns growable runtime storage.
- `LinkageView<DOF, MARKS>` is a read-only borrowing boundary for evaluators,
  renderers, serializers, and other consumers. It erases fixed step capacity
  `N`; it is not an owner, builder, or general composition layer.
- An owned linkage combines a borrowed right input with `.combine(view)` and
  returns the same ownership family as its receiver.

The short explanation of the API must remain true:

> `LinkageFixed` owns const-friendly fixed storage, while `LinkageBuf` owns
> growable runtime storage. Both expose `LinkageView` for read-only operations
> that do not care about backing capacity. Combining preserves the receiver's
> ownership kind and copies a borrowed right input.

If an implementation choice cannot be explained without qualifying that
paragraph, treat it as evidence that the choice does not belong in the basic
API.

This specification supersedes the construction, derived-size, view-first,
and composition directions in these older specs:

- `LINKAGE_API_ERGONOMICS_FOLLOWUP_SPEC.md`;
- `LINKAGE_API_FINAL_POLISH_SPEC.md`;
- `LINKAGE_DERIVED_SIZES_SPEC.md`;
- `LINKAGE_VIEW_FIRST_COMPOSITION_SPEC.md`;
- the composition portions of `LINKAGE_FILE_MACRO_SPEC.md` and
  `LINKAGE_FILE_MODULE_API_SPEC.md`.

Preserve the established file-reading API unless its implementation depends
on a deleted composition abstraction. During implementation, delete obsolete
specs or edit their still-relevant file-API requirements into agreement with
this specification. Do not leave contradictory API plans in `specs/`.

## Canonical call sites

Ordinary const construction produces owned fixed storage and does not end in
`.view()`:

```rust
// TODO000API Review whether this ordinary fixed construction is self-explanatory.
const LEFT: LinkageFixed<1, 0, 3> =
    LinkageFixed::start()
        .forward(10.0)
        .yaw(30.0);
```

Fixed combination consumes the fixed receiver, copies a borrowed right input,
and returns `LinkageFixed` with dimensions and capacity supplied by the result
annotation:

```rust
// TODO000API Review the canonical two-input fixed combination.
const LEFT_AND_RIGHT: LinkageFixed<3, 1, 4> =
    LEFT.combine(RIGHT.view());
```

Runtime buffer combination follows the same rule and returns a buffer:

```rust
// TODO000API Review whether receiver-owned buffer combination reads as append-and-copy.
let combined: LinkageBuf<3, 1> =
    left_buf.combine(RIGHT.view());
```

Consumers borrow either owner through the same concrete view:

```rust
// TODO000API This is a real read-only boundary; confirm that `.view()` helps here.
render(combined.view());
```

Do not introduce a trait to hide `.view()`. The view is useful because it is
the actual common borrowed representation. A trait would add generic bounds
or trait-object dispatch without representing different linkage behavior, and
would complicate const use.

## Multiple inputs

Use named binary intermediates first:

```rust
// TODO000API Three ordered inputs require an intermediate; assess whether the name improves clarity.
const LEFT_AND_RIGHT: LinkageFixed<3, 1, 4> =
    LEFT.combine(RIGHT.view());

// TODO000API If many real sites look worse than this, reconsider a variadic combination macro.
const COMPLETE: LinkageFixed<3, 2, 5> =
    LEFT_AND_RIGHT.combine(TAIL.view());
```

Do not initially provide a variadic combination macro. A macro is justified
only if the marked production call sites show that named binary intermediates
are materially harder to understand. If reconsidered, evaluate it against the
real Armatron migration rather than a toy example. It must return owned
`LinkageFixed`, not promoted `LinkageView`, so that construction never hides
ownership behind a borrow.

## Explicit conversion from an arbitrary view

The receiver-owned rule intentionally makes ownership asymmetric. When the
left source exists only as a view but fixed output is actually required, make
the ownership conversion visible:

```rust
// TODO000API Confirm that this uncommon view-to-fixed conversion is genuinely needed.
let left_fixed: LinkageFixed<1, 0, 3> =
    left_buf.view().to_fixed();

// TODO000API Review the combination after the explicit ownership conversion.
let combined: LinkageFixed<3, 1, 4> =
    left_fixed.combine(RIGHT.view());
```

`LinkageView::to_fixed` is the only general view-to-owner conversion in the
initial API. It copies the active data, verifies matching `DOF` and `MARKS`,
and verifies that the annotated `N` is large enough. Never convert an existing
fixed owner to a view and back merely to gain capacity; reserve suffix capacity
when constructing the fixed owner instead. Do not add
`from_view_pair`, `from_views`, a free `combine_views`, or a parallel trait.

## Step-capacity erasure

Erase `N` only when borrowing for a read-only operation. Consumers generally
need active parameters, marks, and steps, not the capacity of the fixed arrays
that hold them. This is the same reason an array is commonly borrowed as a
slice.

Do not erase `N` merely to make construction appear uniform. Producing another
fixed owner needs a capacity, so early erasure forces the API to reconstruct
that information using annotations, assertions, promotions, or macros. The
current experimental promoted-view combination is an example of complexity
created by erasing capacity too early.

Keep `DOF` and `MARKS` in `LinkageView`'s type. They are semantic linkage
dimensions rather than storage capacity.

## APIs to remove

Delete these public paths and migrate every use without compatibility aliases,
deprecated forwarding paths, or hidden replacements:

- the free `combine_views` experiment;
- `LinkageBuf::combine_views`;
- `LinkageBuf::combine_ref`;
- `linkage_program!`;
- `linkage_combine!`;
- `linkage_extend!`;
- `linkage_view!`;
- public helpers used only by those macros, including promoted-backing and
  step-count replay machinery that becomes unused.

Replace `LinkageFixed::combine(fixed)` with
`LinkageFixed::combine(view)`. Replace `LinkageBuf::combine(buf)` and
`combine_ref(view)` with the single canonical `LinkageBuf::combine(view)`.
Both operations consume the receiver, copy the right view, skip its implicit
`Start`, offset its parameter and mark indexes, and return their receiver's
ownership family.

Prefer ordinary typed fixed methods over structural capacity-derivation macros.
Move the application-specific `linkage_with_joint_spheres!` behavior to an
Armatron-local const function rather than preserving it in the public API. Mark
every such site as described below; repeated unreadable capacities may motivate
a narrowly scoped future facility, but do not preserve a general macro system
preemptively.

Keep `linkage_file!` and the useful fixed/buffer/view access supplied by file
modules. File parsing and inclusion are not the design problem addressed here.

## `TODO000API` review protocol

Before migrating behavior, mark every logical public Linkage API use with an
adjacent comment containing the exact token `TODO000API`. Preserve or move the
marker with the call site during migration. The human will review every marker
after the repository compiles.

A logical use includes:

- fixed or buffered construction expressions;
- file linkage declarations and calls returning their fixed, buffered, or view
  forms;
- every combination;
- every fixed/buffer-to-view conversion;
- capacity-changing transformations and parameter specialization;
- every evaluator, renderer, serializer, or other consumer boundary that
  accepts a view;
- API examples and doctests;
- behavioral, UI-pass, and UI-fail tests.

One marker may cover one contiguous fluent expression; do not annotate every
method in the same chain. Imports and type-only mentions do not need markers.
Generated `.lb.rs` data does not need hand-edited markers; mark the generator,
template, parser test, or `linkage_file!` use through which it enters the API.

Every marker must say what the reviewer should consider. Do not add a bare,
identical marker everywhere. Examples:

```rust
// TODO000API Ordinary fixed declaration; confirm the capacity annotation is understandable.
const LINKAGE: LinkageFixed<1, 0, 3> = LinkageFixed::start().forward(1.0);

// TODO000API Four ordered inputs make named intermediates lengthy; possible variadic-macro evidence.
const SCENE: LinkageFixed<15, 4, 159> = SCENE_WITH_ARM.combine(GHOST_ARM.view());

// TODO000API View is used only for rendering here; confirm this is a useful erasure boundary.
renderer.draw(LINKAGE.view());
```

For Markdown prose, place an HTML comment containing `TODO000API` immediately
before the relevant code fence. Keep all markers until the human explicitly
accepts or changes each call site. The implementation is not complete merely
because tests pass while review markers remain unexamined.

Use repository-wide searches before and after migration. At minimum, audit:

```text
rg -n "Linkage(Fixed|View|Buf)|linkage_.*!|\\.view\\(\\)|\\.combine(_ref)?\\(" crates --glob '*.rs'
rg -n "TODO000API" crates specs --glob '*.rs' --glob '*.md'
```

## Known pressure points to mark specifically

The initial inventory found these places likely to motivate more complex API.
They must receive specific `TODO000API` explanations during migration:

- `crates/linkage-blaze-core/src/examples/armatron/main.rs`: three-input scene
  composition, joint-sphere capacity growth, restoration and fluent suffixes,
  a repeated arm, and a separate arm-tip linkage. This is the primary evidence
  for or against a variadic macro.
- `crates/linkage-blaze-core/src/examples/skeleton_clock.rs`: prepending drawing
  style to file data and then specializing parameters while changing dimensions.
- `crates/linkage-blaze-core/src/examples/ballet.rs`: prepending drawing style
  to a large file linkage. This tests whether one simple binary combine remains
  readable.
- `crates/linkage-blaze-core/tests/both_storage_types.rs`: mixed fixed, buffer,
  and view operations. This is the primary evidence for whether the asymmetric
  ownership rule remains functional.
- `crates/linkage-blaze-core/tests/pirouette_specialization.rs`: fixed
  specialization and capacity changes.
- `crates/linkage-blaze-utils/src/lib.rs` and `src/bvh.rs`: runtime file parsing,
  buffer ownership, and read-only consumption.
- core documentation and doctests in `crates/linkage-blaze-core/src/lib.rs`:
  these are the explainability test and must teach only the canonical paths.

Do not treat this list as exhaustive. The pre-migration repository-wide search
defines the complete review set.

## Experimental project cleanup

Use `experiments/linkage-api` only long enough to prove the receiver-owned
signatures on Rust stable. Do not preserve its current `tests/api.rs` as a
second API or port its view-to-view combination design into production.

After the production migration and focused tests pass:

1. Delete `experiments/linkage-api/tests/api.rs`.
2. Delete the rest of `experiments/linkage-api`, including the literal-slice
   alternative, README, manifest, and local lockfile.
3. Delete the root `just check-linkage-api` recipe and its `TODO00` comment.
4. Verify no documentation links or scripts refer to the deleted experiment.

## Implementation order

1. Inventory and annotate every logical API use with a specific `TODO000API`
   comment before changing behavior.
2. Add focused compile tests for `LinkageFixed::combine(view)`,
   `LinkageBuf::combine(view)`, `LinkageView::to_fixed()`, and `.view()`
   at read-only consumer boundaries.
3. Implement the three owner operations without macros or traits.
4. Migrate ordinary two-input fixed and buffer call sites.
5. Migrate complex examples using named, typed fixed intermediates. Preserve
   their `TODO000API` comments explaining any resulting friction.
6. Migrate docs, doctests, UI tests, utilities, and remaining workspace crates.
7. Delete obsolete macros, helpers, tests, and conflicting spec requirements.
8. Delete the experimental project and its `just` recipe.
9. Run formatting, focused tests, `cargo check-all`, and `git diff --check`.
10. Hand the complete `TODO000API` inventory to the human for call-site review.

## Acceptance criteria

- The basic API is explainable by the short paragraph at the start of this
  specification.
- Fixed construction and combination return owned `LinkageFixed`; buffer
  combination returns owned `LinkageBuf`; neither returns a promoted view.
- `.view()` appears only at a borrowed input or read-only consumer boundary.
- Binary combination consumes an owner and accepts one right-hand
  `LinkageView`.
- Multiple inputs use named fixed intermediates unless reviewed production
  evidence later authorizes a macro.
- No deleted macro, free combination function, duplicate buffer combination,
  compatibility shim, or macro-only helper remains.
- Every logical API use carries a specific `TODO000API` review comment.
- Armatron, skeleton clock, ballet, fixed/buffer parity tests, file parsing,
  embedded builds, WASM builds, doctests, and UI tests continue to work.
- `cargo check-all` passes.
- `experiments/linkage-api` and `just check-linkage-api` are gone.
