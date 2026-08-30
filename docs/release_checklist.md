# Release Checklist

This is the canonical release procedure for the Linkage Blaze crate and web
demo gallery.

## 1. Choose the Release Version

- Pick the next crate version and confirm that its tag does not already exist.
- Start from a clean, current `main` checkout unless the release intentionally
  uses another integration workflow.
- Keep the release commit local until the package preflight passes.

```bash
git status --short --branch
git tag --list 'vX.Y.Z'
```

## 2. Audit Demo and Gallery Versions

Demo versions are independent from crate versions. Bump a demo only when its
rendered behavior, layout, controls, assets, or browser integration changed
since that demo's current snapshot was created.

- Read `pages/demos.tsv` to identify each demo's current version.
- Find when each current snapshot was created. For example:

```bash
git log --diff-filter=A --format='%h %aI %s' -- pages/demos/clock/vN
```

- Review commits after that date against the demo's Rust source, WASM adapter,
  page shell, shared CYD browser assets, and gallery metadata.
- Do not infer that every demo changed merely because the crate version or a
  dependency version changed.
- Bump every materially changed demo before snapshotting the gallery:

```bash
just bump-demo-version clock
just bump-demo-version armatron
just bump-demo-version skeleton-clock
```

  Run only the commands for demos that actually changed. An explicit version,
  such as `v7`, may be supplied as the final argument.
- After all demo pointers are current, snapshot the gallery when its rendered
  cards, ordering, metadata, or demo-version targets changed:

```bash
just bump-gallery-version
```

- Confirm `pages/demos.tsv` and the new gallery snapshot point to the intended
  mix of new and unchanged demo versions.
- Confirm new snapshots do not contain generated `pkg` or `node_modules`
  directories.

## 3. Update Versions and Dependencies

- Update the workspace version in `Cargo.toml`.
- Update the workspace `linkage-blaze` dependency requirement.
- Update all Device Envoy requirements together when coordinating a Device
  Envoy release.
- Update the Device Envoy tag used by the Clippy, MSRV, and Pages workflows.
- Refresh the lockfile through Cargo and review it so unrelated packages do not
  move:

```bash
cargo update -w
git diff -- Cargo.lock
```

## 4. Finalize the Changelog

- Add a concise, stable release section to `CHANGELOG.md`.
- Describe material crate, demo, gallery, dependency, and behavior changes.
- Remove draft markers. This command must print no matches:

```bash
rg -n -i '\bunreleased\b|\btbd\b' CHANGELOG.md
```

## 5. Run Release Checks

- Check formatting and release-priority TODOs:

```bash
cargo fmt --all -- --check
rg -n '(?i)\btodo''0+\b' crates xtask specs docs
```

- Test the snapshot tooling when demo or gallery versions changed:

```bash
cargo test -p linkage-blaze-xtask
```

- Build the Pages artifact and run browser contract tests when web-facing code
  or snapshots changed:

```bash
just test-cyd-browser
```

- Run the repository's full local CI equivalent:

```bash
cargo check-all
```

## 6. Package Preflight

- Commit the release preparation locally but do not push it yet.
- Use a fresh target directory for the locked publish dry-run:

```bash
CARGO_TARGET_DIR="$(mktemp -d)" \
  cargo publish --dry-run --locked -p linkage-blaze
```

- Review the package inventory:

```bash
cargo package --list -p linkage-blaze
```

- Fix packaging problems in the local release commit before pushing.

## 7. Push and Require Automation to Pass

- Push the validated release commit through the chosen integration workflow so
  it lands on `main`.
- Require the Clippy and MSRV workflows to pass.
- A push to `main` automatically starts `.github/workflows/pages.yml`. That
  workflow builds the WASM applications, assembles `target/pages`, verifies the
  canonical Device Envoy CYD assets, and deploys GitHub Pages.
- Do not normally run `just publish-pages`; it is a manual retry mechanism for
  the same Pages workflow.

## 8. Verify the Web Deployment

- Require both the Pages build and deploy jobs to pass.
- Open the live gallery and each newly versioned demo:

```text
https://carlkcarlk.github.io/linkage-blaze/demos/
https://carlkcarlk.github.io/linkage-blaze/demos/vN/
https://carlkcarlk.github.io/linkage-blaze/demos/<demo>/vN/
```

- Confirm the live gallery's "Open latest" links target the intended versions.
- Check loading, rendering, touch/mouse interaction, version selectors, images,
  and browser console errors.
- Verify unchanged demos still identify their prior current versions.

## 9. Publish the Crate

Publishing is effectively permanent. Run the real publish only from the clean,
CI-approved `main` commit:

```bash
git status --short --branch
cargo publish --locked -p linkage-blaze
```

The person performing the release must run the real `cargo publish`; automated
agents must not run it.

- Wait until the exact version resolves from crates.io outside the workspace:

```bash
cd /tmp
cargo info --registry crates-io linkage-blaze@X.Y.Z
```

## 10. Tag and Create the GitHub Release

- Tag the exact published `main` commit and push the annotated tag:

```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

- Create a non-draft, non-prerelease GitHub Release from the tag using the
  curated changelog section.
- Verify the release metadata and links.

## 11. Final Verification

- Verify the exact crate version on crates.io.
- Verify its docs.rs build.
- Confirm the README badges resolve to the new release.
- Confirm the live gallery remains healthy after crate publication and tagging.
