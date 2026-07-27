<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Derive Linkage File Metadata with `linkage_file!`

## Purpose

Introduce one clear, canonical application-facing declaration for access to a
linkage `.lb.rs` file:

```rust
linkage_file! {
    ClockLinkage {
        file: "../assets/examples/clock.lb.rs",
    }
}
```

The declaration must derive `DOF`, `MARKS`, and step capacity from the file
during const evaluation. Users must not count or repeat those values.

This change is intentionally narrow. Establish and migrate the file-access API
without redesigning linkage composition, specialization, views, promotion, or
the fluent DSL. Those APIs will be reviewed separately after this part works.

Do not add compatibility aliases or preserve the current file form of
`linkage_program!`.

## User Model

`linkage_file!` defines access to an external linkage file in the same general
way that Device Envoy's `pcm_clip!` defines access to an external audio file.
It declares metadata and constructors; it does not itself choose or materialize
the application's final storage.

After:

```rust
linkage_file! {
    ClockLinkage {
        file: "../assets/examples/clock.lb.rs",
    }
}
```

the generated namespace-like type must provide:

```rust
ClockLinkage::DOF
ClockLinkage::MARKS
ClockLinkage::STEP_COUNT
ClockLinkage::fixed()
ClockLinkage::buf() // only with `alloc`
```

Do not generate `ClockLinkage::VIEW`. In particular, do not make
`linkage_file!` silently choose promotion, static backing, or another final
storage form. The caller remains responsible for choosing fixed storage, a
view, or growable storage.

Keep using a zero-sized type plus associated constants and functions, as the
current declaration macro does. This permits conventional UpperCamelCase names
such as `ClockLinkage` and method-like access such as
`ClockLinkage::fixed()`. Do not change this into a lowercase Rust module as part
of this work.

## Syntax and Visibility

Support ordinary Rust visibility before the generated name:

```rust
linkage_file! {
    PrivateLinkage {
        file: "private.lb.rs",
    }

    pub(crate) CrateLinkage {
        file: "crate.lb.rs",
    }

    pub PublicLinkage {
        file: "public.lb.rs",
    }
}
```

Requirements:

- no visibility means private;
- accept ordinary `$vis` forms including `pub`, `pub(crate)`, and
  `pub(super)`;
- apply the captured visibility to the generated zero-sized type;
- associated constants and constructors may be `pub`, because the enclosing
  type's visibility still controls whether callers can reach them;
- retain support for attributes on individual declarations;
- retain support for multiple declarations in one invocation;
- do not add a custom `visibility:` field.

## Derived Metadata

The user must not provide:

```rust
dof: 2,
marks: 2,
```

Derive all three structural values from the included `.lb.rs` body:

- `DOF` is the number of parameter slots created by `define_param`;
- `MARKS` is the number of distinct mark slots after the existing mark-name
  semantics are applied;
- `STEP_COUNT` is the exact number of active steps, including the implicit
  start step, under the existing convention.

Preserve all current validation. Derivation must not weaken errors for invalid
parameter defaults, unknown parameter references, restores of unknown marks,
or malformed linkage bodies.

### Suggested const-evaluation passes

Extend or replace the current internal measurement builder as needed, but keep
the implementation private. A suitable approach is:

1. Evaluate the file with a metadata counter that records:
   - step count;
   - number of `define_param` calls;
   - number of `mark` calls.
2. Use those values as safe capacities for a const-evaluated candidate
   `LinkageFixed`.
3. Let the candidate apply the real name-resolution rules and report its exact
   `param_count()` and `mark_count()`.
4. Expose those exact results as `DOF` and `MARKS`.
5. Evaluate the file into the final exact
   `LinkageFixed<DOF, MARKS, STEP_COUNT>` returned by `fixed()`.

Counting mark calls alone is not sufficient for the public `MARKS` value:
re-marking an existing name currently reuses its slot. Do not change that
behavior merely to simplify measurement. The intermediate mark-call count may
be an upper bound, but the public final type and `MARKS` constant must use the
exact distinct-slot count.

Do not introduce an arbitrary public maximum for parameters, marks, or steps.

## Generated Constructors

### `fixed()`

Generate a `const fn fixed()` returning the exact inferred type:

```rust
pub const fn fixed() -> LinkageFixed<
    { ClockLinkage::DOF },
    { ClockLinkage::MARKS },
    { ClockLinkage::STEP_COUNT },
>
```

The exact implementation spelling may differ, but callers must not supply any
of the three dimensions.

### `buf()`

With the `alloc` feature, generate:

```rust
pub fn buf() -> LinkageBuf<
    { ClockLinkage::DOF },
    { ClockLinkage::MARKS },
>
```

It must load the same `.lb.rs` body through the growable implementation. This
preserves the important property that one linkage file supports both
const-evaluated fixed loading and runtime dynamic loading.

