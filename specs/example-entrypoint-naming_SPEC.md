<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Example entry-point naming cleanup

## Objective

Make the platform-neutral example APIs concise and consistent by relying on
the module name for context. Each example module should expose its primary
long-running entry point as `run`, rather than repeating the example name:

```rust,no_run
armatron::run(&mut cyd, &mut button).await?;
```

The public result type should remain `Exit` where an example returns control
requests. The module already provides the context, so names such as
`ArmatronExit` or `SkeletonClockExit` are unnecessary.

## Scope

### Linkage Blaze examples

Apply the convention to the four modules under
`crates/linkage-blaze-core/src/examples/`:

- `armatron`: rename `armatron` to `run`.
- `ballet`: rename `ballet` to `run`.
- `clock`: rename `clock` to `run`.
- `skeleton_clock`: rename `skeleton_clock` to `run`.

Update every RP, WASM, and ESP caller, including ESP example templates and
all generated platform examples. Remove local aliases such as
`armatron as run_armatron`; callers should import the module and call its
`run` function.

For the clock modules, consider renaming the secondary setup functions
`clock_splash` and `skeleton_clock_splash` to `splash` as well. The module path
already disambiguates them, and this makes them consistent with the DNS Tester
API. Make this part of the implementation only if all callers can be updated
without retaining compatibility aliases.

Keep `Exit` and `Error` as module-local public type names. Do not introduce
example-prefixed aliases or compatibility shims.

### Device Envoy DNS Tester

Treat `device-envoy-examples-core::dns_tester` as the coordinated fifth case.
It already follows the target convention with `run`, `splash`, `wifi_status`,
and `Exit`. Verify that its platform callers and templates remain consistent;
do not rename already-correct APIs as part of the Linkage Blaze changes.

Before implementation, read and follow the Device Envoy repository's
`AGENTS.md`. Changes in that repository should be made separately and should
preserve existing user modifications.

## Design decisions

- The module supplies the example identity; the primary operation supplies the
  generic action name `run`.
- `Exit` is a suitable result type because it describes why the loop returned,
  not which example produced it.
- `Error` remains the module's error type, following the repository's existing
  error naming convention.
- Do not rename private helpers merely to satisfy this convention.
- Do not add backwards-compatibility wrappers or aliases; update all in-tree
  callers in one change.

## Implementation checklist

- [x] Confirm the complete caller and template list with `rg`.
- [x] Rename the four Linkage Blaze primary functions to `run`.
- [x] Update module imports and calls in RP, WASM, and ESP examples.
- [x] Update ESP templates before changing or regenerating generated examples.
- [x] Decide whether the two clock splash functions should become `splash` and
      update their documentation and callers if so.
- [x] Remove obsolete naming TODOs and stale references to the old function
      names.
- [x] Confirm the DNS Tester API remains unchanged and consistent.
- [x] Inspect the final diff to ensure unrelated user changes are preserved.

## Verification

Run formatting and search checks first, then:

1. Run the relevant host tests for the core examples.
2. Build/check the affected RP and WASM examples.
3. Run `just check-all` in Linkage Blaze.
4. If Device Envoy files are changed, run its documented `cargo check-all`
   command as well.
5. Search both repositories for stale calls to `armatron(...)`, `ballet(...)`,
   `clock(...)`, `skeleton_clock(...)`, and `run_armatron`.

## Completion criteria

The four Linkage Blaze example modules expose `run` as their primary entry
point, all in-tree callers compile without aliases or compatibility wrappers,
the optional clock splash cleanup is either completed or explicitly left
unchanged, and the applicable workspace checks pass.

Suggested commit message:

```text
Standardize example run entry points
```
