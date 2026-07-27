<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Linkage API Ergonomics Follow-Up

## Purpose

Revise the first implementation of `LINKAGE_DERIVED_SIZES_SPEC.md` before treating its public API as final.

The first implementation successfully measures step count `N`, derives exact combination sizes, and passes the core test suite. Its public surface is nevertheless too noisy:

```rust
linkage_fixed_const! {
    const LINKAGE0 =
        linkage_fixed!("../assets/mocap/pirouette.lb.rs", { MOTION.dof() }, 6);
}
```

It also replaces ordinary fluent linkage instructions with operation-specific macros:

```rust
linkage_restore!(...)
linkage_display_style!(...)
```

That direction obscures the linkage DSL we want users to see. Replace it with an audio-style named-program macro, retain only macros for genuinely structural operations, and make exact final storage part of view materialization rather than a separate “compaction” concept.

This is a planning specification. Do not preserve the current API merely for compatibility; migrate all in-workspace callers to the final design.

## Design Principles

1. `DOF` and `MARKS` are meaningful program properties and may remain user-supplied.
2. `N` is a backing-storage detail. Derive it and hide it from runtime callers.
3. Preserve the fluent DSL for ordinary linkage instructions.
4. Use simple const arithmetic when an output size follows directly from its parts.
5. Use two-pass const evaluation when a result must be discovered by executing a transformation.
6. A final view should receive exact promoted backing automatically.
7. One `.lb.rs` source must continue to support fixed const construction and growable runtime construction.
8. Do not add build scripts, procedural macros, allocation to the core fixed path, nightly features, or `unsafe`.

## Keep the Successful Internal Work

Retain the useful concepts from the first implementation:

- a const-evaluable pass that counts steps in an `.lb.rs` program;
- exact `LinkageFixed<DOF, MARKS, N>` materialization;
- compile-time equality checks for supplied `DOF` and `MARKS`;
- derived `combine` output sizes;
- derived output size for `with_joint_spheres`;
- the existing compile-time validation of parameters, marks, ranges, and specialization;
- reuse of the same `linkage![...]` body by fixed and growable builders.

The current manually duplicated `LinkageStepCount` method list does not satisfy the earlier specification. Replace it with a shared DSL operation inventory used to generate the corresponding `LinkageFixed`, `LinkageBuf`, and counter methods. Adding a DSL instruction must not require remembering an unrelated second list.

## Replace Nested Declarations with an Audio-Style Macro

### Desired syntax

Provide a named declaration macro analogous to Device Envoy’s `pcm_clip!`:

```rust
linkage_program! {
    Pirouette {
        file: "../assets/mocap/pirouette.lb.rs",
        dof: { MOTION.dof() },
        marks: 6,
    }
}
```

Allow multiple declarations in one invocation:

```rust
linkage_program! {
    CameraControl {
        file: "../../assets/examples/armatron/camera_control.lb.rs",
        dof: 3,
        marks: 1,
    }

    Grid9x9 {
        file: "../../assets/examples/armatron/grid_9x9.lb.rs",
        dof: 0,
        marks: 1,
    }

    Armatron1 {
        file: "../../assets/examples/armatron/armatron1.lb.rs",
        dof: 6,
        marks: 1,
    }
}
```

The generated namespace should preferably be a zero-sized marker type with associated constants and functions, avoiding non-snake-case modules:

```rust
Pirouette::DOF
Pirouette::MARKS
Pirouette::STEP_COUNT
Pirouette::fixed()
Pirouette::view()
```

The exact internal backing type remains:

```rust
LinkageFixed<
    { Pirouette::DOF },
    { Pirouette::MARKS },
    { Pirouette::STEP_COUNT },
>
```

Application code should almost never need to write that type.

### Fixed construction

`Pirouette::fixed()` returns the exactly sized fixed program. It must be const-evaluable and suitable as input to structural macros:

```rust
linkage_combine!(STYLE, Pirouette::fixed())
```

The generated implementation measures the included `.lb.rs` chain, constructs exact storage, and asserts:

```rust
fixed.param_count() == Pirouette::DOF
fixed.mark_count() == Pirouette::MARKS
```

### View access

`Pirouette::view()` returns a promoted `'static` `LinkageView<DOF, MARKS>` with `N` erased:

```rust
const LINKAGE: LinkageView<{ Pirouette::DOF }, { Pirouette::MARKS }> =
    Pirouette::view();
```

If associated const syntax is cleaner and compiles reliably, `Pirouette::VIEW` is acceptable. Choose one canonical public form rather than exposing equivalent methods and constants.