Do not generate `buf()` for expression-based derived programs in this change.

## Relationship to Existing Macros

Split the current responsibilities cleanly:

- `linkage_file!` handles named external `.lb.rs` declarations;
- the `program:` form of `linkage_program!` may remain for named programs
  constructed from linkage expressions;
- remove the `file:` form from `linkage_program!`;
- do not generate a `VIEW` associated constant from either file declarations
  or as a replacement compatibility path.

Keep `linkage_fixed!` and `linkage_buf!` as the existing lower-level
implementation/one-off facilities for now. Do not redesign their signatures in
this task. Their documentation should identify `linkage_file!` as the normal
choice for named application assets, rather than presenting all file-loading
paths as equally preferred.

Do not change these APIs except where mechanically required for the new file
declaration:

- `LinkageFixed`;
- `LinkageBuf`;
- `LinkageView`;
- `linkage_view!`;
- `linkage_combine!`;
- `linkage_extend!`;
- `linkage_with_joint_spheres!`;
- parameter specialization;
- mark and restore semantics;
- the `.lb.rs` callback format.

## Repository Migration

Use `linkage_file!` everywhere in this repository that declares named access
to a linkage `.lb.rs` file.

At minimum, migrate:

- Clock;
- Skeleton Clock;
- Ballet;
- all Armatron linkage files;
- Pirouette specialization tests;
- fixed/buffer equivalence tests;
- documentation examples and doctests;
- utilities or editor examples that declare named linkage files.

For example, replace:

```rust
linkage_program! {
    pub ClockLinkage {
        file: "../assets/examples/clock.lb.rs",
        dof: 2,
        marks: 2,
    }
}
```

with:

```rust
linkage_file! {
    pub ClockLinkage {
        file: "../assets/examples/clock.lb.rs",
    }
}
```

Do not opportunistically rewrite surrounding composition or view code during
this migration. Remove only declarations and imports made obsolete by the new
file macro.

Preserve each declaration's required visibility. Do not make every generated
type public merely because the old macro accidentally emitted public structs.

## Documentation

Document `linkage_file!` as the primary way to declare a named linkage file.
Its main example should show:

1. the minimal file declaration with no dimensions;
2. discovered metadata such as `ClockLinkage::DOF`;
3. `ClockLinkage::fixed()`;
4. `ClockLinkage::buf()` behind `alloc`, demonstrating that the same source
   file supports static and dynamic loading.

Explain that the macro declares access and constructors but does not choose the
application's final storage. Do not introduce the rest of the macro family in
the opening example.

Update `linkage_program!` documentation so it describes only named programs
constructed from expressions. Update `linkage_fixed!` and `linkage_buf!`
documentation to point named-file users toward `linkage_file!`.

## Tests

Add focused tests for:

1. A representative file derives the expected `DOF`, `MARKS`, and
   `STEP_COUNT`.
2. `fixed()` returns and evaluates the expected exact linkage.
3. Under `alloc`, `buf()` and `fixed()` from the same declaration are
   behaviorally equivalent.
4. A synthetic file with a repeated mark name derives the exact number of
   distinct mark slots, not the number of `mark` calls.
5. A file with multiple parameter definitions derives its exact `DOF`.
6. Invalid parameter references and invalid restores still fail during
   compilation.
7. A private declaration is inaccessible outside its module.
8. `pub(super)`, `pub(crate)`, and `pub` declarations have the expected
   reach.
9. Attributes on declarations and multiple declarations in one macro
   invocation continue to work.
10. The no-`alloc` configuration does not expose or require `buf()`.

Prefer small synthetic `.lb.rs` fixtures for metadata edge cases. Keep the
existing real Clock and Armatron coverage for integration confidence.

## Required Verification

Run formatting and the focused core tests while iterating. Before considering
the implementation complete, run from the Linkage Blaze repository root:

```text
cargo check-all
```

It must complete successfully. This command covers the core crate, allocation
support, UI tests, doctests, rendered examples, embedded targets, WASM, and the
cross-repository Device Envoy checks.

Cargo lock-wait status lines and known non-fatal toolchain diagnostics are not
test failures, but do not interrupt or report the command as passing before it
exits successfully.

## Acceptance Criteria

- A user declares a named linkage file with only its name and path.
- `DOF`, exact distinct `MARKS`, and exact `STEP_COUNT` are derived at compile
  time.
- The declaration honors ordinary Rust visibility.
- The generated surface consists of metadata and constructors, not a stored
  `VIEW`.
- The same declaration supports `fixed()` and, with `alloc`, `buf()`.
- All named file declarations in the repository use `linkage_file!`.
- `linkage_program!` no longer accepts `file:`.
- The rest of the linkage API is unchanged except for necessary migration
  edits.
- Documentation gives `linkage_file!` one clear file-access meaning.
- `cargo check-all` exits successfully.
