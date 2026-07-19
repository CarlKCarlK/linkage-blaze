<!-- todo0 consider deleting this spec once the work below is implemented and released. -->
<!-- TODO0000 audit unnecessary core:: and std:: prefixes and replace them with imports. -->

# Example entry-point and button naming consistency

## Objective

Finish the coordinated Linkage Blaze and Device Envoy example cleanup using
module-qualified entry-point calls and type-based button variable names.

This spec follows the API direction in
[`example-entrypoint-naming_SPEC.md`](example-entrypoint-naming_SPEC.md). The
four Linkage Blaze core examples already expose `run`, Clock and Skeleton Clock
already expose `splash`, and DNS Tester already has the desired core API. The
remaining work is caller consistency, local variable naming, and nearby core
organization.

Implementation that changes Device Envoy requires separate authorization and
must follow the Device Envoy repository's `AGENTS.md`.

## Decisions

### Module-qualified entry points

Import each example module and call its public operations through that module:

```rust,no_run
use linkage_blaze_core::examples::clock;

clock::splash(&mut display).await?;
clock::run(&mut display, &clock_sync, &mut button).await?;
```

Do not import `run` or `splash` directly into platform examples. Continue to
import constants and concrete types directly where that remains clearer.

Keep the core API names:

- `armatron::run`, `armatron::Error`, and `armatron::Exit`
- `ballet::run` and `ballet::Error`
- `clock::run`, `clock::splash`, `clock::Error`, and `clock::Exit`
- `skeleton_clock::run`, `skeleton_clock::splash`,
  `skeleton_clock::Error`, and `skeleton_clock::Exit`
- `dns_tester::run`, `dns_tester::splash`, `dns_tester::wifi_status`,
  `dns_tester::Error`, and `dns_tester::Exit`

Do not add compatibility wrappers or aliases.

### Button variable names

Name button locals after their concrete type unless multiple same-type buttons
require a distinguishing suffix:

- `ButtonWatch` -> `button_watch`
- `ButtonRp` -> `button`
- WASM capability button -> `button`
- Add a pin or role suffix only when two or more same-type buttons are in scope.

Function parameters may remain `button` because they describe the role and are
usually generic over the `Button` trait.

## Linkage Blaze work

### Core examples

The primary entry-point names and descriptive generic/error parameters are
already correct. Keep them unchanged.

Apply these organization and documentation fixes:

- In `ballet.rs`, move `StatusTextError` and `Error<FlushError>` directly after
  `run` and before private timing/status helpers.
- In `skeleton_clock.rs`, move `Error<FlushError>` beside `Exit`, after the
  public `run` and `splash` entry points and before private helpers.
- In `skeleton_clock.rs`, change the stale `skeleton_clock` loop link in the
  `splash` documentation to link to `run`.
- Keep Clock's existing entry-point and error organization.
- Keep Armatron's current `run`, `Error<CydError>`, and `Exit` organization.

Do not propagate Armatron-specific visibility, FPS, target-distance, helper, or
UI-state changes to the other examples.

### RP callers

Update these files to import their example module and call `module::run` and,
where applicable, `module::splash`:

- `crates/linkage-blaze-examples-rp/examples/armatron.rs`
- `crates/linkage-blaze-examples-rp/examples/armatron_one_spi.rs`
- `crates/linkage-blaze-examples-rp/examples/ballet.rs`
- `crates/linkage-blaze-examples-rp/examples/clock.rs`
- `crates/linkage-blaze-examples-rp/examples/skeleton_clock.rs`

Apply the button naming rule:

- Keep the Armatron `ButtonRp` local named `button`.
- Rename Ballet's `button_watch: ButtonRp` to `button`.
- Rename Clock and Skeleton Clock's `button_watch15: ButtonRp` to `button`.

### WASM callers

Update the Armatron, Ballet, Clock, and Skeleton Clock WASM examples to call
their module-qualified `run` and `splash` operations.

