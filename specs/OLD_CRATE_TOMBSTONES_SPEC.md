# Old Crate Tombstones

<!-- todo0 consider deleting this spec once both tombstone releases are published and verified. -->

## Status

Planned after the publication of `linkage-blaze` 0.1.5. The unified crate is
already published; the two tombstone packages described here have not yet been
prepared or published.

## Summary

Publish one final version of each former package name:

- `linkage-blaze-core` 0.1.5; and
- `linkage-blaze-utils` 0.1.5.

These releases exist only to direct people who discover the old crates.io or
docs.rs pages to the unified [`linkage-blaze`](https://crates.io/crates/linkage-blaze)
package. They are documentation tombstones, not supported libraries and not
compatibility layers.

The already-published 0.1.4 releases remain available and unyanked. Crates.io
does not delete historical releases, and yanking them would not provide a
redirect.

## Goals

- Make the latest page for each old package name say prominently that the
  package has moved.
- Link directly to the unified crate's crates.io page, docs.rs documentation,
  repository, and README.
- Reserve the old names as clear historical entry points without maintaining
  two additional APIs.
- Publish the tombstones only after the corresponding unified version is
  available from crates.io.
- Leave no ambiguity about which dependency new users should add.

## Non-goals

- Do not preserve the old Rust APIs.
- Do not re-export `linkage-blaze` from either tombstone.
- Do not depend on `linkage-blaze` merely to make the tombstones compile.
- Do not add type aliases, feature forwarding, or migration shims.
- Do not yank or attempt to delete the 0.1.4 releases.
- Do not continue versioning the tombstones after their final 0.1.5 releases.

There are no known users of the old packages. A forwarding API would therefore
add maintenance and create another apparent way to use the project without
solving a real compatibility problem.

## Repository Layout

Keep the release sources in a clearly archival nested workspace:

```text
tombstones/
├── Cargo.toml
├── linkage-blaze-core/
│   ├── Cargo.toml
│   ├── README.md
│   └── src/lib.rs
└── linkage-blaze-utils/
    ├── Cargo.toml
    ├── README.md
    └── src/lib.rs
```

Exclude `tombstones/` from the main workspace. The nested workspace allows the
two packages to be checked and packaged together without making them ordinary
members of Linkage Blaze's development workspace or changing the requirement
that `linkage-blaze` is its sole publishable product.

The root workspace should include:

```toml
[workspace]
exclude = ["tombstones"]
```

The nested `tombstones/Cargo.toml` should define both members and shared release
metadata, including version 0.1.5, edition 2024, Rust 1.93, the repository URL,
and the existing dual license.

## Package Design

Each package must be a minimal Rust library with no dependencies and no public
API. Its manifest should provide:

- the historical package name;
- version 0.1.5;
- `publish = true`;
- a description beginning with `Deprecated: use linkage-blaze`;
- `repository` and `homepage` pointing to the Linkage Blaze repository;
- `documentation` pointing to `https://docs.rs/linkage-blaze` rather than the
  tombstone's generated documentation; and
- a package-specific README.

The library source should contain only crate documentation. It may use
`include_str!` to make the README the docs.rs landing content, but it must not
declare compatibility items or depend on the unified crate.

Do not use `compile_error!`: merely depending on a tombstone should not break a
build. Do not add a dummy public symbol solely to trigger deprecation warnings.
The package description, crates.io README, and crate-level documentation are
the migration notice.

## Required Tombstone Message

Both READMEs should lead with wording equivalent to:

> This package has moved. Use
> [`linkage-blaze`](https://crates.io/crates/linkage-blaze) instead. Linkage
> Blaze now provides its allocation-free embedded API, optional allocation
> support, BVH conversion, and the `bvh-to-lb` command from one crate.

Each README must provide these direct links:

- [crates.io](https://crates.io/crates/linkage-blaze)
- [docs.rs](https://docs.rs/linkage-blaze)
- [GitHub repository](https://github.com/CarlKCarlK/linkage-blaze)
- [repository README](https://github.com/CarlKCarlK/linkage-blaze#readme)

The former `linkage-blaze-utils` README should explicitly say that BVH users
now enable the `bvh` feature and install the converter with:

```bash
cargo install linkage-blaze --features bvh --bin bvh-to-lb
```

The former `linkage-blaze-core` README should show the replacement dependency:

```toml
[dependencies]
linkage-blaze = "0.1.5"
```

## Publication Order

1. Confirm `linkage-blaze` 0.1.5 is visible on crates.io and its documentation
   is available on docs.rs.
2. Add the nested tombstone workspace and both packages.
3. Run formatting and checks for the nested workspace.
4. Inspect both package file lists.
5. Run a publication dry run for each package from a clean worktree.
6. Commit and push the tombstone sources.
7. The human publishes `linkage-blaze-core` 0.1.5.
8. Verify its crates.io and docs.rs pages and all redirect links.
9. The human publishes `linkage-blaze-utils` 0.1.5.
10. Verify its crates.io and docs.rs pages and all redirect links.
11. Tag the repository release if that has not already been done.
12. Mark this spec complete and consider deleting it.

The agent must not run the real `cargo publish` commands.

## Verification

Before either publication:

```bash
cargo check --manifest-path tombstones/Cargo.toml
cargo package --manifest-path tombstones/linkage-blaze-core/Cargo.toml --list
cargo package --manifest-path tombstones/linkage-blaze-utils/Cargo.toml --list
cargo publish --dry-run --manifest-path tombstones/linkage-blaze-core/Cargo.toml
cargo publish --dry-run --manifest-path tombstones/linkage-blaze-utils/Cargo.toml
```

Also verify that:

- neither package has dependencies;
- neither package exports public API items;
- both packaged READMEs use absolute web links;
- both package descriptions identify `linkage-blaze` as the replacement;
- the main workspace's `cargo metadata` still reports only `linkage-blaze` as
  publishable; and
- the main `cargo check-all` remains unchanged and passes.

## Acceptance Criteria

- The latest crates.io page for each former package begins with a clear move
  notice and links to `linkage-blaze`.
- The latest docs.rs page for each former package displays the same notice.
- Copyable replacement dependency instructions are present.
- BVH and converter users receive the correct feature and installation command.
- No compatibility shim or dependency on the unified crate is introduced.
- The old 0.1.4 releases remain unyanked.
- Both 0.1.5 tombstones are the final releases under the former names.
