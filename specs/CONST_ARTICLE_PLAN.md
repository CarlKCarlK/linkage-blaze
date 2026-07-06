<!-- todo0 consider deleting this spec once the article is published. -->

# Article Plan: "9 Rules for `const` and Constant Functions" (Rust, embedded, no-alloc)

Planning document for a Medium article drawing on `linkage-blaze` and `device-envoy`.

## Audit: where const lives in the two repos

### device-envoy (~270 `const fn` occurrences)

| Area | Files | What it does in const |
| --- | --- | --- |
| Audio | `device-envoy-core/src/audio_player.rs` (52) | Parses WAV/ADPCM headers (`__parse_adpcm_wav_header`), decodes ADPCM → PCM (`with_pcm`), applies gain by decode→scale→re-encode (`with_gain`), resamples, synthesizes sine waves in Q31 fixed point (Bhaskara approximation, `sine_sample_from_phase`), converts `Duration` ↔ sample counts, `Volume::percent`/`spinal_tap`/`db` typed units |
| 2D LED layout | `device-envoy-core/src/led2d/layout.rs` (18) | `LedLayout<N, W, H>` closed set of constructors (`linear_h/v`, `serpentine_row/column_major`) and transforms (`rotate_cw: LedLayout<N,W,H> → LedLayout<N,H,W>` — dimensions swap in the type), const `equals` enabling `const _: () = assert!(...)` compile-time tests in doctests |
| LED strips | `led_strip.rs` (5) | `generate_combo_table` builds a 256-entry gamma+brightness lookup table into flash; `max_brightness(worst_case_ma)` computes a power-budget safety cap at compile time |
| Servos | `servo_player.rs` (5) | `linear` / `combine` — const generation and concatenation of motion step arrays |
| Misc | `clock.rs`, `capabilities.rs` (const bitset with `with`/`contains`), `button_watch.rs`, `wifi_auto/fields.rs`, `led4.rs` | Const constructors, `new_static()` + `StaticCell` no-alloc singleton pattern |
| Codegen | `xtask/src/*_generated.rs` | xtask writes source files that invoke const-evaluating macros (`pcm_clip!`, `adpcm_clip!`); a `#[cfg(doc)]` mirror module documents the macro's generated items |

### linkage-blaze (~200 `const fn` occurrences)

| Area | Files | What it does in const |
| --- | --- | --- |
| Linkage language | `linkage-blaze-core/src/lib.rs` (99) | Fluent const DSL: `.yaw().forward().pen_down().yaw_param(...)` chains build `LinkageFixed<DOF, MARKS>` entirely at compile time; `freeze_param_*` is compile-time partial evaluation; `with_joint_spheres` synthesizes new geometry; `emit_fixed_step_methods!` macro shares the method set across storage types |
| BVH mocap | `bvh_parse.rs` (24) | Full text parser in const: `parse_f32`, `scale_pow10`, whitespace/token skipping, channel counting, normalization, f32 → u16 quantization (halves flash). Existing `todo000` notes: parsing a 764 KB BVH file in const takes ~8 s of compile time — good anecdote |
| Images | `cyd-core/src/tga.rs` (13) | TGA decode to RGB565 in const, alpha/magenta/white transparency masks, `tga565!` macro wraps `include_bytes!` + const decode |
| Tiling | `cyd-core/src/tiling.rs` (15) | Const `max`/`div_ceil` helpers sizing DMA tile buffers; `TileGrid` |
| Math | `math.rs` (5) | `degrees_to_radians`, const vector/matrix accessors |
| UI controls | `example-core/src/ui.rs` (11), `armatron/controls.rs` | `static Slider`/`Button`/`Label` items with stable addresses; `ptr::eq` identifies the active control with no IDs and no allocation; module docs already explain the `static`-vs-`const` distinction |
| Memory | `cyd-memory` | Static framebuffer allocation |

## Candidate rules (trim/merge to ~9; currently 12 — see "Trimming" below)