### Growable construction

The same `.lb.rs` file must remain usable for growable runtime construction.

Prefer:

```rust
let linkage = Pirouette::buf();
```

when the dependency was built with `alloc`. Implement feature-sensitive expansion through a helper macro whose definition is selected inside `linkage-blaze-core`; do not use a downstream `#[cfg(feature = "alloc")]` that accidentally tests the application crate’s features.

If a clean generated `buf()` cannot be provided without feature-resolution problems, keep the existing:

```rust
linkage_buf!("pirouette.lb.rs", 132, 6)
```

and document it beside `Pirouette::fixed()`. The essential requirement is that the `.lb.rs` source remains representation-independent.

### Named derived programs

Support an expression form for a derived program that deserves a reusable name:

```rust
linkage_program! {
    CameraAndGrid {
        program: linkage_combine!(CameraControl::fixed(), Grid9x9::fixed()),
        dof: 3,
        marks: 2,
    }
}
```

The macro validates the supplied `DOF` and `MARKS`, discovers the expression’s active step count, resizes it to exact backing, and exposes the same `fixed()` and `view()` interface.

This replaces repeated anonymous wrappers such as:

```rust
linkage_fixed_const! {
    const CAMERA_AND_GRID = linkage_combine!(CAMERA_CONTROL, GRID_9X9);
}
```

Use named derived programs only when the result is reused or its name materially clarifies the application. Inline one-use structural expressions.

## Remove `linkage_fixed_const!`

Remove the first implementation’s public `linkage_fixed_const!` macro after migrating callers.

Rust does require a concrete type on a named const, so simply replacing it with inferred `let` bindings is not sufficient:

- a local fixed value cannot be borrowed into a `'static` view;
- a local value cannot be referenced inside const-generic output arguments.

The named `linkage_program!` namespace solves those constraints while giving the declaration useful structure and an audio-like API.

Do not retain both declaration systems.

## Keep `linkage_combine!`, but Keep It Structural

Combination is a genuine structural transformation, unlike `.restore()` or `.pen_color()`.

Keep one generic:

```rust
linkage_combine!(first, second)
```

It derives:

```rust
DOF_OUT = first.param_count() + second.param_count()
MARKS_OUT = first.mark_count() + second.mark_count()
N_OUT = first.step_count() + second.step_count() - 1
```

The subtraction skips the second linkage’s implicit `Start`.

This needs no additional measurement pass: output length follows directly from the exact parts. The macro should call one low-level combination implementation and return exact fixed storage.

Do not require application code to write:

```rust
{ First::STEP_COUNT + Second::STEP_COUNT - 1 }
```

The constants should exist for inspection and type construction, but the macro can perform this mechanical arithmetic.

## Keep `linkage_with_joint_spheres!`

`with_joint_spheres` changes program structure according to the contents of the input, so a structural macro is justified:

```rust
linkage_with_joint_spheres!(Armatron1::fixed(), 0.15)
```

Its count helper and materializer must share the same definition of move-like instructions. Do not maintain two lists that can drift.

## Add One Generic Fluent Extension Macro

Remove:

```rust
linkage_restore!
linkage_display_style!
```

Do not add future macros for `.pen_color`, `.sphere_param`, `.yaw`, or other ordinary DSL methods.

Provide one generic two-pass extension operation:

```rust
linkage_extend!(scene;
    .restore("scene origin")
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("close hand", 0.5, 0.0)
)
```

It must:

1. count the appended ordinary steps;
2. resize the base to `base.step_count() + appended_step_count`;
3. replay the fluent chain on the resized base;
4. return exact fixed storage;
5. preserve all ordinary DSL validation.

The first version may support only chains that preserve `DOF` and `MARKS`:

- fixed and parameterized motion;
- pen state, color, and width;
- disks and spheres;
- restoring an existing mark.

Reject `define_param` and the creation of a new mark with a clear compile-time diagnostic until an exact semantic-dimension design exists. Do not silently over-allocate `DOF` or `MARKS`.

Use shared macro infrastructure so the extension counter and fixed builder recognize the same ordinary step methods.

## Kill Public Compaction

Remove:

```rust
LinkageFixed::compact
LinkageBuf::compact
linkage_compact!
LinkageFixed::__compact_step_count
```

Remove or rewrite documentation and tests that present compaction as a separate optimization stage.

### Why

Parameter specialization already runs:

1. fixed no-op removal;
2. adjacent fixed-step merging;
3. another no-op removal.

For the current pirouette body, measurement shows:

```text
Before the later compact call:
active steps: 384
concrete value size: 17,432 bytes

After the later compact call:
active steps: 384
concrete value size: 12,504 bytes
```

The later call removes zero additional active steps. It only changes backing capacity from `538` to `384`, saving 4,928 bytes in the measured host representation.

Preserve the storage saving, but call it what it is: exact final materialization.

Keep the cleanup pipeline private and invoke it from specialization where it already performs useful work. Do not expose an independently named “optimizer” that applications are expected to remember to call.

## Make Exact Storage Part of View Materialization

Provide:

```rust
const LINKAGE: LinkageView<3, 6> =
    linkage_view!(SPECIALIZED_EXPRESSION);
```

`linkage_view!` must:

1. read the expression’s active `step_count()`;
2. copy active steps into exactly sized fixed backing;
3. promote that backing;
4. return `LinkageView<DOF, MARKS>`;
5. erase `N` from the runtime-facing type.

It must not rerun the removed public compaction pipeline. Specialization has already performed its cleanup. View materialization only shrinks backing storage to the active length.

Named `linkage_program!` declarations should perform the same exact resize internally before exposing `view()`.

The low-level resize helper may need to be public for downstream macro expansion. If so:

- prefix it with `__`;
- use `#[doc(hidden)]`;
- explain that it is public only because exported macros expand in downstream crates.

## `?Sized` View Representation

Revisit the feasibility of sharing one storage representation:

```rust
LinkageStorage<DOF, MARKS, [Step; N]>
LinkageStorage<DOF, MARKS, [Step]>
```

with:

```rust
Steps: ?Sized
```

The fixed form would own `[Step; N]`; the view would borrow the unsized `[Step]` tail. This is analogous to the audio buffer design and naturally erases `N`.

Perform a focused stable-Rust spike covering:

- const fixed construction;
- safe array-to-slice unsizing through `.view()`;
- promotion to a `'static` borrowed view;
- `len()`, `poses()`, and `draw_items_3d()`;
- fixed transformations remaining ergonomic;
- no `unsafe`.

Adopt it only if it removes the separate field-by-field `LinkageView` wrapper without complicating transformations, trait implementations, or diagnostics. Otherwise retain the existing view struct. Do not expose both representations.

This decision is independent of the surface macro cleanup and must not delay removal of the poor operation-specific macros.

## Do Not Replace Flattened Composition with `&dyn` Sequences

An audio-like array of trait-object views is appealing:

```rust
&[&dyn LinkagePart]
```

Do not use it as the primary linkage representation in this change.

Unlike independent audio clips, composed linkage parts share:

- the continuing pose;
- pen and drawing state;
- parameter-index remapping;
- mark definitions and restores;
- specialization and rewriting across the flattened program.

A trait-object sequence would require a new runtime interpreter, indirect dispatch, runtime parameter slices, and careful cross-part mark semantics. It may later be useful for independent scene layers, but it is not a simpler replacement for current `combine`.

Keep final runtime programs flat, exact, and statically checked.

## Target Examples

### Simple imported program

Replace:

```rust
linkage_fixed_const! {
    const LINKAGE0 =
        linkage_fixed!("../assets/examples/clock.lb.rs", 2, 2);
}
const LINKAGE: LinkageView<2, 2> = LINKAGE0.view();
```

with:

```rust
linkage_program! {
    ClockLinkage {
        file: "../assets/examples/clock.lb.rs",
        dof: 2,
        marks: 2,
    }
}

const LINKAGE: LinkageView<2, 2> = ClockLinkage::view();
```

### Pirouette specialization

Target:

```rust
linkage_program! {
    Pirouette {
        file: "../assets/mocap/pirouette.lb.rs",
        dof: 132,
        marks: 6,
    }
}

const LINKAGE: LinkageView<3, 6> = linkage_view!(
    Pirouette::fixed()
        .freeze_param_name::<131>("l_shin_yrotation", 57.6)
        .retain_param_names::<3>(&[
            "head_yrotation",
            "l_shldr_zrotation",
            "r_shldr_zrotation",
        ])
);
```

Keeping the meaningful `DOF` arguments is acceptable. No step capacity appears.

### Armatron

Use named file programs for the three reusable source files. Keep `linkage_combine!` and `linkage_with_joint_spheres!` structural. Use `linkage_extend!` for ordinary fluent instructions.

Conceptually:

```rust
const LINKAGE: LinkageView<15, 4> = linkage_view!(
    linkage_extend!(
        linkage_combine!(
            linkage_combine!(
                CameraControl::fixed(),
                Grid9x9::fixed(),
            ),
            linkage_with_joint_spheres!(Armatron1::fixed(), 0.15),
        );
        .restore("scene origin")
    )
);
```

