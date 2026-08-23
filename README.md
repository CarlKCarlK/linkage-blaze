# linkage-blaze

[![GitHub](https://img.shields.io/badge/github-linkage--blaze-8da0cb?style=flat&labelColor=555555&logo=github)](https://github.com/CarlKCarlK/linkage-blaze)

3D turtle graphics for animated joints. Describe a figure with moves, turns, branches, links, joints, disks, and spheres. Then animate parameters to bring it to life.

## Project Links

- **[Live demo gallery](https://carlkcarlk.github.io/linkage-blaze/demos/)** - Preview and run the interactive browser demos.
- **[GitHub repository](https://github.com/CarlKCarlK/linkage-blaze)** - Source, issues, and development history.

### Rust Crate

**`linkage-blaze`** is the single published crate. Its default build is
allocation-free and does not require the Rust standard library. Enable `alloc`
for runtime-owned programs and `bvh` for host-side motion-capture conversion.
[![crates.io](https://img.shields.io/crates/v/linkage-blaze?style=flat&color=fc8d62&logo=rust)](https://crates.io/crates/linkage-blaze)
[![docs.rs](https://img.shields.io/docsrs/linkage-blaze?style=flat&color=66c2a5&labelColor=555555)](https://docs.rs/linkage-blaze)

```toml
[dependencies]
linkage-blaze = "0.1.5"
```

Use `linkage_blaze::{LinkageFixed, linkage_fixed}` for allocation-free ESP and
RP applications. Add `features = ["alloc"]` for owned parsing and
`features = ["bvh"]` for host-side BVH APIs. Install the converter with:

```bash
cargo install linkage-blaze --features bvh --bin bvh-to-lb
```

### Platform Examples

- **[Raspberry Pi Pico / RP](https://github.com/CarlKCarlK/linkage-blaze/tree/main/crates/linkage-blaze-examples-rp)** - Pico 1 and Pico 2 examples, including Pico W variants.
- **[ESP32](https://github.com/CarlKCarlK/linkage-blaze/tree/main/crates/linkage-blaze-examples-esp)** - CYD-oriented examples across supported ESP32 families and boards.
- **[Browser / WASM](https://github.com/CarlKCarlK/linkage-blaze/tree/main/crates/linkage-blaze-examples-wasm)** - Browser builds behind the live gallery.

The platform examples and browser adapter use the workspace version but are not
separately published to crates.io. WASM applications provide their own
`cdylib` and may depend on `linkage-blaze` with the `alloc` feature.

## What is Linkage Blaze?

Linkage Blaze is a small language for making animated jointed drawings. It works like 3D turtle graphics: move forward, turn, branch, draw links, place joints, and add simple shapes such as disks and spheres. Animate a few parameters, and the drawing moves.

The demos use this to make clocks, skeletons, dancers, and robot-arm-like figures. Everything is a Rust workspace that renders on microcontrollers (e.g. the CYD / ESP32 display boards) and in the browser via WASM.

The core is `no_std` and allocation-free, so figures live in flash and animate on small microcontrollers. An opt-in `alloc` feature adds heap-based conveniences where an allocator is available.

## Gallery

The live gallery is the main showcase: **[carlkcarlk.github.io/linkage-blaze/demos/](https://carlkcarlk.github.io/linkage-blaze/demos/)**

It shows preview images of each demo and links to the live, interactive WASM versions.

<p>
  <img src="https://raw.githubusercontent.com/CarlKCarlK/linkage-blaze/main/crates/linkage-blaze/tests/assets/armatron.png" alt="Armatron demo preview" width="200" />
  <img src="https://raw.githubusercontent.com/CarlKCarlK/linkage-blaze/main/crates/linkage-blaze/tests/assets/ballet.png" alt="Ballet demo preview" width="150" />
</p>

## Example Linkage

This is the `armatron1.lb.rs` [(interactive editor)](https://carlkcarlk.github.io/linkage-blaze/demos/editor/v2/#armatron) linkage based on a toy robot arm. It defines six parameters for the shoulder, elbow, and hand, then builds a simple robot-arm-like figure with a wrist mark so the claw can branch into two fingers.

```rust,no_run
linkage![
    .define_param("raise hand", 0.5)
    .define_param("bend elbow", 0.5)
    .define_param("close hand", 0.0)
    .define_param("lower arm", 0.5)
    .define_param("spin whole arm", 0.5)
    .define_param("spin hand", 0.5)
    .yaw_param("spin whole arm", 180.0, -180.0)
    .pen_color(Rgb888::new(0, 139, 139)) // dark cyan (0, 139, 139)
    .pen_width(0.15)
    .up(2.5)
    .pitch_param("lower arm", -30.0, 0.0)
    .forward(3.0)
    .yaw_param("bend elbow", 90.0, -90.0)
    .forward(3.0)
    .pitch_param("raise hand", 90.0, -90.0)
    .forward(1.0)
    .roll_param("spin hand", -180.0, 180.0)
    .forward(0.5)
    .mark("wrist")
    .yaw(90.0)
    .forward_param("close hand", 0.5, 0.0)
    .left(-1.0)
    .restore("wrist")
    .yaw(-90.0)
    .forward_param("close hand", 0.5, 0.0)
    .left(1.0)
    .restore("wrist")
    .pen_up()
    .forward(0.25)
    .pen_down()
]
```

The linkage compiles to a `const` with no heap allocation and no runtime parsing, so it can live in flash on a microcontroller.

## Status

⚠️ **Alpha / Experimental**

The API is actively evolving. Not recommended for production use, but good for experimentation and learning.

## Policy on AI-assisted development and contributions

The use of AI tools is permitted for development and contributions to this repository. AI may be used as a productivity aid for drafting, exploration, and refactoring.

All code and documentation contributed to this repository must be reviewed, edited, and validated by a human contributor. AI tools are not a substitute for design judgment, testing, or responsibility for correctness.

[AGENTS.md](AGENTS.md) contains the general instructions and constraints given to AI tools used during development of this repository.

## License

Licensed under either:

- MIT license (see LICENSE-MIT)
- Apache License, Version 2.0 (see LICENSE-APACHE)

at your option.
