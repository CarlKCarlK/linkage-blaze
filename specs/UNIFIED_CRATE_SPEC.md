# Unified `linkage-blaze` Crate

<!-- TODO0 consider deleting this spec once the unified crate is implemented and released. -->

## Status

Implemented for the next Linkage Blaze release and verified locally. Keep this
spec through the 0.1.5 release, then consider deleting it after the clean-tree
publication dry run and release are complete. Do not publish
`linkage-blaze-core` or `linkage-blaze-utils` version 0.1.5 packages.

## Summary

Linkage Blaze should present one published Cargo package named
`linkage-blaze`. The package should cover:

- allocation-free embedded use on ESP and RP targets;
- optional allocation-backed parsing and owned linkage APIs;
- host-side BVH parsing and conversion;
- the `bvh-to-lb` command-line program; and
- use as the platform-neutral dependency of WASM applications.

Hardware examples, board adapters, gallery applications, and JavaScript glue
may remain separate workspace packages, but they must be marked
`publish = false`. These are build organization, not additional products that
users must discover or version.

## Goals

- Give users one crate name, one version, one crates.io page, and one primary
  docs.rs page.
- Preserve allocation-free embedded builds.
- Make richer capabilities explicit Cargo features on the same crate.
- Ship `bvh-to-lb` from the same package.
- Keep platform and browser adapters out of the published package list.
- Replace the current public `core`/`utils` distinction with capability-based
  documentation.
- Keep `cargo check-all` as the complete local release gate.

## Non-goals

- Do not combine every workspace package into one Cargo package.
- Do not publish RP, ESP, WASM, gallery, or xtask packages.
- Do not make Device Envoy part of Linkage Blaze; it remains the optional
  hardware/display integration used by the examples.
- Do not retain compatibility aliases for the old Rust crate names. There are
  no known users, so update the workspace directly to the intended API.
- Do not require the public library itself to be a WASM `cdylib`. A WASM
  application is the final `cdylib` and depends on `linkage-blaze` as an
  ordinary Rust library.

## User Experience

### ESP

```toml
[dependencies]
linkage-blaze = "0.1.5"
device-envoy-esp = "0.1.3"
```

```rust,no_run
use linkage_blaze::{LinkageFixed, linkage_fixed};
```

The default Linkage Blaze build must not require allocation or `std`.
Device Envoy supplies the ESP-specific display and hardware integration.

### RP

```toml
[dependencies]
linkage-blaze = "0.1.5"
device-envoy-rp = "0.1.3"
```

```rust,no_run
use linkage_blaze::{LinkageFixed, linkage_fixed};
```

The same library API must compile for supported RP2040 and RP235x targets.
There is no separately published Linkage Blaze RP crate.

### Allocation-backed Rust

```toml
[dependencies]
linkage-blaze = { version = "0.1.5", features = ["alloc"] }
```

This enables runtime-owned linkage storage and `.lb.rs` parsing while retaining
the ability to build without the Rust standard library.

### WASM

A browser application supplies its own `cdylib` and JavaScript-facing API:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
linkage-blaze = { version = "0.1.5", features = ["alloc"] }
wasm-bindgen = "0.2"
```

The gallery's existing editor bindings become a thin, unpublished workspace
adapter. Generic Linkage Blaze parsing, evaluation, and rendering remain in the
published crate. External WASM applications can create their own adapter in the
same way.

### BVH Rust API

```toml
[dependencies]
linkage-blaze = { version = "0.1.5", features = ["bvh"] }
```

```rust,no_run
use linkage_blaze::bvh::{bvh_to_lb_rs, parse_bvh};
```

The initial `bvh` feature may require `std` because the existing full BVH
implementation uses synchronization and standard-library collections. This
must be stated explicitly. Moving full BVH support from `std` to `alloc` alone
is desirable but is not required merely to consolidate the packages.

### BVH Command-Line Program

```bash
cargo install linkage-blaze --features bvh --bin bvh-to-lb
bvh-to-lb input.bvh output.lb.rs
```

The binary is shipped by the `linkage-blaze` package and uses its public BVH
API. It may use `std` independently of allocation-free embedded consumers.

## Cargo Package Design

The single published package should live at `crates/linkage-blaze` and use the
package and Rust crate name `linkage-blaze` / `linkage_blaze`.

```toml
[package]
name = "linkage-blaze"
version.workspace = true
publish = true

