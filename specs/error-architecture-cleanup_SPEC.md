<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Error architecture cleanup

## Objective

Bring the Linkage Blaze and Device Envoy error paths into one consistent
pattern:

- Each relevant module has one primary error type named `Error`.
- Error enums carry the original error returned by fallible dependencies.
- `derive_more::From` generates ordinary conversions wherever the conversion
  is a direct variant insertion.
- Callers propagate errors with plain `?`.
- `map_err`, handwritten `From` implementations, and error flattening remain
  only where they perform an intentional semantic translation that cannot be
  represented by a direct `From` variant.

The Device Envoy repository is the sibling checkout at `../mcu/device-envoy`.
Its existing user changes must be preserved while this work is implemented.

## Required design

### Primary error names

Use `Error` for the primary error type of a module. Rename module-level types
such as `RenderError` and `MocapParseError` when they are the sole public or
private error abstraction for that module. Keep descriptive names only for
genuinely distinct nested leaf errors, such as a parser-specific error that is
embedded by a higher-level `Error`.

Every error-producing module should expose an appropriate `Result` alias when
that improves readability, with the module's `Error` as its default error
type.

### Derived conversions

Use `#[derive(Debug, derive_more::From)]` (or the repository's equivalent
`use derive_more::From` form) for enums whose variants directly contain source
errors. Add `#[from(ignore)]` only for generic foreign errors where deriving a
blanket conversion would conflict with a concrete conversion under Rust's
coherence rules. Document that exception at the enum.

Do not write a handwritten `impl From<T> for Error` when the implementation is
only `Self::Variant(value)`. Handwritten conversions may remain when they:

- destructure a compound error and intentionally map it to a different public
  error taxonomy;
- convert an infallible or unit error into a meaningful domain variant; or
- preserve a platform boundary that cannot be expressed as a direct variant.

### Propagation and payload preservation

Prefer:

```rust
let value = fallible_operation()?;
```

over `map_err(Error::Variant)?`, `map_err(|_| Error::Variant)?`, or a manual
`From` implementation. Every wrapper variant must retain the source value
unless the source is intentionally uninformative, such as `()` or
`Infallible`.

Do not use unit variants or ignored parameters to discard useful diagnostics.
In particular, top-level example errors must retain the complete nested error
(`DeviceEnvoy(error)`, `Cyd(error)`, `Ballet(error)`, etc.) instead of reducing
it to a label-only variant.

`map_err` is acceptable for deliberate semantic normalization, such as
turning a low-level formatting failure into a stable public domain error, but
the reason must be clear in a nearby comment or documentation. It is not
acceptable merely to make `?` compile.

## Linkage Blaze scope

### Core and utility crates

- Evaluate whether the core crate's root `Error` and `MarkError` should be
  unified under the module-level naming rule. Preserve `MarkError` only if it
  remains a semantically distinct leaf error.
- Convert `linkage-blaze-utils::RenderError` to the chosen `Error` naming and
  derive direct `From<String>` and `From<linkage_blaze_core::Error>` variants.
- Review `MocapParseError` in `linkage-blaze-utils/src/bvh.rs`; either make it
  the module's `Error` or embed it as a distinct parser leaf error in a module
  `Error`.
- Preserve contextual parser messages where they are part of the public
  behavior. Do not replace useful context with a blind `?`.
- Convert the `xtask` error type's direct I/O conversion to derive-more and
  retain explicit message/context variants only where the operation changes
  the diagnostic meaning.

### RP examples

Update all Linkage Blaze RP examples so their top-level error is named `Error`
or follows the module's established entry-point convention, derives
`derive_more::From`, and stores source errors in its variants. Remove the
label-only `MainError` variants and handwritten conversions that discard their
arguments.

At minimum this includes `ballet`, `armatron`, `armatron_one_spi`, `clock`,
and `skeleton_clock`.

### ESP examples and generation

Apply the same changes to the source/template used to generate the repeated
ESP examples, then regenerate every generated example. Do not make a one-off
edit to a generated output without changing its source of truth.

The Armatron and one-SPI Armatron examples currently flatten platform,
display/touch, and Armatron errors; all of those source values must be retained
in the resulting error enum.

## Device Envoy scope

- Review `device-envoy-core`, `device-envoy-rp`, and `device-envoy-esp` primary
  error enums for direct variants that can use `derive_more::From`.
- Keep `Display`/`Error` derives and `#[error(not(source))]` annotations where
  required by no-std or dependency trait limitations.
- Convert direct `CydError` wrapper conversions to derive-more where possible.
- Review `WritingError`, `SpiWritingError`, flash-block errors, calibration
  errors, and other module-local errors for the primary-name rule and direct
  source preservation.
- Audit `map_err(|_| ...)` in Wi-Fi formatting/storage, flash, portal, and
  platform-boundary code. Replace error discards with source-carrying variants
  unless the conversion is an intentional domain normalization.
- Update Device Envoy example entrypoints consistently, including RP and ESP
  DNS tester examples.

Do not overwrite or rebase the existing modified files in the Device Envoy
working tree. Reconcile overlapping edits manually.

## Exceptions to record explicitly

Each remaining non-plain propagation site should be reviewed and either
removed or documented. Expected valid exceptions include:

- generic platform errors whose blanket `From<F>` would conflict with concrete
  derived conversions;
- conversion of `Infallible` or unit errors into a meaningful domain state;
- deliberate error taxonomy changes at a public platform boundary; and
- contextual parsing or command-runner diagnostics where the added context is
  the actual API behavior.

No exception should silently discard a nontrivial source error.

## Verification

Run the following after implementation:

1. Search both repositories for error declarations, handwritten `From` impls,
   `map_err`, `.ok()`, and label-only error variants.
2. Run Linkage Blaze `just check-all`.
3. Run the corresponding Device Envoy `cargo check-all` command.
4. Build/test host utilities and parser tests.
5. Build the affected RP, WASM, and ESP examples, including regenerated
   examples.
6. Inspect `git diff` and confirm no generated file changed without its source
   template changing first.

## Completion criteria

The work is complete when every reviewed primary module error follows the
`Error`/`derive_more::From`/plain-`?` pattern, all useful source errors remain
inspectable, documented exceptions are intentional, both repositories pass
their full local checks, and no unrelated user changes are modified.