The actual Armatron program must still append the ghost arm before applying its red style and hand sphere. Preserve behavior and arrange the structural and fluent operations in the correct order; do not blindly copy the abbreviated conceptual expression.

If the final expression becomes unreadably nested, declare one or two meaningfully named derived programs with the `program:` form. Do not recreate Luna’s one-wrapper-per-line structure.

## Remove Example-Specific Core API

Delete `__append_display_style` and `linkage_display_style!`.

Core linkage APIs must not encode “the two display-style steps used by the Armatron example.” The generic `linkage_extend!` mechanism handles this without embedding an application concept in the library.

Likewise delete `__restore` and `linkage_restore!` once restore is supported through the generic extension mechanism.

## Tests

Add or update tests for:

1. `linkage_program!` file declarations measure exact `STEP_COUNT`.
2. Supplied `DOF` and `MARKS` are validated in both directions.
3. Multiple named programs may be declared in one macro invocation.
4. `fixed()` returns exact fixed storage.
5. `view()` has `'static` backing and hides `N`.
6. The same `.lb.rs` file constructs equivalent fixed and growable programs.
7. `linkage_combine!` derives exact output dimensions and skips the second `Start`.
8. `linkage_with_joint_spheres!` remains exact.
9. `linkage_extend!` counts and replays every supported ordinary DSL operation.
10. Unsupported parameter or new-mark declarations in `linkage_extend!` fail clearly.
11. `linkage_view!` shrinks a specialized `LinkageFixed<4, 6, 538>` with 384 active steps to backing capacity 384.
12. View materialization does not change active steps, poses, draw items, parameter names, or marks.
13. No public `compact`, `linkage_compact!`, `linkage_restore!`, or `linkage_display_style!` remains.
14. Armatron, Skeleton Clock, Ballet, and Clock retain their current behavior.
15. Fixed/default builds remain `no_std` and allocation-free.

Retain low-level tests that intentionally exercise oversized storage only if oversized storage remains a supported internal capability. Application examples and primary documentation must not contain a manually maintained step capacity.

## Documentation

The primary story should be:

```text
.lb.rs program
    ├── measured and materialized as exact LinkageFixed during const evaluation
    ├── promoted and exposed as LinkageView with N erased
    └── interpreted into LinkageBuf when growable runtime storage is wanted
```

Explain that:

- the file describes a linkage program, not its storage;
- `DOF` and `MARKS` remain visible because callers care about them;
- `N` is discovered and hidden;
- ordinary instructions remain fluent;
- structural operations use a small number of macros because stable Rust cannot express their output const arithmetic in method return types;
- specialization performs cleanup;
- final view materialization shrinks storage and does not perform a separately advertised “compaction” pass.

Use `rust,no_run` doctests and link every public macro to a primary compilable example.

## Migration Order

1. Add the new named `linkage_program!` API and focused tests.
2. Add `linkage_extend!`.
3. Add exact `linkage_view!` materialization.
4. Migrate Clock as the smallest example.
5. Migrate Ballet and Skeleton Clock.
6. Migrate Armatron, using no example-specific core macros.
7. Remove `linkage_fixed_const!`.
8. Remove `linkage_restore!`, `linkage_display_style!`, and their helpers.
9. Remove public compaction APIs while retaining private specialization cleanup.
10. Run formatting, focused tests, example builds, and full local CI.

Do not perform removal before the replacement API compiles and its equivalence tests pass.

## Verification

Run:

```text
cargo test -p linkage-blaze-core --features alloc
cargo check-all
```

Also build all affected example feature combinations and the embedded targets that retain the large pirouette linkage.

Measure at least one release artifact before and after migration. Confirm that eliminating the explicit compaction call does not restore the unused 154-step tail to firmware. The final exact view backing should retain the approximately 4,928-byte concrete-storage saving observed in the host-layout measurement.

## Completion Criteria

The follow-up is complete when:

- file declarations resemble the audio macro rather than nested declaration macros;
- no application code names linkage `N`;
- `DOF` and `MARKS` remain visible and validated;
- ordinary DSL instructions remain fluent;
- only genuinely structural operations use dedicated macros;
- no Armatron-specific helper exists in the core API;
- public compaction is gone;
- exact final storage is automatic when producing a view;
- fixed and growable construction still share the same `.lb.rs` source;
- all migrated examples and local CI pass;
- release measurement confirms that exact backing storage is retained.
