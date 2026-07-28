<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Make `linkage_file!` Generate One Idiomatic Rust Module

## Status and precedence

This specification supersedes the user-facing API and naming portions of
`LINKAGE_FILE_MACRO_SPEC.md`.

The previous work successfully established that a linkage file can derive its
own `DOF`, `MARKS`, and `STEP_COUNT`. Preserve that behavior. This follow-up
changes how the resulting API is organized and named.

## Purpose

Make a `linkage_file!` declaration read like an ordinary Rust module that
provides access to one external linkage file:

```rust
linkage_file! {
    clock_linkage {
        file: "../assets/examples/clock.lb.rs",
    }
}

const CLOCK_LINKAGE: clock_linkage::View =
    clock_linkage::view();
```

This is both:

- an API change: `linkage_file!` generates a real module with module-relative
  types, metadata, and constructors;
- a style change: modules use `snake_case`, types use `UpperCamelCase`, and
  materialized constants use `SCREAMING_SNAKE_CASE`.

The declaration must hide the derived const-generic dimensions from ordinary
call sites. A caller loading a simple linkage must not write
`LinkageView<DOF, MARKS>`, call `fixed()` as an exposed intermediate step, or
invoke `linkage_view!`.

Keep the rest of the linkage API as unchanged as practical. Composition,
specialization, and other linkage macros will be reviewed separately.

Do not add compatibility aliases for the current generated zero-sized types.

## Required public syntax

`linkage_file!` supports exactly one file declaration per invocation:

```rust
linkage_file! {
    clock_linkage {
        file: "../assets/examples/clock.lb.rs",
    }
}
```

It must not accept several module declarations in one invocation. Write
separate invocations:

```rust
linkage_file! {
    camera_control {
        file: "../../assets/examples/armatron/camera_control.lb.rs",
    }
}

linkage_file! {
    grid9x9 {
        file: "../../assets/examples/armatron/grid_9x9.lb.rs",
    }
}

linkage_file! {
    armatron1 {
        file: "../../assets/examples/armatron/armatron1.lb.rs",
    }
}
```

The singular macro name must have a singular responsibility: one invocation
declares access to one file.

Retain support for ordinary attributes on the generated module when they are
useful and testable.

## Generated module

Generate a real Rust module, not a zero-sized namespace struct.

For:

```rust
linkage_file! {
    clock_linkage {
        file: "../assets/examples/clock.lb.rs",
    }
}
```

the public surface inside the enclosing scope must conceptually be:

```rust
mod clock_linkage {
    pub const DOF: usize = /* derived */;
    pub const MARKS: usize = /* derived */;
    pub const STEP_COUNT: usize = /* derived */;

    pub type Fixed =
        LinkageFixed<DOF, MARKS, STEP_COUNT>;
    pub type View =
        LinkageView<'static, DOF, MARKS>;

    pub const fn fixed() -> Fixed {
        /* exact fixed representation */
    }

    pub const fn view() -> View {
        /* exact promoted backing and view */
    }

    // With `alloc` only:
    pub type Buf = LinkageBuf<DOF, MARKS>;
    pub fn buf() -> Buf {
        /* growable representation from the same file */
    }
}
```

The exact internal expansion may differ. The observable API and behavior must
match this model.

### Metadata

Preserve compile-time derivation of:

- `DOF`;
- exact distinct `MARKS`;
- exact `STEP_COUNT`.

Users must not provide any of these values.

Preserve validation of parameter definitions and references, mark and restore
semantics, and malformed `.lb.rs` input. Repeated uses of the same mark name
must continue to consume only one mark slot.

### `Fixed`

`Fixed` names the exact fixed-storage representation inferred from the file.
`fixed()` returns it without requiring const-generic arguments.

This remains available for composition and specialization:

```rust
linkage_combine!(
    camera_control::fixed(),
    grid9x9::fixed(),
    linkage_with_joint_spheres!(armatron1::fixed(), 0.15),
)
```

Do not redesign those transformation APIs in this task.

### `View`

`View` is a module-relative alias that hides the derived `DOF` and `MARKS` from
the caller while preserving them in the underlying strong type.

`view()` must hide:

- construction of fixed storage;
- exact backing-size calculation;
- const promotion or other required static backing;
- the current `linkage_view!` implementation detail.

The ordinary application declaration must be:

```rust
const CLOCK_LINKAGE: clock_linkage::View =
    clock_linkage::view();
```

It must not be:

