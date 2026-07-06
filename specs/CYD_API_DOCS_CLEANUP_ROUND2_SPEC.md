<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# CYD API Docs Cleanup — Round 2 Spec

Round 1 (`CYD_API_DOCS_CLEANUP_SPEC.md`) landed for the core `cyd` page
(calibration re-exports collapsed, pixel plumbing moved, macros re-homed).
This spec covers Carl's round-2 review findings on core, plus the ESP/RP work
from round 1 that **has not landed yet** (verified against the rebuilt doc
pages). This spec supersedes the round-1 spec's Phase 2; delete the round-1
spec when starting this work.

Target quality bar remains `wifi_auto`: short module doc, small public
surface, one canonical example per concept, doctests that compile (and run
where possible).

All code changes are in `device-envoy`; downstream fixes in `linkage-blaze`.
Run `just check-all` in **both** repos before finishing.

## Phase 1 — core `cyd` module doc: cut the prose

`device-envoy-core/src/cyd.rs` module doc is three paragraphs; `wifi_auto` is
two sentences. Carl: "I don't want to know about 'opinionated', etc. This
should be short and point [to] the struct and/or traits that have interesting
doc tests and explain and show usage."

- Delete the entire "Modeled on device-envoy's opinionated device
  abstractions ..." paragraph (self-referential design narrative, not user
  docs). If that rationale is worth keeping, move it to a `//` code comment.
- Keep a two-sentence shape like `wifi_auto`'s: what the device is, then
  "See [`Cyd`] for the primary trait and usage example; [`CydTouch`] reads
  calibrated [`TouchEvent`]s." Drop the word "opinionated" everywhere in doc
  text.
- The word "frames" narrative sentence can survive as the second sentence if
  it stays one line.

## Phase 2 — doctest strategy (the core of round 2)

Carl: "We need runnable doc tests." Current state: exactly two `rust,no_run`
doctests in all of core's cyd code (both in `cyd.rs`), zero in `memory`,
`tiling`, `calibration`, and zero in the ESP/RP cyd modules (their four
fences are ```` ```ignore ````, which AGENTS.md forbids without a callout).

Policy (write it once, apply everywhere): **one canonical example per
concept, on the primary type; every other doc site links to it** instead of
duplicating or paraphrasing.

| Concept | Canonical home | Kind |
| --- | --- | --- |
| End-to-end draw + touch | `Cyd` trait | `rust,no_run`, hidden mini test double (the `wifi_auto::connect` pattern) |
| Tiled drawing loop | `CydDisplay::tiles` (already exists) | `rust,no_run` |
| Calibration at startup | `ensure_calibration` | `rust,no_run`, hidden mini mocks (needs `Cyd + CydRawTouch`, `FlashBlock`, `Button`) |
| Host testing / screenshots | `memory` module doc or `MemoryCyd` | **runnable** (see below) |
| TGA images | `Image565Fixed` (via `cyd::tga565!`) | `rust,no_run` |

Key trick for *runnable* doctests: `memory` is `#[cfg(feature = "host")]`,
and doctests attached to cfg'd-out items are only collected when the feature
is on. So examples on `MemoryCyd` can be plain runnable ```` ```rust ````
doctests (construct `MemoryCyd`, draw, `block_on(frame.flush())`, assert a
`pixel(...)` value) and they execute under `cargo test --features host`.
Core `no_std` items cannot reference `memory` in their doctests (they compile
without the feature), so they use the hidden-mock pattern instead — that is
exactly how `wifi_auto` does it (`ButtonMock`/`DemoWifiAuto` behind `#`
lines).

Also:

- Replace all four ```` ```ignore ```` fences in `device-envoy-esp/src/cyd.rs`
  (lines ~66, ~270) and `device-envoy-rp/src/cyd.rs` (lines ~70, ~284) with
  `rust,no_run`. If peripheral setup genuinely cannot compile in a doctest,
  hide the setup behind `#` lines rather than ignoring the whole block, and
  call out any remaining `ignore` explicitly.
- Make sure `just check-all` runs `cargo test --doc --features host` for core
  so the runnable memory doctests are actually exercised.

## Phase 3 — `memory` module: docs and public-surface trim

Carl: "its structs have no docs at all and I see no doc tests for it... WHY
IS SO MUCH STUFF PUBLIC!!!!??"

