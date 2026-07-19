<!-- todo0 consider deleting this spec once the review is complete. -->

# Review file list

Clickable review checklist for the example/API and platform integration changes.

## Four Linkage Blaze examples

- [Armatron](../crates/linkage-blaze-core/src/examples/armatron/main.rs)
- [Ballet](../crates/linkage-blaze-core/src/examples/ballet.rs)
- [Clock](../crates/linkage-blaze-core/src/examples/clock.rs)
- [Skeleton Clock](../crates/linkage-blaze-core/src/examples/skeleton_clock.rs)

## Linkage Blaze platform callers

- [ESP32 Armatron template](../crates/linkage-blaze-examples-esp/examples/templates/armatron.rs.j2)
- [ESP32 one-SPI Armatron template](../crates/linkage-blaze-examples-esp/examples/templates/armatron_one_spi.rs.j2)
- [ESP32 Ballet template](../crates/linkage-blaze-examples-esp/examples/templates/ballet.rs.j2)
- [ESP32 Clock template](../crates/linkage-blaze-examples-esp/examples/templates/clock.rs.j2)
- [ESP32 Skeleton Clock template](../crates/linkage-blaze-examples-esp/examples/templates/skeleton_clock.rs.j2)
- [RP Pico examples](../crates/linkage-blaze-examples-rp/examples/)
- [WASM examples](../crates/linkage-blaze-examples-wasm/examples/)

Review these example templates:

- `armatron.rs` — two-SPI Armatron path
- `armatron_one_spi.rs` — one-SPI Armatron path
- `ballet.rs`
- `clock.rs`
- `skeleton_clock.rs`

## Device Envoy DNS Tester

- [DNS Tester core](../../mcu/device-envoy/crates/device-envoy-examples-core/src/dns_tester.rs)
- [DNS Tester RP examples](../../mcu/device-envoy/crates/device-envoy-examples-rp/examples/)
- [DNS Tester ESP template](../../mcu/device-envoy/crates/device-envoy-examples-esp/examples/templates/dns_tester.rs.j2)

## Pico and memory

- [Pico 1 W linker memory](../../mcu/device-envoy/crates/device-envoy-rp/memory-pico1w.x)
- [Pico 2 linker memory](../../mcu/device-envoy/crates/device-envoy-rp/memory-pico2.x)
- [Pico 2 RISC-V linker memory](../../mcu/device-envoy/crates/device-envoy-rp/memory-pico2-riscv.x)
- [Device Envoy memory module](../../mcu/device-envoy/crates/device-envoy-core/src/memory.rs)

## WASM support

- [WASM module](../../mcu/device-envoy/crates/device-envoy-core/src/wasm.rs)
- [WASM animation frame](../../mcu/device-envoy/crates/device-envoy-core/src/wasm/animation_frame.rs)
- [WASM clock](../../mcu/device-envoy/crates/device-envoy-core/src/wasm/clock.rs)
- [WASM CYD web integration](../../mcu/device-envoy/crates/device-envoy-core/src/wasm/cyd_web.rs)
- [WASM DNS support](../../mcu/device-envoy/crates/device-envoy-core/src/wasm/dns.rs)
- [WASM simulator](../../mcu/device-envoy/crates/device-envoy-core/src/wasm/simulator.rs)
- [Linkage Blaze WASM examples](../crates/linkage-blaze-examples-wasm/examples/)

## Armatron SPI review

- [ESP two-SPI template](../crates/linkage-blaze-examples-esp/examples/templates/armatron.rs.j2)
- [ESP one-SPI template](../crates/linkage-blaze-examples-esp/examples/templates/armatron_one_spi.rs.j2)
- [RP two-SPI example](../crates/linkage-blaze-examples-rp/examples/armatron.rs)
- [RP one-SPI example](../crates/linkage-blaze-examples-rp/examples/armatron_one_spi.rs)

Only review the Jinja templates; do not review generated board files.
