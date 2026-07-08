<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# `linkage-blaze-rp` Spec

## Goal

Add a new `crates/linkage-blaze-rp` subcrate that runs the existing linkage-blaze embedded
examples on Raspberry Pi Pico targets using the same configuration style already used by
`device-envoy-rp`.

Initial example set:

- `armatron`
- `ballet`
- `clock`
- `skeleton_clock`

Initial target configurations:

- Pico 1: `pico1,arm`
- Pico 2: `pico2,arm`
- Pico 1 W: `pico1,arm,wifi`
- Pico 2 W: `pico2,arm,wifi`

The crate should reuse the shared app logic already in `crates/linkage-blaze-example-core`.

## Design Choice

Do **not** start by mirroring the ESP generated-example system.

For RP, the matrix is currently small and `device-envoy-rp` itself does not use a centralized
board-profile table. It selects configuration through:

- Cargo features
- target triples
- per-example hardcoded wiring

`linkage-blaze-rp` should follow that same model first.

## Why This Simpler Approach

This is the lower-risk first step.

- `device-envoy-rp` already demonstrates the exact startup and wiring style to copy.
- The RP target space is only four configurations, not a large chip/board matrix like ESP.
- The shared logic is already isolated in `linkage-blaze-example-core`, so the remaining work is
  platform glue.
- A hand-written RP crate can be validated faster than building an RP-specific generator first.

If RP support later grows to many board-specific wiring variants, the crate can adopt generated
examples then.

This first pass also deliberately sticks to `device-envoy-rp`'s existing two-SPI CYD model:

- display on `SPI0`
- touch on `SPI1`

That is the simplest path because it matches the current RP API surface. The main downside is that
it occupies both hardware SPI peripherals, leaving no dedicated SPI peripheral free for user-added
devices. A later phase may add a one-SPI RP CYD path to free one of those peripherals.

## Non-Goals

- Do not add an RP `boards.rs` database in phase 1.
- Do not add RP template generation in phase 1.
- Do not add one-SPI RP CYD support in phase 1.
- Do not merge RP and ESP into one crate.
- Do not fork the generic app logic from `linkage-blaze-example-core`.
- Do not publish anything during this work.

## Current State

`linkage-blaze` already has the right layering for this:

- `crates/linkage-blaze-example-core` contains shared example logic.
- `crates/linkage-blaze-esp` contains ESP-only platform entrypoints.

`device-envoy-rp` already exposes the required primitives:

- `CydRp`, `CydDisplayRp`, and touch calibration support
- `WifiAutoRp` and `ClockSyncRp`
- `ButtonRp` / `button_watch!`
- RP examples showing the expected wiring and startup style

Relevant RP examples to mirror:

- CYD display + touch: `crates/device-envoy-rp/examples/cyd_touch_paint.rs`
- WiFi clocks: `crates/device-envoy-rp/examples/clock_console.rs`,
  `clock_led12x4.rs`, `clock_led8x12.rs`, `clock_servos.rs`

## Crate Layout

Add:

- `crates/linkage-blaze-rp`

Update workspace membership in the root `Cargo.toml`.

Phase 1 should use ordinary handwritten `examples/*.rs` files, following the current
`device-envoy-rp` convention.

Suggested initial example file set:

- `examples/armatron.rs`
- `examples/ballet.rs`
- `examples/clock.rs`
- `examples/skeleton_clock.rs`

The crate may also contain a minimal `src/lib.rs` if a library target is useful for tests or
shared constants, but the examples are the primary deliverable.

## Configuration Model

Follow `device-envoy-rp` rather than `linkage-blaze-esp`.

Board/configuration selection should happen through:

- target triple
- Cargo features
- optional helper commands in a crate-local `justfile`

No generated board-specific filenames are needed in phase 1.

Instead, one example file should compile for multiple RP configurations, with support gated by
features where needed.

Examples:

- `armatron.rs` should build for `pico1`, `pico2`, `pico1,wifi`, and `pico2,wifi`
- `clock.rs` should build only when `wifi` is enabled

## Cargo Features

`crates/linkage-blaze-rp/Cargo.toml` should define and forward:

- board features: `pico1`, `pico2`, `arm`
- optional connectivity feature: `wifi`
- app features: `armatron`, `ballet`, `clock`, `skeleton-clock`

The crate should forward board and platform features into `device-envoy-rp`.

Expected use:

- non-W Pico 1: `--features pico1,arm,armatron --no-default-features`
- non-W Pico 2: `--features pico2,arm,ballet --no-default-features`
- Pico 1 W: `--features pico1,arm,wifi,clock --no-default-features`
- Pico 2 W: `--features pico2,arm,wifi,skeleton-clock --no-default-features`

## Wiring Model

Phase 1 should hardcode wiring directly in each RP example, exactly like `device-envoy-rp`
examples do today.

Do not invent a separate shared RP board metadata layer yet.

Phase 1 should also keep the current RP CYD bus split:

- display on `SPI0`
- touch on `SPI1`

This is intentionally conservative. It matches `device-envoy-rp` as it exists today and avoids
adding RP shared-bus work during the initial port.

Use the existing RP CYD example wiring as the default source:

- Display SPI0:
  - `PIN_18` SCK
  - `PIN_19` MOSI
  - `PIN_16` MISO
  - `PIN_17` CS
  - `PIN_20` DC
  - `PIN_21` RST
  - `PIN_22` backlight
- Touch SPI1:
  - `PIN_10` SCK
  - `PIN_11` MOSI
  - `PIN_12` MISO
  - `PIN_13` CS
  - `PIN_14` IRQ
- Button:
  - `PIN_15`

For WiFi examples, use the same CYW43 wiring that `WifiAutoRp` examples already use:

- `PIN_23`
- `PIN_24`
- `PIN_25`
- `PIN_29`

## Example Strategy

### `armatron`

Create an RP entrypoint that:

- initializes Embassy RP peripherals
- creates `FlashBlockRp`
- creates a `ButtonRp`
- creates `CydRp`
- runs `linkage_blaze_example_core::armatron::armatron`

This should target all four RP configurations.

### `ballet`

Create an RP entrypoint that:

- initializes Embassy RP peripherals
- creates `CydDisplayRp`
- runs the shared `linkage_blaze_example_core::ballet` logic

This should also target all four RP configurations.

### `clock`

Create an RP entrypoint that:

- is gated with `#![cfg(feature = "wifi")]`
- initializes `CydDisplayRp`
- sets up `FlashBlockRp`
- sets up `WifiAutoRp`
- sets up `ClockSyncRp`
- runs `clock_splash()` and then `clock()`

This should target only the W configurations.

### `skeleton_clock`

Create an RP entrypoint that:

- is gated with `#![cfg(feature = "wifi")]`
- initializes `CydDisplayRp`
- sets up `FlashBlockRp`
- sets up `WifiAutoRp`
- sets up `ClockSyncRp`
- runs the shared skeleton clock splash and main loop

This should target only the W configurations.

## Unsupported Configurations

Phase 1 does **not** need ESP-style placeholder example generation.

Unsupported RP configurations should simply be handled by feature gating:

- `clock` and `skeleton_clock` exist as examples, but require `wifi`
- attempting to build them without `wifi` should fail in the normal Cargo way or be excluded by
  check commands

That matches how `device-envoy-rp` already works.

## `justfile` And Workflow

Add a `crates/linkage-blaze-rp/justfile` mirroring `device-envoy-rp`'s ergonomics:

- `run name board="1"`
- `check name board="1"`
- `build name board="1"`

Supported board suffixes should match `device-envoy-rp`:

- `1`
- `2`
- `w`
- `2w`

The wrapper can map those short forms to:

- target triple
- RP board features
- whether `wifi` is enabled

This is intentionally simpler than the ESP generated example names.

## Root `xtask` / CI

Phase 1 does not need an RP example generator, but local CI still needs to build the RP example
surface.

Update `linkage-blaze` workflow so it can build representative RP examples, likely through a new
xtask command or direct cargo invocations.

`just check-all` must build the full valid RP matrix. Do not silently skip any supported RP
configuration.

Required `linkage-blaze-rp` build matrix:

- `armatron`: `1`, `2`, `w`, `2w`
- `ballet`: `1`, `2`, `w`, `2w`
- `clock`: `w`, `2w`
- `skeleton_clock`: `w`, `2w`

