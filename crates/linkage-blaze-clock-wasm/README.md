# linkage-blaze-clock-wasm

Browser build of the "classic" CYD `clock` example: the same clock that runs
on the esp32 CYD, rendered onto an HTML canvas in landscape mode.

## Run

```sh
just run-clock-wasm
```

Then open <http://localhost:8084>. The recipe checks the crate, runs
`wasm-pack build`, and serves the `www/` directory. (On WSL2, if `localhost`
won't connect from Windows, use the WSL VM's IP or enable
`networkingMode=mirrored` in `.wslconfig`.)

## Design

This reuses the same pieces as [`linkage-blaze-skeleton-clock-wasm`](../linkage-blaze-skeleton-clock-wasm):
the device-agnostic `async fn clock` from
[`linkage-blaze-example-core`](../linkage-blaze-example-core) drawn onto the
[`CydWasm`](../linkage-blaze-cyd-wasm) device, whose `flush` awaits
`requestAnimationFrame`. The browser future is driven by
`wasm_bindgen_futures::spawn_local`.

Like the skeleton-clock WASM build, this crate adds only a tiny `ClockSync`
source, `WasmClockSync`: it reads wall-clock time from JavaScript's `Date` and
ticks once per second via an `embassy_time::Timer`. There is no NTP sync in the
browser; the operating system clock is already correct. The page also exposes
the same time-of-day slider override used by the skeleton-clock simulator.
