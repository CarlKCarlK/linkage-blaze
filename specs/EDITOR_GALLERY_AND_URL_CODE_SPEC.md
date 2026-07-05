<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Editor: Gallery Card and `#code=` Startup URL

Two related features that make the editor a first-class, shareable demo:

1. Publish the editor as the fifth item in the demo gallery.
2. Let a URL carry the initial program text, so links can open the editor pre-loaded with a specific linkage.

## Prerequisite

`specs/EDITOR_VENDOR_DEPS_SPEC.md` must land first. Both features edit `crates/linkage-blaze-editor/www/`, and publishing the editor to GitHub Pages while it still imports CodeMirror from `esm.sh` would freeze the CDN fragility into an immutable snapshot.

## Feature 1: Editor as the Fifth Gallery Item

### Manifest

Add a row to `pages/demos.tsv`:

```text
editor	Editor	v1	crates/linkage-blaze-editor	crates/linkage-blaze-editor/www	linkage_blaze_editor	v1
```

Then create the first frozen snapshot with the existing flow:

```sh
just bump-demo-version editor v1
just build-pages editor
```

No changes to `bump-demo-version` or `build_demo` are expected: the snapshot copy already excludes `pkg/`, and `find_pkg_out_name` resolves `linkage_blaze_editor` from the `../pkg/linkage_blaze_editor.js?v=…` import in `src/main.js` (the `?v=` suffix is already handled because parsing stops at the first `.js`).

Caution: `find_pkg_out_name` scans every `.js` file under the snapshot in sorted order and takes the first `pkg/<name>.js` match. After the vendoring spec lands, verify the minified `vendor/editor-deps.js` does not accidentally contain a `pkg/…​.js` substring that sorts before `src/main.js`; if it does, tighten `parse_pkg_out_name` to require the `./pkg/` or `../pkg/` import forms.

### Preview image

`capture_demo_preview` renders previews by running a `cargo test` in `linkage-blaze-example-core` that draws a linkage frame. That cannot draw the editor's actual UI (CodeMirror pane, three.js viewport, sliders), and a plain armatron render would be indistinguishable from the existing Armatron card.

Extend `PreviewSpec` with a source enum instead of forcing every demo through the test renderer:

```rust
enum PreviewSource {
    /// Rendered at build time by a cargo test in linkage-blaze-example-core.
    RenderTest { feature: &'static str, test_name: &'static str },
    /// A checked-in screenshot copied verbatim into the Pages output.
    StaticFile { repo_path: &'static str },
}
```

- The four existing demos keep `RenderTest` with their current feature/test values.
- The editor uses `StaticFile` pointing at a checked-in screenshot, e.g. `pages/demos/editor/preview-source.png` — a real screenshot of the editor with the default armatron program loaded (code pane on the left, 3D view on the right), so the card visibly reads as "an editor," not another linkage render. Landscape orientation.
- `capture_demo_preview` copies the static file to `target/pages/demos/editor/preview.png` and keeps the existing non-empty-file check.
- The screenshot can be captured manually or with the headless Playwright probe from the vendoring spec's verification step; either way it is checked in, so `just build-pages` stays deterministic and browser-free.

### Card content

`demo_card_html` needs no structural change. Consider setting the eyebrow text per demo (`"Preview"` for renders, `"Tool"` for the editor) so the card is honest about being an interactive tool rather than an animation — optional polish, not required.

## Feature 2: Initial Program Text via URL

### Design

Carry the program in the URL fragment, not a query parameter:

```text
https://…/editor/v1/#code=<base64url of UTF-8 source>
```

Reasons for the fragment: it is never sent to the server (no log leakage, works identically under `file://`, the local demo gallery, and GitHub Pages), it has no practical length limit, and it survives reload so a shared link stays a shared link. Support exactly one parameter name (`code`) and one encoding — no `?code=` alias (avoid redundant API paths, per AGENTS.md).

Encoding is base64url without padding over the UTF-8 bytes of the source. In `main.js`:

```javascript
function encodeCodeFragment(source) {
  const bytes = new TextEncoder().encode(source);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

function decodeCodeFragment(fragment) {
  const base64 = fragment.replaceAll("-", "+").replaceAll("_", "/");
  const binary = atob(base64);
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}
```

No compression: programs are a few KB; base64 expansion (~4/3) keeps fragment URLs comfortably small. If programs ever outgrow that, revisit with a vendored compressor — out of scope now.

### Load behavior

Initial document precedence in `main.js` (currently `localStorage.getItem(STORAGE_KEY) ?? default_program()`):

1. `#code=…` fragment, if present and decodable;
2. otherwise `localStorage`;
3. otherwise `default_program()`.

If decoding fails (malformed base64/UTF-8), fall back to the next source and surface the problem in the existing `#error` element rather than failing silently.

Accepted behavior to document in a code comment: autosave is a single localStorage slot, so opening a `#code=` link and then editing overwrites the previously autosaved program. That matches the existing model (opening a file has the same effect) and is not worth a second storage slot now.

Leave the fragment in the address bar after loading, so refresh reproduces the shared state. The first edit should clear it (`history.replaceState`) — once the document diverges, the URL no longer describes what is on screen.

### Share button

Add a `Copy Link` button to the file toolbar (after `Save As`, before the Recent group). On click:

1. Build `location.origin + location.pathname + "#code=" + encodeCodeFragment(getSource())`.
2. `navigator.clipboard.writeText(url)`.
3. Give lightweight feedback (button text flips to `Copied!` for ~1.5 s).

Keyboard shortcut is not needed; the existing Ctrl+S/O/I handler stays untouched.

### Uses this unlocks (not built here)

- Gallery or README links that open the editor pre-loaded with each example (`armatron1.lb.rs`, the clock, etc.).
- "Open in editor" buttons on the other demo pages.

## Non-goals

- No `#file=`/remote-fetch parameter (loading arbitrary URLs into the editor is a different feature with different security questions).
- No multi-slot autosave or document history.
- No compression of the fragment payload.

## Verification

1. `just build-pages editor`, serve `target/pages`, and confirm `/demos/editor` still starts from localStorage/default with no fragment.
2. Click `Copy Link`, open the copied URL in a private window: the same program must appear. Type a character: the fragment must disappear from the address bar and the render must update.
3. Corrupt the fragment by hand (`#code=%%%`): the editor must fall back to localStorage/default and show a decode message in `#error`.
4. Round-trip a program containing non-ASCII characters (e.g. a `°` in a comment) through Copy Link.
5. `just bump-demo-version editor v1 && just build-pages`: the gallery shows five cards, the editor card's preview renders, `Open latest` loads a working editor from `target/pages/`, and the page makes no CDN requests.
6. `just check-all` still passes (xtask changes compile; existing demos' previews still render).