This is a hard requirement, not an optional stretch goal.

## `device-envoy-rp` Validation

This work depends on the RP CYD stack in the sibling `device-envoy` repo, so the validation
requirement is not limited to `linkage-blaze-rp` builds alone.

As part of this effort, the `device-envoy-rp` side must also keep full CYD RP support validated.

At minimum, the relevant `device-envoy-rp` checks must continue to build and remain part of normal
validation:

- `examples/cyd_touch_paint.rs`
- any RP CYD display-only constructor paths exercised by the linkage-blaze ports
- any RP WiFi + CYD combinations needed by the linkage-blaze clock examples

`just check-all` should therefore be understood as covering both:

- the full valid `linkage-blaze-rp` example matrix
- the corresponding `device-envoy-rp` CYD support surface those examples rely on

If new RP CYD helpers or constructors are added in `device-envoy-rp` for this work, they must be
built in normal checks rather than only ad hoc during development.

## Implementation Sequence

### Phase 1: New Crate Scaffold

- add `crates/linkage-blaze-rp`
- add workspace membership
- add `Cargo.toml`
- add feature forwarding to `device-envoy-rp`
- add a crate-local `justfile`

### Phase 2: Port Non-WiFi Examples

Port in this order:

1. `ballet`
2. `armatron`

This proves the RP CYD display and CYD touch/calibration paths first.

### Phase 3: Port WiFi Examples

Port in this order:

1. `clock`
2. `skeleton_clock`

This proves the RP WiFi + clock sync path after the display path is already working.

### Phase 4: CI Wiring

- add RP build coverage to local checks
- make `just check-all` build the full valid RP configuration matrix
- make `just check-all` keep the required `device-envoy-rp` CYD support surface under test
- ensure missing embedded targets fail loudly
- document the supported build invocations

### Phase 5: Revisit Abstractions

Only after phase 1 is stable, evaluate whether RP now has enough repetition to justify:

- a shared RP wiring module
- a board-profile table
- a generated example pipeline

Those are follow-up refactors, not entry requirements.

### Phase 6: Optional One-SPI CYD Support

If freeing one hardware SPI peripheral becomes important, add a second RP CYD path that shares one
SPI bus between display and touch.

Motivation:

- keep one hardware SPI peripheral available for user-added devices such as RFID, external flash,
  sensors, or other displays

Constraints:

- this should be treated as a follow-up to the working two-SPI port, not a prerequisite for
  `linkage-blaze-rp`
- it likely requires new `device-envoy-rp` support first, analogous in spirit to the ESP one-SPI
  CYD work

Deliverable shape:

- either a new RP one-SPI CYD abstraction or a generalized RP CYD constructor that can share a
  bus safely
- `linkage-blaze-rp` examples can then decide whether to stay on two-SPI by default or migrate to
  one-SPI where appropriate

## Open Questions

- Does `linkage_blaze_example_core::ballet` already compile cleanly against `CydDisplayRp`, or
  does it still assume some ESP-specific behavior?
- Do `clock` and `skeleton_clock` fit comfortably within RP RAM/flash budgets on both W boards,
  or will one of them need a later RP-specific resource gate?
- Should the root `justfile` expose RP convenience commands immediately, or should the first pass
  keep them crate-local?
- After phase 1 lands, should the default RP CYD path remain two-SPI for simplicity, or should a
  later one-SPI path become the preferred default to leave one hardware SPI peripheral free?

## Success Criteria

This spec is complete when:

- `crates/linkage-blaze-rp` exists
- `just check-all` builds the full valid `linkage-blaze-rp` RP matrix
- `armatron` builds on Pico 1, Pico 2, Pico 1 W, and Pico 2 W
- `ballet` builds on Pico 1, Pico 2, Pico 1 W, and Pico 2 W
- `clock` builds on Pico 1 W and Pico 2 W
- `skeleton_clock` builds on Pico 1 W and Pico 2 W
- the required `device-envoy-rp` CYD support surface is also part of normal validation
- the crate follows current `device-envoy-rp` configuration conventions rather than introducing a
  new RP board-profile system
