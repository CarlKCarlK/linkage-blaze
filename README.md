# linkage-blaze

[![GitHub](https://img.shields.io/badge/github-linkage--blaze-8da0cb?style=flat&labelColor=555555&logo=github)](https://github.com/CarlKCarlK/linkage-blaze)

3D turtle graphics for animated joints. Describe a figure with moves, turns, branches, links, joints, disks, and spheres. Then animate parameters to bring it to life.

## What is Linkage Blaze?

Linkage Blaze is a small language for making animated jointed drawings. It works like 3D turtle graphics: move forward, turn, branch, draw links, place joints, and add simple shapes such as disks and spheres. Animate a few parameters, and the drawing moves.

The demos use this to make clocks, skeletons, dancers, and robot-arm-like figures. Everything is a Rust workspace that renders on microcontrollers (e.g. the CYD / ESP32 display boards) and in the browser via WASM.

The core is `no_std` and allocation-free, so figures live in flash and animate on small microcontrollers. An opt-in `alloc` feature adds heap-based conveniences where an allocator is available.

## Gallery

The live gallery is the main showcase: **[carlkcarlk.github.io/linkage-blaze/demos/](https://carlkcarlk.github.io/linkage-blaze/demos/)**

It shows preview images of each demo and links to the live, interactive WASM versions.

<p>
  <img src="https://raw.githubusercontent.com/CarlKCarlK/linkage-blaze/main/crates/linkage-blaze-example-core/tests/assets/armatron.png" alt="Armatron demo preview" width="200" />
  <img src="https://raw.githubusercontent.com/CarlKCarlK/linkage-blaze/main/crates/linkage-blaze-example-core/tests/assets/ballet.png" alt="Ballet demo preview" width="150" />
</p>

## Example

This is the clock demo's linkage, abridged. It draws a clock face and three hands as turtle-graphics steps. The two parameters (`hour` and `face spin`) are recomputed each tick, and the hands move.

```rust,no_run
linkage![
    .define_param("hour", 0.0)
    .define_param("face spin", 0.5)
    // Common transform for the whole clock face.
    .roll_param("face spin", -90.0, 90.0)
    .mark("face")
    // Face disk
    .pen_color(Rgb888::new(24, 62, 118)) // desaturated deep blue (24, 62, 118)
    .disk(65.0)
    // 12 o'clock tick
    .restore("face")
    .pen_color(Rgb888::new(230, 195, 115)) // muted pale gold (230, 195, 115)
    .pen_up()
    .forward(45.0)
    .pen_down()
    .forward(18.0)
    // ... 3, 6, and 9 o'clock ticks elided ...
    // Hour hand
    .restore("face")
    .pen_color(Rgb888::new(245, 220, 165)) // warm brass ivory (245, 220, 165)
    .pen_width(10.5)
    .yaw_param("hour", 360.0, 0.0)
    .forward(40.0)
    // Minute hand
    .restore("face")
    .pen_color(Rgb888::new(96, 205, 220)) // softened blue-green (96, 205, 220)
    .pen_width(6.0)
    .yaw_param("hour", 4320.0, 0.0)
    .forward(52.0)
    // Second hand
    .restore("face")
    .pen_color(Rgb888::new(230, 95, 70)) // muted coral red (230, 95, 70)
    .pen_width(2.0)
    .yaw_param("hour", 259_200.0, 0.0)
    .forward(60.0)
]
```

The full linkage compiles to a `const` — no heap, no runtime parsing — so it lives in flash on the microcontroller. See the complete version in [clock.lb.rs](crates/linkage-blaze-example-core/src/clock.lb.rs).

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
