# Coding Notes for Agents

This file contains shared workspace rules for this repository.

## General Policies

- When autonomous work is interrupted or reaches a stopping point, report the
  current status, what remains, and the recommended next step. If the next
  step is within the current task and safe to perform, perform it rather than
  stopping merely to recommend it.

- In this repository, **devolve** means inlining a code element's behavior at
  its call sites (or into its containing function) and deleting the original
  struct, enum, helper, or other abstraction. Prefer the clearer term
  **inline** in prose when there is no need for the project-specific shorthand.

- Avoid introducing `unsafe` blocks. If a change truly requires `unsafe`, call it out explicitly and explain the justification so the user can review it carefully.
- Do not "fix" warnings or errors by suppressing lints (for example `#[allow(...)]`, crate-level allow attributes, or similar) unless the human explicitly requests that suppression.
- If warnings are caused by obsolete code, delete or refactor the obsolete code instead of hiding the warning.
- Never use `let _ = …` to suppress a `Result`. Handle the error properly. For a non-`Result` value that is intentionally unused, call the function as a plain statement instead of binding it to `_`.
- Never use `.ok()` to discard a `Result`. When the enclosing function can propagate the error, use `?` so it eventually reaches `inner_main`/`main`. In MCU app paths, do not use `.expect(...)` or `.unwrap()` for fallible operations; return the error instead. When the operation truly cannot fail because the error type is `Infallible`, use `.unwrap_infallible()` (from `device_envoy_core::UnwrapInfallible`) rather than `.expect(...)`.
- Do not replace `.unwrap()`, `.expect(...)`, or fallible error propagation with `panic!(...)`. In production code and test helpers, return or propagate the `Result`; in tests, make the test function return `Result` and use `?`, or use an explicit assertion when failure is the behavior being tested.
- Prefer imported, unqualified names over `crate::`-prefixed paths in Rust code. Use `use` (and `super::` where appropriate) to bring local crate items into scope. Macro hygiene paths using `$crate` and required documentation links such as `crate::Type` are exceptions.
- Condense imports from the same crate into one `use` tree. For example, prefer `use crate::{render::Projection, Error, LinkageFixed};` over separate `use crate::render::Projection;` and `use crate::{Error, LinkageFixed};` declarations.
- Prefer a plain `?` over an explicit `.map_err(Variant)?`. Give an error enum a derived `From` (e.g. `derive_more::From`, or `#[from]` on the variant) for each source error so propagation is just `?`. When a generic blanket conversion (such as `impl<F: SomeBound> From<F> for Error<F>` for a device/flush error) would collide under coherence with those concrete `From`s, reserve the clean `?` path for *our own* error types and make the single generic/foreign error the explicit `.map_err(Error::Flush)?` exception — not the other way around. Document the collision at the enum so the asymmetry is not mistaken for an oversight. See `ballet::Error` in `linkage-blaze::examples` for the canonical example.
- Keep the core crate `no_std` and no-allocation unless the user explicitly changes that goal.
- Avoid silent clamping; prefer asserts or typed ranges so out-of-range inputs fail fast.
- Prefer `no_run` doctests; use `ignore` only when absolutely necessary, and call out why.
- Always use `rust,no_run` in doctest fences, not just `no_run`.
- Hide boilerplate in doctests using the `#` prefix when it is noise to the reader but required for compilation, such as `#![no_std]` or ordinary imports.
- When adding docs for modules or public items, link readers to the primary type and keep a single compilable example on that type when practical.
- For public methods, almost every method should either include its own doctest or link directly to a doctest that mentions the method. If one shared doctest covers a family of methods, make each method doc point to that example explicitly.
- Prefer `const` values defined in the local context when they are only used there.
- Prefer explicit `drop(variable)` over anonymous code blocks when the only purpose of the block is to end a borrow or temporary lifetime early.
- Do not add redundant command wrappers that only mirror an existing `cargo` command.
- Do not maintain backwards-compatibility shims or type aliases. Refactor aggressively so the code looks as-if-designed knowing the final requirements.
- Any time a color is defined with numeric components, add a nearby comment with its approximate color name.
- Put primary entry points (`main`, `inner_main`, public run/game-loop functions) near the top of the file before helper types and functions. Do not bury them at the bottom. Concretely, order top-level items as: includes and constants; then the entry point(s); then error types, other helper types, and helper functions. In particular, error enums (e.g. `MainError`) and their `From` impls belong after the entry point, not before it.

### Error Handling

- Prefer one primary module error type named `Error`, with a module-level `Result` alias when it improves readability.
- Preserve useful diagnostics by storing the original source error in wrapper variants. Use `#[derive(Debug, derive_more::From)]` for direct variant conversions and propagate them with plain `?`.
- Keep `map_err`, handwritten `From` implementations, and error flattening only for intentional semantic translation, contextual diagnostics, platform boundaries, or coherence exceptions. Document those exceptions nearby.
- Never replace a useful source error with a label-only or unit variant. Unit and infallible conversions are appropriate only when the source carries no meaningful information.

