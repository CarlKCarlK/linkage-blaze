<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Editor: Vendor Runtime JS Dependencies (Fix Dead Editor on CDN Failure)

## Problem

The gallery-served editor page is dead: nothing can be typed or pasted, and the toolbar appears broken.

Diagnosis (reproduced headlessly with Playwright on 2026-07-05):

- `crates/linkage-blaze-editor/www/src/main.js` imports CodeMirror at runtime from the `esm.sh` CDN.
- `esm.sh` is currently returning HTTP 500 for `@codemirror/view` (`?target=es2022` builds). The server's own error body says `no space left on device` — an outage on their side, not a bug in this repo.
- Because ES module graphs are all-or-nothing, the failed import aborts the entire `main.js` module. CodeMirror never mounts into `#source` (so there is nothing to type into), and none of the toolbar button listeners attach.
- The "Open and Insert appear twice" observation is the static HTML toolbar working as designed (one Open/Insert pair for the file picker, one for the Recent… dropdown), but with all JS dead it reads as a rendering bug. See the UX cleanup below.

Root cause: the editor's core functionality depends on two third-party CDNs at page-load time:

1. `esm.sh` — CodeMirror 6 packages, imported with floating `@6` versions (`main.js` lines 4–9).
2. `cdn.jsdelivr.net` — three.js 0.160.0 via the import map in `index.html`.

Any CDN outage, network filtering, or offline use kills the whole editor. The floating `@6` versions are an additional hazard: even when `esm.sh` is up, an upstream minor release can change resolution and (a known CodeMirror failure mode) produce duplicate `@codemirror/state` instances, which breaks input handling silently.

## Fix

Vendor all runtime JS dependencies into the repo so the served page is fully self-contained. No request should leave `www/` at page load.

### 1. Build a single vendored dependency bundle

Add a tiny bundling setup under `crates/linkage-blaze-editor/www/`:

- `package.json` with exact-pinned devDependencies: `esbuild`, `three@0.160.0`, `codemirror` packages (`@codemirror/view`, `@codemirror/commands`, `@codemirror/language`, `@codemirror/autocomplete`, `@codemirror/lang-rust`, `@codemirror/theme-one-dark`) at the exact versions currently resolving. Pin exact versions (no `^`) so the bundle is reproducible.
- `deps-entry.js` — a re-export entry module:

```javascript
export * as THREE from "three";
export { OrbitControls } from "three/addons/controls/OrbitControls.js";
export { CSS2DRenderer, CSS2DObject } from "three/addons/renderers/CSS2DRenderer.js";
export { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from "@codemirror/view";
export { history, historyKeymap, defaultKeymap, toggleLineComment, indentWithTab } from "@codemirror/commands";
export { syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput } from "@codemirror/language";
export { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
export { rust } from "@codemirror/lang-rust";
export { oneDark } from "@codemirror/theme-one-dark";
```

- Bundle it once with esbuild into `crates/linkage-blaze-editor/www/vendor/editor-deps.js` (ESM format, minified):

```bash
npx esbuild deps-entry.js --bundle --format=esm --minify --outfile=vendor/editor-deps.js
```

- Check the generated `vendor/editor-deps.js` into git (roughly 1 MB). This keeps `just build-editor` and CI free of any npm/network requirement; npm is only needed when regenerating the bundle to upgrade a dependency.

Bundling everything through one entry also guarantees a single shared `@codemirror/state` instance, eliminating the duplicate-instance failure mode entirely.

### 2. Update `main.js` and `index.html`

- Replace the six `https://esm.sh/...` imports and the `three`/`three/addons/` imports in `main.js` with imports from `../vendor/editor-deps.js`. Since `THREE` becomes a namespace re-export, the existing `import * as THREE from "three"` becomes `import { THREE, OrbitControls, CSS2DRenderer, CSS2DObject, ... } from "../vendor/editor-deps.js"`.
- Delete the import map `<script type="importmap">` block from `index.html` — nothing external remains to map.
- Keep the existing cache-busting query pattern on the `main.js` script tag; add one to the vendor import if needed when the bundle is regenerated.

### 3. Add a `just` recipe for regeneration

```
# Regenerate the editor's vendored JS dependency bundle (requires npm)
build-editor-deps:
    cd {{_editor_www}} && npm ci && npx esbuild deps-entry.js --bundle --format=esm --minify --outfile=vendor/editor-deps.js
```

Do not add this to `check-all`; the checked-in bundle is the source of truth for builds.

### 4. Toolbar UX cleanup (small, same change)

The duplicate-looking labels confused even with JS working. In `index.html`:

- Relabel `#btn-recent-open` to `Open Recent` and `#btn-recent-insert` to `Insert Recent` (keep the tooltips), or visually group the `Recent…` select with its two buttons (e.g. a thin separator or a wrapping `<span class="recent-group">` with distinct background) so the second pair clearly belongs to the dropdown.

### 5. Cleanup

- Delete the stale `linkage_blaze.js` / `linkage_blaze.d.ts` / `linkage_blaze_bg.wasm*` files in `crates/linkage-blaze-editor/www/pkg/` left over from the old `--out-name`; `main.js` imports `linkage_blaze_editor.js` only. (The directory is gitignored, so this is a local hygiene step.)

## Non-goals

- No change to the wasm build (`wasm-pack` recipes) or the Rust editor crate.
- No offline/vendoring work for the other demo pages (clock, ballet, printer, mocap) in this spec; if they share the same CDN pattern, file that as follow-up work.

## Verification

1. `just build-pages editor`, serve `target/pages`, open `/demos/editor`, confirm: CodeMirror mounts with the default program, typing and pasting work, sliders render, three.js view draws.
2. Browser devtools network tab: zero requests to `esm.sh` or `cdn.jsdelivr.net`.
3. Repeat with network access disabled (devtools "Offline" after first load, or firewall the CDNs) — the editor must still fully work.
4. Optional: keep a headless Playwright probe (load page, assert `.cm-editor` mounts, type a sentinel string, assert it appears) as a scriptable regression check; a working probe script from this diagnosis exists and can be adapted into `.tools/`.