Verified external usage (device-envoy tests/examples + all of
linkage-blaze):

**Used downstream — keep `pub`, add one-line docs:**

- `MemoryCyd` (linkage-blaze golden tests in `ballet.rs`, `clock.rs`,
  `skeleton_clock.rs`, `armatron/main.rs`) and its used methods:
  `set_frame_budget`, `memory_button`, `push_touch_event`, `flush_count`,
  `last_flush_rectangle`, `pixel`.
- `MemoryCydError`.
- `MemoryButton` (returned by the used `memory_button()`).
- `MemoryDisplayPart`, `MemoryTouchPart`, `MemoryFrame` — must stay `pub`
  because they are `Cyd::Display`/`Touch`/`Frame` associated types, but each
  still needs a one-line summary.
- `assert_framebuffer_matches_expected_png` — Carl doubts this should be
  public, but four linkage-blaze test files call it; it is the golden-PNG
  assertion downstream tests are built on. Keep it `pub`, give it real docs
  (including the bless/update-expected workflow). If Carl still wants it out
  of the API, the alternative is duplicating it into linkage-blaze test
  helpers — decide with him before moving.

**Zero external users — make `pub(crate)` (or private):**

- `MemoryFrameClock` and `MemoryCyd::frame_clock()`.
- `MemoryFlashBlock` (in-crate calibration tests only).
- Scripting methods with no external callers: `script_raw_frames`,
  `script_raw_frames_owned`, `script_touch_frames`,
  `script_touch_frames_owned`, `script_idle_frames`, `push_raw_touch_event`,
  `script_tap`.
- `write_framebuffer_tga`, `write_framebuffer_png`, `framebuffer()`
  (keep whatever `assert_framebuffer_matches_expected_png` needs internally).

Note: `pub(crate)` on methods of a `pub` struct is legal — use it rather than
deleting, since the in-crate calibration tests use the scripting API.

Then give the module doc one runnable doctest (Phase 2) showing the intended
downstream test shape: construct → draw → flush → `pixel`/PNG assert.

## Phase 4 — `tiling` module: broken link, stale prose, over-public helpers

- **Broken `[Rectangle]` link** in the module doc renders literally as
  "[ Rectangle ]". The sentence is also stale — it reads as if `Rectangle`
  were a local tiling type ("[`Rectangle`] describes a single rectangle (for
  example a full-width text band)"). Rewrite the sentence around
  `embedded_graphics::primitives::Rectangle` with a full-path intra-doc link,
  or drop it.
