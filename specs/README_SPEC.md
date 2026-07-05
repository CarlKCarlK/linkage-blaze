<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# README Spec

Create a nice but simple top-level `README.md` for the linkage-blaze repository. Keep it short for now; it can grow later.

## Goals

- Give a first-time visitor a quick idea of what the project is.
- Point them at the live gallery (with preview images) as the main showcase.
- Match the tone and structure of the `device-envoy` README where sensible.

## Required Sections

### 1. Title and intro

- Title: `# linkage-blaze` with a GitHub badge in the style of `device-envoy`
  (`https://github.com/CarlKCarlK/linkage-blaze`).
- Tagline: "3D turtle graphics for animated joints. Describe a figure with
  moves, turns, branches, links, joints, disks, and spheres. Then animate
  parameters to bring it to life."
- Do NOT describe the project as "mechanical linkages" / linkage mechanisms —
  that overpromises mechanical-engineering simulation. Steer toward jointed
  drawing / kinematic animation.

### 2. What is Linkage Blaze?

A short, plain-language explanation:

- Linkage Blaze is a small language for making animated jointed drawings. It
  works like 3D turtle graphics: move forward, turn, branch, draw links,
  place joints, and add simple shapes such as disks and spheres. Animate a
  few parameters, and the drawing moves.
- The demos use this to make clocks, skeletons, dancers, and robot-arm-like
  figures.
- Mention the delivery targets: a Rust workspace rendering on
  microcontrollers (e.g. CYD / ESP32 display boards) and in the browser via
  WASM.
- Prominently mention that the core is `no_std` and allocation-free (figures
  can live in flash and run on small microcontrollers), while an opt-in
  `alloc` cargo feature on `linkage-blaze-core` adds heap-based conveniences.
- Keep it to a couple of short paragraphs; no math, no mechanism theory.

### 3. Gallery

- Link to the live gallery on GitHub Pages:
  `https://carlkcarlk.github.io/linkage-blaze/gallery/`
  (verify the exact published URL before committing; the gallery lives under
  `pages/gallery/` and is versioned `v1`, `v2`, `v3`).
- Mention that the gallery shows preview images of each demo and links to the
  live WASM versions.
- Optionally embed one or two preview screenshots in the README itself
  (static PNGs, as used by the gallery preview system), using
  `raw.githubusercontent.com` URLs so they render on crates.io/GitHub.

### 4. Example (linkage code sample)

- Immediately after the Gallery section, show a sample of real linkage code.
- Use the clock demo's linkage
  (`crates/linkage-blaze-example-core/src/clock.lb.rs`), abridged: params,
  face disk, one tick (elide the rest with a comment), and the three hands.
- Briefly introduce it (turtle-graphics steps; the `hour` and `face spin`
  parameters drive the motion) and note that the linkage compiles to a
  `const` — no heap, no runtime parsing — so it lives in flash.
- Link to the full `clock.lb.rs` file.

### 5. Workspace crates (brief)

- Do NOT list all 18 crates individually for now. Instead, describe the
  layout in a few bullets:
  - core crates (`linkage-blaze-core`, `linkage-blaze-example-core`) — `no_std`,
    no-allocation linkage modeling;
  - device crates (`linkage-blaze-cyd*`, `*-armatron-c6`, etc.) — run on
    microcontroller display boards;
  - WASM crates (`*-wasm`) — the browser demos shown in the gallery;
  - `linkage-blaze-editor` — desktop editor.
- Crates are not yet on crates.io, so skip crates.io/docs.rs badges for now.

### 6. Status

- Same style as device-envoy: ⚠️ Alpha / Experimental, API actively evolving,
  good for experimentation and learning.

### 7. Policy on AI-assisted development and contributions

Copy the section from the `device-envoy` README verbatim (adjusting nothing
but repo-specific references):

> The use of AI tools is permitted for development and contributions to this
> repository. AI may be used as a productivity aid for drafting, exploration,
> and refactoring.
>
> All code and documentation contributed to this repository must be reviewed,
> edited, and validated by a human contributor. AI tools are not a substitute
> for design judgment, testing, or responsibility for correctness.
>
> [AGENTS.md](AGENTS.md) contains the general instructions and constraints
> given to AI tools used during development of this repository.

### 8. License

- Dual license MIT / Apache-2.0, same wording as device-envoy:

  > Licensed under either:
  >
  > - MIT license (see LICENSE-MIT)
  > - Apache License, Version 2.0 (see LICENSE-APACHE)
  >
  > at your option.

- The repo currently has **no** `LICENSE-MIT` or `LICENSE-APACHE` files.
  As part of this work, copy `LICENSE-MIT` and `LICENSE-APACHE` from
  `device-envoy` (updating the copyright holder/year if needed).

## Out of Scope (for now)

- Feature list, per-crate badges, videos/articles, forum links, code
  examples, and a development guide. Add these later as the project matures.

## Formatting

Follow the repo Markdown rules in `AGENTS.md`: blank lines around headings,
lists, and fenced code blocks; consistent list markers; American spelling.
