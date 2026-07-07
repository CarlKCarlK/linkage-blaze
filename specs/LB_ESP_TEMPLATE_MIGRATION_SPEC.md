<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# `linkage-blaze-esp` Template Migration Spec

## Goal

Replace the current single-board `linkage-blaze-classic` embedded example crate with a renamed `linkage-blaze-esp` crate that follows the `device-envoy-esp` generated-example pattern:

- reuse `device-envoy-esp`'s board-profile source of truth
- generate board-specific example files from templates
- generate `Cargo.toml` `[[example]]` entries from the same template inventory
- emit unsupported-board placeholders instead of hand-maintained exclusions

The target is not "make every example actually run on every board". The target is "generate a complete, explicit board/chip example matrix from shared board metadata, with supported examples building normally and unsupported ones rendered as placeholders".

## Non-Goals

- Do not introduce a second independently maintained board-profile database in `linkage-blaze`.
- Do not keep the old hand-written `linkage-blaze-classic/examples/*.rs` layout long term.
- Do not require every device-envoy board to support all four linkage-blaze examples.
- Do not publish or release during this work.

## Current State

Today `crates/linkage-blaze-classic` contains four hand-written examples:

- `armatron`
- `ballet`
- `clock`
- `skeleton-clock`

Each one hard-codes CYD-oriented peripherals and pin mappings directly in the example source:

- display SPI peripheral and GPIO wiring
- touch SPI peripheral and GPIO wiring
- calibration button pin
- Wi-Fi assumptions

This makes the crate effectively a single-board crate even though it already depends on `device-envoy-esp`.

## Target Architecture

### Crate Rename

Rename:

- `crates/linkage-blaze-classic` -> `crates/linkage-blaze-esp`
- workspace membership and all cargo package references accordingly
- `justfile` task names from `*-classic` to `*-esp`

The renamed crate remains the embedded application crate for linkage-blaze examples.

### Template-Driven Example Generation

`linkage-blaze-esp` should mirror the `device-envoy-esp` pattern:

- `examples/templates/*.rs.j2` contains the canonical example sources
- generated outputs live under `examples/<chip>/<board>/<example>.rs`
- `Cargo.toml` contains a generated `[[example]]` block delimited by markers
- an `xtask` command regenerates files and manifest entries

Initial template set:

- `examples/templates/armatron.rs.j2`
- `examples/templates/ballet.rs.j2`
- `examples/templates/clock.rs.j2`
- `examples/templates/skeleton_clock.rs.j2`

Generated example names should include chip feature and board directory, following the same shape as `device-envoy-esp`, for example:

- `ballet_esp32_generic`
- `clock_esp32s3_devkitc1_v1_0_n16r8`

### Shared Board-Profile Source of Truth

`linkage-blaze-esp` should reuse `device-envoy-esp` board metadata rather than duplicating it manually.

Preferred approach:

- factor the board-profile definitions into a shared Rust module/crate that both projects consume

Acceptable first step if factoring is too disruptive:

- `linkage-blaze` xtask reads the board-profile source directly from the local sister `device-envoy-esp` workspace

Rejected approach:

- copying the board table into `linkage-blaze` as a permanent second source of truth

The implementation should preserve one authoritative place for:

- chip IDs and feature names
- board IDs and directory names
- CYD display wiring
- CYD touch wiring
- button pins
- chip capabilities such as Wi-Fi support, SPI count, and stack constraints

## Template Context

The generated linkage-blaze templates should consume a context very close to the one already used by `device-envoy-esp` CYD templates.

Required context keys:

- `example_name`
- `chip_name`
- `chip_feature`
- `board_slug`
- `button_pin`
- `clock_supported`
- `cyd_display_sck_pin`
- `cyd_display_mosi_pin`
- `cyd_display_miso_pin`
- `cyd_display_cs_pin`
- `cyd_display_dc_pin`
- `cyd_display_rst_pin`
- `cyd_display_backlight_pin`
- `cyd_touch_sck_pin`
- `cyd_touch_mosi_pin`
- `cyd_touch_miso_pin`
- `cyd_touch_cs_pin`
- `cyd_touch_irq_pin`

Optional additional keys may be added if needed to avoid template conditionals leaking chip-specific Rust details into the example body.

## Example Support Rules

Each template must declare explicit support requirements, similar to `device-envoy-esp`'s `@board-example` requirements.

### `ballet`

Requirements:

- dual SPI

Reason:

- uses CYD display, but not Wi-Fi or touch calibration flow

### `armatron`

Requirements:

- dual SPI
- large stack if needed by final binary size

Reason:

- uses display plus touch plus flash-backed calibration and a button

Note:

The first implementation should assume `armatron` requires CYD touch wiring and a button pin. If stack usage proves problematic on some chips, add `large_stack` explicitly rather than relying on accidental linker failures.

### `clock`

Requirements:

- dual SPI
- Wi-Fi
- large stack

Reason:

- uses CYD display plus `WifiAutoEsp` plus `ClockSyncEsp`

### `skeleton_clock`

Requirements:

- dual SPI
- Wi-Fi
- large stack

Reason:

- same device requirements as `clock`

## Unsupported Boards

Unsupported board/chip/example combinations should still generate a `.rs` file and manifest entry, but the generated file should be a placeholder that states why the example is unsupported on that board profile.

This behavior is required because it keeps:

- template inventory complete
- Cargo manifest generation deterministic
- support boundaries explicit

It also avoids hand-maintained allowlists spread across manifests and scripts.

## Generator Responsibilities

Add generator support in this repository's `xtask` so it can:

1. discover example templates
2. load shared board profiles
3. evaluate per-template requirements
4. render supported examples
5. render placeholder examples for unsupported profiles
6. write generated example files under nested chip/board directories
7. rewrite the generated `[[example]]` block in `crates/linkage-blaze-esp/Cargo.toml`
8. clean up stale generated files
9. run `rustfmt` over generated outputs

The generator should follow `device-envoy-esp` closely enough that future maintenance is obvious and diffable.

## `justfile` and Developer Workflow

The root [justfile](/home/carlk/programs/linkage-blaze/justfile) should migrate from direct single-example commands to a small wrapper strategy closer to `device-envoy-esp`.

Desired end state:

- renamed `*-esp` tasks instead of `*-classic`
- support for building and running generated board examples by name
- `check-all` validates the generated embedded example surface, not just four old top-level files

It is acceptable to keep a few convenience aliases for common examples during migration, but the canonical path should use generated examples.

## Implementation Sequence

### Phase 1: Rename and Scaffolding

- rename crate directory and package name to `linkage-blaze-esp`
- update workspace membership
- update `justfile` names and comments
- add a generated-example markers block to the new crate `Cargo.toml`

### Phase 2: Generator Skeleton

- add xtask module(s) for template discovery and manifest rewriting
- wire in a `generate-board-examples` style command
- initially target one simple template to validate the flow

### Phase 3: Shared Board Metadata Reuse

- hook the generator to `device-envoy-esp` board metadata
- avoid permanent board table duplication
- document the dependency on the local sister workspace if that is the temporary integration path

### Phase 4: Template Migration

Migrate examples in this order:

1. `ballet`
2. `clock`
3. `skeleton_clock`
4. `armatron`

Rationale:

- `ballet` is the smallest CYD display example
- `clock` and `skeleton_clock` validate Wi-Fi gating
- `armatron` is the most complex due to touch and calibration flow

### Phase 5: CI Migration

- update `check-all` to regenerate examples before checking
- validate selected supported generated examples across representative chips
- remove the old hand-written example files once parity is established

## Risks

### Shared Metadata Coupling

If `linkage-blaze` reads `device-envoy-esp` board metadata directly, changes in that repo can break generation here. This is acceptable only if the coupling is intentional and documented.

### False Capability Assumptions

Some board profiles may compile but not be practically usable for a CYD-style wiring setup. The generator should prefer explicit requirements and placeholders over optimistic compilation attempts.

### Naming Churn

Renaming `skeleton-clock` to `skeleton_clock` inside generated file paths versus cargo example names needs to be handled deliberately. Keep a documented rule for:

- template filename
- generated Rust filename
- cargo example name

### Tooling Drift

If the template/generator pattern diverges too far from `device-envoy-esp`, future maintenance will become confusing. The implementation should intentionally preserve the same mental model and similar helper structure.

## Open Questions

- Should the shared board-profile source become a real shared crate/module immediately, or is direct xtask consumption from the sister workspace acceptable for the first slice?
- Should `linkage-blaze-esp` have its own `scripts/device-action.sh` equivalent, or should root `justfile` commands stay as the main interface?
- Do we want generated placeholder examples to fail at compile time with a clear message, or compile and panic immediately if run?
- Should generated example names preserve `skeleton-clock` spelling for CLI familiarity, or standardize to `skeleton_clock` for path symmetry?

## Acceptance Criteria

This work is complete when:

- `linkage-blaze-classic` is replaced by `linkage-blaze-esp`
- all four embedded examples originate from templates
- generated outputs are emitted per chip and board
- board metadata comes from the `device-envoy-esp` source of truth rather than a permanent duplicate table
- unsupported combinations are represented by generated placeholders
- the crate `Cargo.toml` `[[example]]` entries are generated
- `just check-all` validates the new generated example flow