## Development Tools

Assume the repository's normal development tools are available, including
Rust/Cargo, `just`, Node.js/npm, Python 3, Playwright, `wasm-pack`, and the
required Rust/WASM targets. If a required tool is missing, use the repository's
documented or project-local installation path when available. Do not silently
install system-wide packages or alter unrelated global configuration. For
Playwright, install its browser binaries when needed before diagnosing browser
test failures.

## Specs

In this `AGENTS.md` only, `&#48;` inside a TODO marker means the ASCII digit
zero. Agents must write the real, contiguous marker (`TODO` immediately
followed by one or more `0` digits) in source and spec files. This encoding is
used only here so plain-text release-TODO searches do not find documentation
examples.

Put implementation specs (`*_SPEC.md` and similar planning documents) in the `specs/` directory, not the repo root. Every spec must include a `todo&#48;` comment near the top reminding readers to consider deleting the spec once the work it describes is complete, for example:

```markdown
<!-- todo&#48; consider deleting this spec once the work below is implemented and released. -->
```

## Local CI

`cargo check-all` is the local CI test. It tests, checks, and builds all crates across all targets (embedded, WASM, editor). Run this before pushing to verify everything works. The GitHub CI pipeline mirrors this same test suite.

## Module Structure Convention

Do not create `mod.rs` files.

Correct pattern:

- `src/foo.rs` for a main module file
- `src/foo/bar.rs` for a submodule
- `src/foo/baz.rs` for another submodule

Incorrect pattern:

- `src/foo/mod.rs`

Example:

```rust
// File: src/kinematics.rs
pub mod frame;
pub mod linkage;

// File: src/kinematics/frame.rs
// File: src/kinematics/linkage.rs
```

## Variable Naming Conventions

### Module Context and Prefixes

Prefer concise names when the containing module already supplies the context.
Use the module path to disambiguate related items instead of repeating the
module name in every item. For example, prefer `clock::run`, `clock::splash`,
and `clock::Exit` over `clock::clock`, `clock::clock_splash`, and
`ClockExit`.

Add a prefix only when the item is commonly used outside that module, when it
would otherwise be ambiguous in the same scope, or when the prefix conveys
meaning that the module path cannot. Do not add redundant prefixes or retain
example-, module-, or platform-prefixed compatibility aliases merely to
preserve an older naming scheme.

Variables should generally match their type names converted to snake_case. This improves predictability and encourages better type names.

Avoid abbreviations like `addrs`; spell out `addresses`.

Use standard Rust snake_case for locals, fields, and functions; UpperCamelCase for types; SCREAMING_SNAKE_CASE for constants.

Treat dimension markers like 12x4, 8x12, and 3x4 as suffix qualifiers, not separate words.

Prefer `layout12x4`, `frame8x12`, `matrix3x3`.

Avoid inserting an underscore before the dimension: avoid `layout_12x4`, `matrix_3x3`.

For constants, keep underscores as word separators: prefer `LINKAGE_12X4`, `MATRIX_3X3`, etc.

Type-based naming examples:

- `RobotArm` -> `robot_arm`
- `LinkageStep` -> `linkage_step`
- `Frame3d` -> `frame3d`

Generic/contextual names are acceptable when the type is obvious and verbose naming would be redundant.

Prefer shadowing over adding generic disambiguating suffixes like `_value`. For example, use `previous_tick` again when narrowing or reinterpreting a tick value instead of renaming it to `previous_tick_value` solely to avoid shadowing.

Avoid single-character variables; use descriptive names:

- Bad: `i`, `j`, `x`, `y`, `a`, `b`
- Good: `step_index`, `row_index`, `position_x`, `position_y`, `first_value`, `second_value`

When capturing variables in closures or creating references, append `_ref`:

- `robot_arm` -> `robot_arm_ref`
- `linkage` -> `linkage_ref`

## Comment Conventions

Use `TODO&#48;*` for release-priority TODO items (`TODO` plus one or more trailing zeroes):

```rust
// TODO&#48;&#48; high priority task
// TODO&#48; lower priority consideration
// TODO&#48;&#48;&#48;&#48; release-blocking task with explicit emphasis
// TODO later/non-release work
```

- `TODO&#48;*` means action is required before the next release.
- Plain `TODO` means later/non-release work unless explicitly stated otherwise.
- For code that uses a stable workaround where a clearly better nightly feature exists, add:
  `// TODO_NIGHTLY When nightly feature <feature_name> becomes stable, change this code by <specific change>.`