- **Helper functions.** External usage is one call site each:
  `max_pixel_count` + `rectangle_pixel_count`
  (`linkage-blaze-classic/examples/skeleton-clock.rs`), `max_u32`
  (`linkage-blaze-example-core/src/skeleton_clock.rs`),
  `max_rectangle_pixel_count` (`.../clock.rs`); `div_ceil_usize` has zero.
  - Make `div_ceil_usize` private immediately.
  - Keep the two domain-meaningful ones: `rectangle_pixel_count`,
    `max_rectangle_pixel_count`.
  - Privatize `max_u32` and `max_pixel_count` (they exist only because
    `cmp::max` is not `const`); update the two linkage-blaze call sites to
    use `max_rectangle_pixel_count` where it fits, else define the one-line
    `const fn` locally per AGENTS.md ("prefer `const` values defined in the
    local context"). Judgment call — if the local-copy outcome feels worse,
    keep `max_pixel_count` public and documented as the buffer-sizing helper
    and still drop `max_u32`.
- **Example location:** the tiling loop example already lives on
  `CydDisplay::tiles` — that stays the single canonical home. The tiling
  module doc and `TileGrid` docs must *link* to it ("see
  [`CydDisplay::tiles`](...) for the draw loop") rather than growing their
  own copies. `TileGrid::new` may keep a two-line construction snippet only
  if linking alone reads badly.

## Phase 5 — calibration surface polish

- `CalibrationConfig` and `EnsureCalibrationSettings` render with **empty
  description cells** on the cyd index — add one-line summaries.
- `ensure_calibration_with_settings`'s summary is a paragraph; rustdoc shows
  all of it in the index. First line: "Like [`ensure_calibration`], with
  tunable flow timings." Move the browser-frame-budget rationale into the doc
  body.
- Add the `ensure_calibration` doctest (Phase 2 table).
- **`CydRawTouch` vs `CydTouch` — both stay public.** Carl asked "Both of
  these?": yes — `CydRawTouch` appears in the public bounds of
  `ensure_calibration` (`C: Cyd<Error = E> + CydRawTouch<Error = E>`) and is
  implemented by the ESP/RP crates, so it cannot be hidden. Fix is
  cross-linking docs so the pairing is obvious: `CydTouch` = "calibrated,
  screen-space events — what apps read"; `CydRawTouch` = "raw controller
  samples — implemented by devices so [`ensure_calibration`] can run".
- The structs Carl questioned on the index page all have verified downstream
  users and stay public: `ContiguousPixels` (clock), `CopySizeError` (return
  type of `CydFrame::copy_from_565`), `Image565Fixed`/`Mask`/`View` (all
  examples), `RawPoint` (`script_tap`, ESP/RP), `Tiles` (returned by
  `tiles()`), `CalibrationConfig`/`EnsureCalibrationSettings` (wasm apps).
  The problem was missing docs, not visibility — after this spec every row on
  the index has a description.

## Phase 6 — ESP and RP modules (round-1 Phase 2, still not landed)

Verified against the current doc builds: nothing from round 1 Phase 2 has
been applied. Same issues in both crates — fix together.

### 6.1 Drop the renamed trait aliases

`device-envoy-esp/src/cyd.rs:33` / `device-envoy-rp/src/cyd.rs:21` still
re-export `Cyd as CydDevice`, `CydDisplay as CydDisplayTrait`,
`CydFrame as CydFrameTrait`, `CydTouch as CydTouchTrait`, so both platform
pages list the traits under names that exist nowhere else. No collision
forces this (`CydEsp`/`CydRp` are structs). Re-export under real names;
mechanical rename in callers (examples only import them anonymously:
`CydDevice as _` → `Cyd as _` in `cyd_tiles.rs`/`cyd_touch_paint.rs` across
all chip dirs) and in doc links (`CydDisplayTrait::frame_mut` →
`CydDisplay::frame_mut` in `src/cyd.rs` and `src/cyd/text.rs` of both
crates).

### 6.2 Port RP's doc comments to the blank ESP items

Blank on the ESP page, documented on RP: `CydEsp` (the primary type!),
`CalibratedCydEsp`, `CydDisplayEspPart`, `CydTouchEspPart`, `CydFrameEsp`,
`PixelBuffer`, `RegionBuffer`, `RegionView`, `CydError`,
`CydDisplayEspFlushError`, `CydDisplayEspInitError`, `CydTouchEspInitError`,
`DISPLAY_SPI_HZ`, `TOUCH_SPI_HZ`. Adapt the RP one-liners. Blank on **both**
pages: `CalibrationConfig` (fixed by Phase 5 in core), `RegionPixels` (core
doc landed in round 1 — verify it shows after rebuild).

### 6.3 Convert the `ignore` doctests

Covered in Phase 2 — the `CydEsp`/`CydRp` usage examples are currently
`ignore` fences, which also means the module docs' claim "See [`CydEsp`] for
the primary constructor and usage example" points at code that is never
compiled.

### 6.4 Re-check platform re-export lists

After the core trims, confirm each remaining platform re-export
(`RegionPixels`, `RawPoint`, `RawTouchEvent`, `CalibrationConfig`,
`Orientation`, `TouchEvent`, `SCREEN_*`, `tiling`) is used from the platform
path; drop any that aren't.

## Phase 7 — verification

1. `device-envoy`: `just check-all` (must include `cargo test --doc
   --features host` for core), then rebuild all three doc sets and re-render
   the four pages (core cyd, core cyd/tiling, esp cyd, rp cyd). Acceptance:
   no empty description cells, no literal `[Rectangle]`-style broken links,
   no `ignore` fences without a written justification, module docs ≤ 2 short
   paragraphs.
2. `linkage-blaze`: `just check-all` (catches the tiling-helper and memory
   `pub(crate)` fallout in `linkage-blaze-example-core` and
   `linkage-blaze-classic`).
3. Compare each page side-by-side with `wifi_auto`'s once more.
