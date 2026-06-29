# Pages Publishing

The GitHub Pages site is built from immutable demo snapshots under `pages/demos/<slug>/vN/`.

The current demos and their latest versions live in `pages/demos.tsv`.

## Commands

Build the full Pages site:

```sh
just build-pages
```

Build one demo only:

```sh
just build-pages armatron
```

Bump one demo to the next version and freeze the current live web assets into a new snapshot:

```sh
just bump-demo-version armatron
```

Bump one demo to an explicit version:

```sh
just bump-demo-version armatron v3
```

## Publish One Demo

1. Update the demo's live source files in its `www/` directory.
2. Freeze a new immutable version:

```sh
just bump-demo-version <demo>
```

3. Verify the Pages output for that demo:

```sh
just build-pages <demo>
```

4. Review the generated snapshot under `pages/demos/<demo>/vN/` and the manifest change in `pages/demos.tsv`.
5. Commit and push the changes.
6. In GitHub Actions, run the `Deploy to GitHub Pages` workflow.

The workflow rebuilds the full Pages artifact from the checked-in snapshots, so publishing one demo at a time does not remove old versions of other demos.