```rust
const CLOCK_LINKAGE: LinkageView<2, 2> =
    linkage_view!(clock_linkage::fixed());
```

Do not expose a public `VIEW` constant. The public operation is `view()`, which
lets the caller decide whether and where to materialize a named const value.

The implementation may use private const values or promotion internally.
Those details must not appear in application code or public documentation.

### `Buf`

With `alloc`, retain dynamic loading from the same source file:

```rust
let clock_linkage = clock_linkage::buf();
```

Expose the module-relative `Buf` alias if it is useful for explicit type
annotations:

```rust
let clock_linkage: clock_linkage::Buf =
    clock_linkage::buf();
```

Do not expose `Buf` or `buf()` without `alloc`.

## Visibility

Support ordinary optional Rust visibility on the generated module:

```rust
linkage_file! {
    private_linkage {
        file: "private.lb.rs",
    }
}

linkage_file! {
    pub(crate) crate_linkage {
        file: "crate.lb.rs",
    }
}

linkage_file! {
    pub public_linkage {
        file: "public.lb.rs",
    }
}
```

Requirements:

- omitted visibility produces a private module;
- support `pub`, `pub(crate)`, and `pub(super)`;
- apply visibility to the generated module;
- items such as `View`, `view()`, and metadata are public within that module,
  while the module's own visibility controls their external reach;
- do not add a custom `visibility:` field.

Production examples must omit visibility unless another module or downstream
crate actually uses the generated module. Do not preserve accidental public
API merely because an earlier macro expansion always emitted a public struct.

In particular, Clock, Ballet, Skeleton Clock, and the Armatron linkage-file
modules should be private unless a concrete cross-module use proves otherwise.

## Naming style

Treat this as a deliberate repository-wide naming migration.

### Generated modules

Use normal Rust `snake_case`:

```rust
clock_linkage
camera_control
grid9x9
armatron1
pirouette
```

Do not use UpperCamelCase module names such as:

```rust
ClockLinkageFile
CameraControl
Grid9x9
Armatron1
```

Do not suppress `non_snake_case` to retain those names.

### Module contents

Use concise names because the module already provides context:

```rust
clock_linkage::View
clock_linkage::Fixed
clock_linkage::Buf
clock_linkage::view()
clock_linkage::fixed()
clock_linkage::buf()
clock_linkage::DOF
clock_linkage::MARKS
clock_linkage::STEP_COUNT
```

Avoid redundant forms such as:

```rust
clock_linkage::ClockLinkageView
clock_linkage::clock_linkage_view()
clock_linkage::CLOCK_LINKAGE_DOF
```

### Materialized values

Name application constants after what the value represents:

```rust
const CLOCK_LINKAGE: clock_linkage::View =
    clock_linkage::view();
```

Use normal `snake_case` for local variables:

```rust
let clock_linkage: clock_linkage::Buf =
    clock_linkage::buf();
```

Do not add `FILE`, `FIXED`, or `VIEW` to an application variable merely to
mirror construction details when the shorter semantic name is unambiguous.

## One-file-per-invocation diagnostics

The macro pattern must accept exactly one declaration. Add a compile-fail test
for an attempted batch declaration:

```rust,ignore
linkage_file! {
    first_linkage {
        file: "first.lb.rs",
    }
    second_linkage {
        file: "second.lb.rs",
    }
}
```

A focused custom `compile_error!` explaining that `linkage_file!` accepts one
file per invocation is preferable if it does not substantially complicate the
macro. At minimum, the batch form must not compile.

## Repository migration

Migrate every named linkage-file declaration to the module form, including:

- Clock;
- Ballet;
- Skeleton Clock;
- the three Armatron input files;
- Pirouette specialization tests;
- fixed/buffer equivalence tests;
- visibility UI tests;
- `linkage_file!` doctests and documentation;
- any utility or editor example that declares named access to an `.lb.rs`
  file.

For simple application code, migrate:

```rust
linkage_file! {
    pub ClockLinkage {
        file: "../assets/examples/clock.lb.rs",
    }
}

const LINKAGE: LinkageView<2, 2> =
    linkage_view!(ClockLinkage::fixed());
```

to:

```rust
linkage_file! {
    clock_linkage {
        file: "../assets/examples/clock.lb.rs",
    }
}

const CLOCK_LINKAGE: clock_linkage::View =
    clock_linkage::view();
```

Update uses of the materialized constant consistently. Do not retain a generic
`LINKAGE` name when the clearer `CLOCK_LINKAGE` name materially improves the
example.