- When changing code, generally do not remove TODO comments. Move them if needed. If you think they no longer apply, add `(may no longer apply)` rather than deleting them.
- Do not remove debug/test code, commented debugging blocks, or "THIS WORKS" / "THIS DOESN'T" comparison code until the bug is proven fixed and cleanup is explicitly accepted.
- Always suggest a concise 1-2 line commit message when completing work. Present it in a fenced code block so it is easy to copy.
- Do not run the real `cargo publish`. Prepare release notes/versioning/commands, but the actual publish step must be run by the person.

## Documentation Conventions

- When linking to module documentation, name the module in the link text.
- When referring to examples, use the concrete type or module name.
- Use American spelling.
- When making up variable names for examples and elsewhere, never use the prefix "My".
- If an item comes from `crate`, `core`, `std`, or `alloc`, import it with `use` instead of using a fully-qualified path in code. Fully-qualified paths are fine in docs or comments.
- Rust getters should not use a `get_` prefix:
  - Getters: `position()`, `steps()`
  - Setters: `set_position()`, `set_steps()`

Markdown formatting rules:

- Add blank lines before and after lists.
- Add blank lines before and after fenced code blocks.
- Add blank lines before and after headings.
- Ensure consistent list marker style within a file.

## Parsing into a Stronger Type

Prefer shadowing when converting from weaker to stronger types:

```rust
let width = width.parse::<u32>()?;
```

Guidelines:

- Prefer shadowing at the smallest reasonable scope so the new meaning does not leak too far.
- Use assertions or checked conversions before shadowing when truncation or overflow is possible.
- Do not shadow across long spans if it could confuse readers.

## Fixed / View Type Pairs

When a type has both a const-generic "storage" form and a type-erased "view" form, name them with the suffixes `Fixed` and `View` respectively, connected by a `.view()` method. This mirrors the `LinkageFixed` / `LinkageView` pair already in the codebase.

- `FooFixed<const W: usize, …>` — owns the data with compile-time dimensions (lives in flash as a `const`).
- `FooView` — type-erased slice-based view (`&'static [u16]` + runtime size), usable without const generics.
- `.view()` — the conversion method from `FooFixed` to `FooView`.

Example: `Image565Fixed<W, H, N>` → `image.view()` → `Image565View`.

Do not use the `Buf` / plain-name pattern (e.g. `PathBuf` / `Path`) for this; prefer `Fixed` / `View` to stay consistent with the existing codebase convention.

## API Design Patterns

Avoid redundant API paths. Prefer one clear way to do a thing unless there is a strong compatibility or interoperability reason.

- Do not expose both an associated const and an equivalent getter by default.
- If both are temporarily needed during migration, document the canonical one and plan to remove the duplicate.

Avoid the builder pattern. Prefer direct construction and simple data flow:

- Use direct constructors with named parameters.
- Take slices instead of requiring users to construct collections.
- Return arrays or fixed-size types when possible rather than requiring users to build them.

Bad:

todo&#48; this is out of date.

```rust
let linkage = LinkageBuilder::new()
    .yaw(90.0)
    .move_forward(2.5)
    .build();
```

Good:

```rust
let linkage = [
    Step::Yaw(90.0),
    Step::Forward(2.5),
];
```

Bad:

```rust
let mut steps = Vec::new();
steps.push(step1);
steps.push(step2);
arm.simulate(steps);
```

Good:

```rust
let steps = [step1, step2];
arm.simulate(&steps);
```

## Const Generic Turbofish

Omit turbofish const-generic suffixes (e.g. `::<39>`) when the value can be inferred from context. This is almost always the case when the result is assigned to a typed `const`:

```rust
// Bad — redundant turbofish; the 1 is already stated in the const type
const RETAINED: LinkageFixed<1, 0, 5> = BASE.retain_param_names::<1>(&["distance"]);

// Good — type annotation on the const is sufficient
const RETAINED: LinkageFixed<1, 0, 5> = BASE.retain_param_names(&["distance"]);
```

Only write the turbofish when inference would otherwise be ambiguous or when a call site has no surrounding type annotation to infer from.

## Visibility and Documentation

When something should not be in the public API docs, express that through visibility modifiers rather than doc attributes.

Good:

```rust
pub(crate) struct InternalHelper;
struct PrivateHelper;
```

Bad:

```rust
#[doc(hidden)]
pub struct InternalHelper;
```

If something truly should not be in public docs, it should not be `pub` either. Use `pub(crate)` for crate-internal APIs or omit `pub` entirely for private items.

### Exception: Macro Helpers

There is one legitimate use case for `#[doc(hidden)]` on `pub` items: functions or re-exports called by public macros that expand at the call site. These must be `pub` because macro-generated code in downstream crates needs to call them, but they are not part of the user-facing API.

When using `#[doc(hidden)]` for this reason, add a comment explaining why it must be public despite being an implementation detail.

For macro-helper functions, prefix helper names with `__` to clearly signal internal-only usage.
