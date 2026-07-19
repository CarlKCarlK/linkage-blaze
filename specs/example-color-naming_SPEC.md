<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Example color naming cleanup

## Objective

Make color values identifiable from their names wherever the surrounding module
does not already make the value's type and role unambiguous. In particular,
replace the public example constants `BACKGROUND` and `FOREGROUND` with:

```rust,no_run
pub const BACKGROUND_COLOR: Rgb888 = /* ... */;
pub const FOREGROUND_COLOR: Rgb888 = /* ... */;
```

The suffix matters because `BACKGROUND` and `FOREGROUND` can describe images,
layers, objects, or other drawing resources. These constants are imported
unqualified by platform examples, so their names should state that they are
colors.

Use the full word `BACKGROUND`, not `BACK`. `BACK_COLOR` could mean the color
of a rear face or reverse side, while `BACKGROUND_COLOR` is the conventional
and unambiguous counterpart to `FOREGROUND_COLOR`.

## Scope

### Canonical core example constants

Rename the public `Rgb888` constant pair in each of these modules:

- `crates/linkage-blaze-core/src/examples/armatron/main.rs`
- `crates/linkage-blaze-core/src/examples/ballet.rs`
- `crates/linkage-blaze-core/src/examples/clock.rs`
- `crates/linkage-blaze-core/src/examples/skeleton_clock.rs`

Apply these exact renames:

- `BACKGROUND` to `BACKGROUND_COLOR`
- `FOREGROUND` to `FOREGROUND_COLOR`

Update all uses within the core modules, including palette construction,
conversions to `Rgb565`, tests, and `CydMemory` construction.

### Related ambiguous color names

Complete the same cleanup for the nearby private color values whose current
names could describe non-color objects:

- `FIGURE` to `FIGURE_COLOR` in `skeleton_clock.rs`.
- `PLACARD_TEXT` to `PLACARD_TEXT_COLOR` in `skeleton_clock.rs`.
- The local `background: Rgb565` in `clock.rs` to `background565`, preserving
  the repository convention that a native-format conversion is visible in the
  local name.

Keep already-clear names such as `TIME_COLOR` and `EXCEL_BLUE` unchanged.
Names that are themselves conventional color names, such as `BLACK`, `WHITE`,
`RED`, `GREEN`, and `BLUE`, do not need a redundant `_COLOR` suffix.

### Platform examples and templates

Update every RP and WASM example that imports or passes the renamed public
constants.

For ESP examples, update all applicable files under
`crates/linkage-blaze-examples-esp/examples/templates/` first, including the
Armatron, one-SPI Armatron, ballet, clock, and skeleton-clock templates. Then
run `just generate-board-examples` to update the generated board examples.
Do not edit generated ESP examples as the sole source of a change.

Remove the obsolete comment:

```rust,no_run
//todo000 should rename with _COLOR
```

from the one-SPI Armatron template. Regeneration should remove it from all
generated copies.

## Out of scope

Do not rename values that identify a non-color background resource. In
particular, retain names such as:

- `BACKGROUND_BITMAP`
- `BACKGROUND_BITMAP_VIEW`
- `CLOCK_BACKGROUND_VIEW`
- `CLOCK_BACKGROUND_BITMAP`

Do not rename prose, CSS `background` properties embedded in Rust strings, or
functions whose subject is drawing or presenting a complete background rather
than storing a color.

Do not add compatibility aliases for `BACKGROUND` or `FOREGROUND`. Update all
in-tree callers together so the API looks as if it had originally been
designed with the final names.

## Implementation checklist

- [x] Confirm all definitions and uses with searches limited to `.rs` and
      `.j2` files.
- [x] Rename the four canonical `BACKGROUND`/`FOREGROUND` constant pairs.
- [x] Rename `FIGURE`, `PLACARD_TEXT`, and the local `background` color.
- [x] Update core tests and internal call sites.
- [x] Update RP and WASM imports and call sites.
- [x] Update every affected ESP template.
- [x] Remove the obsolete `_COLOR` naming TODO.
- [x] Run `just generate-board-examples` and review the generated changes.
- [x] Search for stale bare color names without matching bitmap and prose
      identifiers accidentally.
- [x] Inspect the final diff and preserve unrelated user changes.

## Verification

Run these targeted searches after the rename:

```text
rg -n '\b(BACKGROUND|FOREGROUND)\b' --glob '*.rs' --glob '*.j2'
rg -n '\b(FIGURE|PLACARD_TEXT)\b' --glob '*.rs' --glob '*.j2' crates/linkage-blaze-core/src/examples
rg -n 'rename with _COLOR' --glob '*.rs' --glob '*.j2'
```

Review any remaining `BACKGROUND` or `FOREGROUND` match individually. A match
is acceptable only when it refers to a non-color resource or prose, not an
`Rgb888`/`Rgb565` value.

Then:

1. Run formatting for the affected Rust code.
2. Run the relevant core example tests.
3. Confirm `just generate-board-examples` leaves the generated examples clean
   on a second run.
4. Run `just check-all`.

## Completion criteria

The four core examples export `BACKGROUND_COLOR` and `FOREGROUND_COLOR`, all
platform examples and tests use those names, related ambiguous private color
values have explicit color names, generated ESP examples match their templates,
the obsolete TODO is gone, non-color background identifiers remain intact, and
the complete local CI suite passes.

Suggested commit message:

```text
Clarify example color constant names
```