1. **Do real work at compile time, not just name numbers.** `const` isn't only for `MAX_LEN: usize = 64`. Parse a WAV header, decode an image, parse a 764 KB motion-capture text file — the result lands in flash with zero startup cost and zero RAM.
2. **Wrap `include_bytes!` + a const fn in a macro to make an asset compiler.** `tga565!("logo.tga")`, `bvh_motion!("dance.bvh")`, `pcm_clip! { ... }` — the macro also computes the const-generic sizes so users never state them.
3. **`assert!` in a const fn is a compile-time validator.** A malformed audio file or an out-of-range block size fails the *build*, not the device in the field. (Ties to the house rule: no silent clamping.)
4. **Write compile-time unit tests with `const _: () = assert!(...)`.** The LED layout doctests assert `ROTATED.equals(&EXPECTED)` at compile time. Requires writing a const `equals` because trait `PartialEq` isn't callable in const.
5. **Transform data, don't just validate it: const codecs.** Store ADPCM (4× smaller flash) and const-decode to PCM when RAM is cheap, or ship PCM when CPU is; apply gain by decode→scale→re-encode; resample; quantize f32 mocap to u16. The compression/CPU/flash tradeoff becomes a one-line type-level choice.
6. **Const math is fixed-point math (mostly).** No `sin`/`powf` in const on stable — so: Bhaskara sine approximation in Q31 (`sine_sample_from_phase`), `scale_pow10` for float parsing, precomputed 256-entry gamma tables, and a current-budget (mA) brightness cap computed from datasheet numbers.
7. **Design closed operation sets on const-generic types.** `LedLayout<N, W, H>` constructors and transforms compose, and `rotate_cw` swapping `W`/`H` in the return type means the type system tracks panel orientation. Pair a `Fixed` (const-generic, lives in flash) with a type-erased `View` for runtime code. *(Internally "an algebra" — don't use that word in the article.)*
8. **A fluent DSL can be 100% const.** The linkage language: method chains build the whole robot-arm program at compile time; `freeze_param` is partial evaluation; `with_joint_spheres` generates geometry. Show the `emit_fixed_step_methods!` trick for sharing the DSL across storage types.
9. **`const` is a value; `static` is an identity.** UI controls are `static` so `ptr::eq` can answer "is this the active slider?" with no IDs, no enums, no allocation. A `const` control could be duplicated/inlined and pointer comparison would silently break. Related: `new_static()` + `StaticCell` for no-alloc singletons.
10. **Const generics carry sizes through your API — let inference fill them in.** Output sizes live in the type (`LedLayout<N, W, H>`, `PcmClipBuf<RATE, SAMPLE_COUNT>`, `with_joint_spheres<N_OUT>`, `freeze_param<OUT_DOF>`); an `assert!(OUT_N == N1 + N2)` inside the fn checks what stable Rust can't yet express as a bound (`generic_const_exprs`). Corollary: annotate the receiving `const` and skip the turbofish — inference flows backward from the annotation.
11. **Define `combine` functions: build big const data from small const pieces.** Stable const can't call trait methods, so concatenation/merging must be hand-rolled const fns: `servo_player::combine` splices two motion arrays into `[_; OUT_N]`, `servo_player::linear` generates a ramp, `with_joint_spheres` splices generated geometry into a step list. Same pattern covers the missing stdlib: const `max_u32`, `div_ceil_usize`, `min_usize` (tiling.rs), and const `equals` in place of `PartialEq`.
12. **Just use `while`, not `for` — and that's fine.** `for` needs the `Iterator` trait, which isn't const-callable on stable; every loop in these codebases is an index + `while`. It reads like C, it works, and a `TODO_NIGHTLY` comment convention marks each one for the day `const_for` stabilizes.

### Trimming to 9

Merge candidates: 1+3 (parse *and* validate at compile time), 5+6 (codecs and the fixed-point math they need), 10+11 (const generics + combine functions are two halves of "arrays as const data structures"). That gets 12 → 9 while keeping every trick.

### Overflow / sidebar candidates

- **Const eval is an interpreter — it's slow.** The 764 KB BVH ≈ 8 s compile-time anecdote; mitigation: quantize/pre-process, or move heavy lifting to xtask codegen that *emits* const-macro invocations (build.rs-free pipeline).
- **Other stable-const limitations** (beyond `while` vs `for`, now rule 12): no trait methods generally, limited float ops; mutable references now OK.
- **Typed units as const constructors:** `Volume::percent(50)`, `Volume::spinal_tap(11)`, `Gain::db(-6)` — validated at compile time when used in const.

## Audience validation (Reddit, 2026-07)

A Reddit post of the core idea — "Process external files in const fn: no build.rs, no proc macros, no binary bloat" — got 139 upvotes (unusually strong for this account). Takeaways for the article:

- The **hook that worked**: `include_bytes!` inside a `const fn`, a tiny self-contained `sum_u16s` example, and the three-negation framing "No build.rs. No proc macros. No runtime cost."
- The detail that landed: *if you never store the result, the file contributes zero bytes to the binary* — the file is pure compile-time input. Keep this line.
- The post is rules 1+2+3 compressed into ~30 lines. Structure implication: open the article with that same minimal example as the on-ramp, then present the 9 rules as "now that you've seen the trick, here's everything it unlocks in two real projects."
- The `while` loop appeared in the post without scaring anyone — supports keeping rule 12's "and that's fine" framing.
- Reuse the post's phrase "compile-time asset pipeline" — it's a better name than "asset compiler" for rule 2.

### What the comment thread taught us

The two *highest-voted comments outscored most of the post's own points* — both are things the article must cover:

- **`const { ... }` blocks (ZZaaaccc, 26 upvotes — top comment).** Two uses: (a) lift a snippet into compile-time evaluation inline, no named `const fn` or `const`/`static` needed; (b) wrap a const fn's body in `const { }` to *force* compile-time evaluation even when called at runtime. Neither repo uses const blocks yet — audit for places they'd simplify, and give them their own rule or a prominent sidebar.
- **Const float arithmetic is stable now (LETS_DISCUSS_MUSIC, 23 upvotes).** `+ - * /` on floats works in const; only transcendental functions are missing. Reframe rule 6: not "const math is fixed-point math" but "basic float math works — roll your own for the rest" (Newton's method for sqrt, Bhaskara for sine, `scale_pow10` for parsing). Carl's reply demoed f16 parsing + NaN filtering + normalization + Newton sqrt in stable const — playground: <https://play.rust-lang.org/?version=stable&mode=release&edition=2024&gist=56d046229610526ff50515eb39b58281>

New tricks surfaced by the thread:

- **Type-erase the length with promotion (Carl's own comment, worth a rule):** `static UPPER: &'static [u8] = &uppercase_ascii(include_bytes!("main.rs"));` — the `&` on a const expression in a `static` promotes the array to `'static`, so the static's type never names `N`. Same move puts mixed clip types behind `&'static dyn Playable` — this is how device-envoy stores PCM/ADPCM/silence clips under one API. Merges naturally with the Fixed/View rule (7). skullt's refinement: make the const fn take `&[u8; N]` and no wrapper macro is needed at all. Playground: <https://play.rust-lang.org/?version=stable&mode=release&edition=2024&gist=c7dcc61c035be9a1dd5f1d2c9243c949>
- **Two-pass reading:** const eval can afford to read the input twice — once to count (sizing the output array's `N`), once to process. This is exactly how `bvh_motion!` infers `DOF`/`SAMPLE_COUNT`; the f16 playground counts non-NaN values first. Name this pattern in the article.
- **Compile-time-infallible builders (JShelbyJ):** const setters that `panic!` on infeasible settings turn runtime errors into compile errors — independent confirmation of rule 3 from someone else's codebase; quote or cite it.
- **Prior art for credibility (newpavlov):** RustCrypto's `blobby` crate does const-fn transformation of a custom binary test-vector format into `&[&[u8]]` / struct slices, wrapped in declarative macros — same architecture as `pcm_clip!`. Cite it so the pattern reads as established, not a one-off hack.

Objections to preempt (the article's "when NOT to use this" section):

- **"build.rs is the right tool" (tm_p).** Three counters from the thread: build.rs has a bigger security surface (Hedanito) — const eval is sandboxed inside the compiler, it can't run arbitrary external tools; build.rs is overkill and more error-prone for simple transforms (tialaramex); and the ergonomic win — sample rate, volume, and compression chosen *at the use site* in main code, not hidden in a build script (Carl's `pcm_clip!`/`with_gain`/`with_adpcm` reply, which earned upvotes as a rebuttal). Concede the real limits: build.rs is right for heavy processing, caching, or calling external tools.
- **Why `while` not `for` (tialaramex):** `for` desugars to the `Iterator` trait and const trait impls aren't stable yet; nightly has it. Fold this precise explanation into rule 12 — it's better than what we had.
- **Reader confusion (PyAndorran):** it wasn't obvious the file is read at *build* time and can be absent at runtime. State the build-time/runtime boundary explicitly and early.

## Proposed article structure

1. Hook: open with the Reddit post's minimal `sum_u16s` example and the "No build.rs. No proc macros. No runtime cost." framing (validated — see "Audience validation" above). The RAM/flash motivation follows as the second beat.
2. One-paragraph tour of the two projects (robot-arm linkage on a CYD; device toolkit for RP2040/ESP32).
3. The 9 rules, each: motivation → short real snippet → what it buys (flash/RAM/startup/safety).
4. Costs and sharp edges (compile time, while-loops, stable vs nightly).
5. Close: links to both repos.

## Open work before drafting

- Apply the merges in "Trimming to 9" (or retitle to match the final count).
- Measure and cite real numbers: BVH compile time, ADPCM flash savings, gamma-table size, RAM saved by const framebuffers.
- Pick the 9 code snippets and shorten them to Medium width (~70 chars).
- Decide title. Candidates: "9 Rules for Rust `const fn` on Embedded", "Compile It to Flash: 9 const fn Rules from Two no-alloc Rust Projects".