[features]
default = []
alloc = []
std = ["alloc"]
bvh = ["std"]

[[bin]]
name = "bvh-to-lb"
path = "src/bin/bvh-to-lb.rs"
required-features = ["bvh"]
```

The exact dependency feature wiring may add entries to these feature lists,
but the user-visible meanings must remain:

| Feature | Meaning |
| --- | --- |
| default | Allocation-free library suitable for embedded targets |
| `alloc` | Owned/runtime linkage and parsing APIs without requiring `std` |
| `std` | Host standard-library support; implies `alloc` |
| `bvh` | Full BVH parsing and conversion; initially implies `std` |

The public library should use conditional `no_std` configuration equivalent to:

```rust,no_run
#![cfg_attr(not(feature = "std"), no_std)]
```

This makes the guarantee precise: default and `alloc`-only builds do not use
`std`; host capabilities may opt into it explicitly.

The package should configure docs.rs to document the useful feature-complete
API, while CI must separately verify the default and `alloc`-only builds.

## Workspace Layout

The intended workspace organization is:

```text
crates/
├── linkage-blaze/                  published
├── linkage-blaze-examples-common/  optional, publish = false
├── linkage-blaze-examples-esp/     publish = false
├── linkage-blaze-examples-rp/      publish = false
├── linkage-blaze-examples-wasm/    publish = false
└── linkage-blaze-editor-wasm/      optional name, publish = false
xtask/                              publish = false
```

The implementation may keep the editor adapter inside an existing unpublished
WASM package instead of creating `linkage-blaze-editor-wasm`. The important
boundary is that `wasm-bindgen`, generated JavaScript, and gallery packaging do
not force the main package to declare `cdylib` for embedded builds.

Shared Armatron, Ballet, Clock, and Skeleton Clock application logic should be
moved to an unpublished common-examples package if that keeps demo APIs out of
the primary library documentation. Platform packages then depend on both the
published library and the common example package.

## Source Migration

1. Rename `crates/linkage-blaze-core` to `crates/linkage-blaze` and rename the
   package and Rust imports from `linkage-blaze-core` / `linkage_blaze_core` to
   `linkage-blaze` / `linkage_blaze`.
2. Preserve the existing allocation-free API as the default library API.
3. Preserve the existing allocation-backed APIs behind `alloc`.
4. Move the full `bvh` module from `linkage-blaze-utils` into the main library
   behind `bvh`.
5. Move `bvh-to-lb` into the main package and make it require `bvh`.
6. Move the current `wasm-bindgen` exports and editor assets into an unpublished
   WASM adapter that depends on the main crate with `alloc`.
7. Delete the `linkage-blaze-utils` package after all consumers, tests, xtask
   commands, gallery metadata, and generated-package paths have moved.
8. Move shared demo application logic out of the public library if doing so
   improves the main docs without duplicating code across platforms.
9. Update every workspace dependency, source import, script, generated target,
   and CI command to use the new names.
10. Do not leave compatibility re-exports or aliases for the old crate names.

## BVH Name Ownership

The current runtime parsers convert generated parameter and mark names to
`&'static str` by leaking allocations. The full BVH converter additionally uses
a global `Mutex`/`OnceLock` interner to bound repeated leaks.

This is not caused by package consolidation, but moving BVH makes the issue more
visible. The preferred eventual API is for allocation-backed linkage storage to
own runtime-generated parameter and mark names, while fixed const linkages keep
their static names. That likely requires an owned/view distinction for runtime
metadata rather than forcing every `LinkageView` name to be `&'static str`.