For Armatron, use three separate private declarations:

```rust
linkage_file! {
    camera_control {
        file: "../../assets/examples/armatron/camera_control.lb.rs",
    }
}

linkage_file! {
    grid9x9 {
        file: "../../assets/examples/armatron/grid_9x9.lb.rs",
    }
}

linkage_file! {
    armatron1 {
        file: "../../assets/examples/armatron/armatron1.lb.rs",
    }
}
```

Then update only the necessary paths:

```rust
camera_control::fixed()
grid9x9::fixed()
armatron1::fixed()
```

Do not use this migration as an opportunity to redesign
`linkage_program!`, `linkage_combine!`, `linkage_extend!`,
`linkage_with_joint_spheres!`, or specialization. Surrounding code may be
renamed where required for consistent style, but preserve behavior and
operation order.

## Relationship to other linkage APIs

After this change:

- `linkage_file!` declares one external file as one real module;
- `linkage_program!` continues to name a program constructed from an
  expression;
- `linkage_fixed!` and `linkage_buf!` remain lower-level/one-off facilities;
- `linkage_view!` remains available for arbitrary expressions, but simple
  linkage-file users do not call it.

Do not merge these concepts or remove the other macros in this task.

Rename internal helpers whose old names would now be actively misleading. For
example, rename `__linkage_program_buf` to a `linkage_file`-specific name.
Keep exported macro helpers documented as implementation details according to
the repository's macro-helper convention.

## Documentation

Make the primary `linkage_file!` example exactly the simple Clock-style use:

```rust
linkage_file! {
    clock_linkage {
        file: "assets/examples/clock.lb.rs",
    }
}

const CLOCK_LINKAGE: clock_linkage::View =
    clock_linkage::view();
```

The opening example must not show:

- manually supplied dimensions;
- `LinkageView<2, 2>`;
- `linkage_view!`;
- a required intermediate `fixed()`;
- unnecessary visibility;
- multiple file declarations in one invocation.

After the simple example, document `fixed()` for composition, `buf()` for
dynamic loading, the derived metadata, and optional visibility.

Document that the macro generates a real module. Explain the naming convention
briefly so users understand why the declared identifier is `snake_case`.

## Tests

Retain and adapt the existing metadata and behavioral tests. Add or update
focused coverage for:

1. `view()` is usable in a const declaration typed as the generated
   module-relative `View`.
2. `View` contains the derived `DOF` and `MARKS` without repeating them at the
   call site.
3. `fixed()` returns the generated module-relative `Fixed`.
4. With `alloc`, `buf()` returns `Buf` and remains behaviorally equivalent to
   `fixed()` for the same file.
5. Repeated marks still produce exact `MARKS`.
6. A private module is inaccessible outside its parent.
7. `pub(super)`, `pub(crate)`, and `pub` visibility behave normally.
8. One invocation with two file declarations fails to compile.
9. Separate invocations for separate files compile and compose correctly.
10. No-`alloc` builds expose neither `Buf` nor `buf()`.
11. Existing Clock, Ballet, Skeleton Clock, and Armatron rendered behavior is
    unchanged.

Do not use redundant `const _` declarations in production examples merely to
test generated types. Put such assertions in focused macro tests.

## Required verification

Run formatting and focused tests while iterating. Before considering the
implementation complete, run from the Linkage Blaze repository root:

```text
cargo check-all
```

It must exit successfully. Do not report the command as passing while it is
still running, and do not confuse Cargo lock-wait status lines with failures.

## Acceptance criteria

- Each `linkage_file!` invocation declares exactly one file.
- It generates one real, idiomatically named Rust module.
- `DOF`, exact `MARKS`, and exact `STEP_COUNT` remain derived.
- The module exposes `Fixed`, `View`, their constructors, and optional
  allocation-backed `Buf`.
- Simple use is exactly:

  ```rust
  const CLOCK_LINKAGE: clock_linkage::View =
      clock_linkage::view();
  ```

- Simple use contains no explicit dimensions, `fixed()` intermediate, or
  `linkage_view!`.
- Visibility is optional and follows ordinary Rust syntax.
- Production examples omit visibility unless it is genuinely required.
- Module, type, function, constant, and variable names follow Rust and
  repository conventions.
- All named file declarations in the repository use the new module API.
- Other linkage APIs and behavior remain substantially unchanged.
- `cargo check-all` exits successfully.