Keep capability button locals named `button`. In Ballet, make the binding and
reference immutable if the concrete API does not require mutability.

### ESP templates

Update these Jinja templates first:

- `crates/linkage-blaze-examples-esp/examples/templates/armatron.rs.j2`
- `crates/linkage-blaze-examples-esp/examples/templates/armatron_one_spi.rs.j2`
- `crates/linkage-blaze-examples-esp/examples/templates/ballet.rs.j2`
- `crates/linkage-blaze-examples-esp/examples/templates/clock.rs.j2`
- `crates/linkage-blaze-examples-esp/examples/templates/skeleton_clock.rs.j2`

Call `module::run` and `module::splash`. Keep concrete `ButtonWatch` locals
named `button_watch`.

Do not edit generated board examples. Use the repository's template-generation
checks to verify the templates without including generated board-file changes.

## Device Envoy work

### DNS Tester core and RP callers

Keep the DNS Tester core API unchanged. It is the model for the desired module
API and already uses descriptive generic and error parameter names.

Keep the two RP DNS Tester examples' concrete `ButtonRp` locals named `button`:

- `../mcu/device-envoy/crates/device-envoy-examples-rp/examples/dns_tester.rs`
- `../mcu/device-envoy/crates/device-envoy-examples-rp/examples/dns_tester_one_spi.rs`

Continue to call `dns_tester::splash`, `dns_tester::wifi_status`, and
`dns_tester::run` through the module.

### DNS Tester ESP template

In
`../mcu/device-envoy/crates/device-envoy-examples-esp/examples/templates/dns_tester.rs.j2`, rename
the concrete `ButtonWatch` local from `button` to `button_watch` and update all
references passed to CYD construction, Wi-Fi setup, and `dns_tester::run`.

Do not edit generated Device Envoy board examples. Use Device Envoy's template
checks to verify the source template without including generated board-file
changes.

No Device Envoy core API rename is part of this work.

## TODO handling

Preserve existing TODO comments. When an implementation makes one obsolete,
append `(may no longer apply)` rather than deleting it.

## Suggested implementation sequence

1. Make the Linkage Blaze core organization and documentation changes.
2. Update Linkage Blaze RP and WASM callers.
3. Update Linkage Blaze ESP templates and run their template checks.
4. In a separately authorized Device Envoy change, update the DNS Tester ESP
   template and run its template checks.
5. Search both repositories for direct `run`/`splash` imports, stale entry-point
   names, and button locals that do not match their concrete type.
6. Format and run both repositories' local CI commands.

## Verification

In Linkage Blaze:

```text
just check-all
```

Also verify with focused searches that:

- Platform callers use `armatron::run`, `ballet::run`, `clock::run`, and
  `skeleton_clock::run`.
- Clock callers use `clock::splash` and Skeleton Clock callers use
  `skeleton_clock::splash`.
- RP `ButtonRp` locals are named `button` in the reviewed examples.
- ESP `ButtonWatch` locals are named `button_watch`.
- No compatibility aliases or old example-prefixed entry points remain.

If Device Envoy is changed, run its documented local CI command:

```text
cargo check-all
```

Verify that DNS Tester still uses module-qualified calls, its RP `ButtonRp`
locals remain `button`, and its ESP `ButtonWatch` local is `button_watch`.

## Completion criteria

- All five example APIs retain concise module-scoped operation and type names.
- Linkage Blaze callers consistently use module-qualified entry points.
- Button locals match their concrete type across Linkage Blaze and DNS Tester.
- Error types appear with their public entry points before private helpers.
- Templates are the only edited ESP source of truth; generated board files
  remain untouched.
- Linkage Blaze and, when changed, Device Envoy local CI pass.

Suggested commit messages:

```text
Standardize Linkage Blaze example entry-point calls
```

```text
Align DNS Tester ButtonWatch naming
```