Do not perform a superficial interner rewrite merely to claim `bvh` is
`alloc`-only. Either complete the ownership design and test it, or initially
document `bvh` as a `std` capability and address ownership separately.

## README Requirements

The root README is the package README and must become the single landing page.
Before publication it must:

- replace the separate core and utils crates.io/docs.rs badges with one
  `linkage-blaze` crates.io badge and one docs.rs badge;
- say that the default build is allocation-free and does not require `std`;
- explain `alloc`, `bvh`, and the command-line installation command;
- show short ESP, RP, WASM, and BVH usage paths;
- retain direct links to the gallery, repository, and platform examples;
- explain that platform examples and browser adapters are not separately
  published; and
- remove all wording that directs new users to `linkage-blaze-core` or
  `linkage-blaze-utils`.

The primary crate description should be close to:

> 3D turtle graphics for animated jointed figures, from allocation-free
> embedded systems to host, BVH, and WebAssembly applications.

The published-library section should be close to:

```markdown
### Rust Crate

**`linkage-blaze`** is the single published crate. Its default build is
allocation-free and does not require the Rust standard library. Enable `alloc`
for runtime-owned programs and `bvh` for host-side motion-capture conversion.
```

The README shown by `cargo package`, crates.io, the generated editor package,
and the repository root must contain consistent links and descriptions. Add a
check or generation step if manual copies can drift.

## Documentation Requirements

- `https://docs.rs/linkage-blaze` is the primary API documentation link.
- The crate root explains the feature model and links to the main fixed,
  allocated, and BVH entry points.
- BVH APIs are grouped under `linkage_blaze::bvh`.
- Host-only APIs clearly state their feature requirements.
- Examples use `linkage_blaze`, never the old crate names.
- The command-line program has `--help` output and a README example.

## Versioning and Existing Published Packages

- Publish the new `linkage-blaze` package as version `0.1.5` so it continues
  the project version visible in demos and release history.
- Do not publish `linkage-blaze-core` or `linkage-blaze-utils` version 0.1.5.
- Leave the already published 0.1.4 packages available; there are no known
  users and yanking provides no useful redirect.
- Mention in the repository release notes that the unified package supersedes
  both old package names.
- Confirm the exact `linkage-blaze` name is still available immediately before
  publication. Search results currently show no package with that exact name,
  but only successful publication reserves it.

## Verification

The implementation is complete only when all of the following pass:

1. Default-feature tests and doctests for `linkage-blaze`.
2. `alloc`-only tests without `std`.
3. Feature-complete host tests including BVH parsing and golden conversion.
4. `bvh-to-lb --help` and representative conversion tests.
5. All ESP board example builds.
6. All RP board example builds.
7. WASM editor and gallery builds, plus browser tests.
8. Pinned latest Clippy and MSRV checks with their intended feature sets.
9. `cargo check-all` from Linkage Blaze.
10. Device Envoy's `cargo check-all` if any shared interface changed.
11. `cargo publish --dry-run -p linkage-blaze` from a clean worktree.
12. Inspection of the packaged file list and packaged README.

CI and local scripts must contain no stale `linkage-blaze-core` or
`linkage-blaze-utils` package commands except deliberate historical text.

## Acceptance Criteria

- `cargo add linkage-blaze` is sufficient for the default embedded-capable
  library.
- The default crate graph does not enable `alloc`, `std`, BVH, or WASM glue.
- Enabling `alloc` does not require `std`.
- `cargo install linkage-blaze --features bvh --bin bvh-to-lb` installs a
  working converter.
- ESP, RP, and WASM examples all depend on `linkage-blaze`.
- Exactly one Linkage Blaze package in the workspace is publishable.
- The root README describes one published crate and all four user paths.
- crates.io and docs.rs links target `linkage-blaze`.
- No source code, Cargo manifest, CI workflow, script, or generated gallery
  configuration relies on the deleted utils package.
- Full local CI and the publication dry-run pass.
