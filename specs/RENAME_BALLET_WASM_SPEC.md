# Rename `linkage-blaze-classic-wasm` to `linkage-blaze-ballet-wasm`

<!-- todo0 consider deleting this spec once the rename is implemented and released. -->

The `ballet` demo's browser crate is named `linkage-blaze-classic-wasm`, so the demo slug
(`ballet`) and the crate name disagree everywhere the two meet (`pages/demos.tsv`, the
justfile, the gallery build). Rename the wasm crate so the mapping is uniform.

**Run this only after the demo-page UX work (`specs/DEMO_PAGE_UX_SPEC.md`) has landed**,
so the two diffs stay independently reviewable.

## Scope — what is and is not renamed

- Rename: crate directory `crates/linkage-blaze-classic-wasm` →
  `crates/linkage-blaze-ballet-wasm`, package name `linkage-blaze-classic-wasm` →
  `linkage-blaze-ballet-wasm`.
- Do **not** rename `crates/linkage-blaze-classic`. There, "classic" means the classic
  ESP32 chip (as opposed to `linkage-blaze-armatron-c6`'s ESP32-C6); it hosts all four
  embedded examples (`armatron`, `clock`, `skeleton-clock`, `ballet`) as `--example`
  binaries and is correctly named.
- Do not edit the frozen snapshots under `pages/demos/ballet/v*`.

## The wasm artifact-name wrinkle (read before starting)

`xtask build-pages` (`build_demo` in `xtask/src/main.rs`) rebuilds the `pkg/` directory
for **every** frozen version of a demo using the single `out_name` column from
`pages/demos.tsv`. The frozen snapshots' `app.js` files hardcode
`import ... from "./pkg/linkage_blaze_classic_wasm.js"`. So if `out_name` simply changes
to `linkage_blaze_ballet_wasm`, every existing frozen ballet version breaks.

Recommended fix — make the artifact name per-version by deriving it from each snapshot:

- In `build_demo`, for each frozen version, parse the snapshot's `app.js` for the pkg
  import (`from "./pkg/<name>.js"`) and pass that `<name>` as `--out-name` for that
  version's `wasm-pack build`. Fail with a clear error if the import cannot be found.
- The `out_name` column in `demos.tsv` then applies only to the live `www/` sources and
  to future bumps; change it to `linkage_blaze_ballet_wasm` for the ballet row.
- Add a unit test beside the existing `DemoRecord` tests covering the import-parsing
  helper (old name, new name, and a missing-import error case).

This keeps old snapshots immutable and working while new versions get the uniform name.
If you find `build_demo` has changed and no longer works this way, stop and re-plan
rather than forcing this design.

## Reference inventory (verify with a fresh `grep -rn classic` — this list may age)

- `Cargo.toml` (workspace members list).
- `crates/linkage-blaze-classic-wasm/` — directory name, `Cargo.toml` package name and
  doc comment, any `src/` doc references to the crate name.
- `crates/linkage-blaze-classic-wasm/www/app.js` — pkg import becomes
  `./pkg/linkage_blaze_ballet_wasm.js`.
- `pages/demos.tsv` ballet row — crate dir, www dir, and `out_name` columns.
- `justfile` — `_ballet_wasm_crate`, `_ballet_wasm_www`, the `wasm-pack build` lines and
  `cargo check -p linkage-blaze-classic-wasm`, and any recipe comments naming the crate.
  Leave every `*-classic` recipe (`run-ballet-classic`, `check-armatron-classic`, …)
  untouched — those target the embedded chip crate.
- CI workflows and docs — grep `.github/`, `pages/README.md`, and crate-level docs for
  the old name.
- Do not remove or reword existing `TODO` comments encountered along the way.

## Verification

1. `grep -rn "classic-wasm\|classic_wasm" --exclude-dir=target --exclude-dir=node_modules --exclude-dir=pages .`
   returns no hits outside this spec's history (frozen `pages/demos/ballet/v*` snapshots
   keep the old artifact name by design).
2. `just check-all` passes.
3. `just run-all-wasm`: the ballet card previews and opens; **every** frozen ballet
   version in the version selector still loads and animates (this exercises the
   per-version out-name fix).
4. `just bump-demo-version ballet`, rebuild, and confirm the new version's `pkg/` uses
   `linkage_blaze_ballet_wasm.js`. Revert the bump afterward unless a release is intended
   (`git status` should show only the rename changes).
