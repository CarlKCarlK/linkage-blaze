# linkage-blaze-skeleton-clock-wasm

Browser build of the "classic" CYD `skeleton_clock` example: the same analog
skeleton clock that runs on the esp32 CYD, rendered onto an HTML canvas.

## Run

```sh
just run-skeleton-clock-wasm
```

Then open <http://localhost:8086>. The recipe checks the crate, runs
`wasm-pack build`, and serves the `www/` directory. (On WSL2, if `localhost`
won't connect from Windows, use the WSL VM's IP or enable
`networkingMode=mirrored` in `.wslconfig`.)

## Design

This reuses the same pieces as the ballet app: the device-agnostic
`async fn skeleton_clock` from
[`linkage-blaze-example-core`](../linkage-blaze-example-core) (an unchanged
`loop { wait_for_tick().await; draw; flush_at(..).await?; }`) drawn onto the
[`CydWasm`](../linkage-blaze-cyd-wasm) device, whose `flush_at` awaits
`requestAnimationFrame`. The browser future is driven by
`wasm_bindgen_futures::spawn_local`.

The only skeleton-clock-specific addition is a tiny `ClockSync` source,
`WasmClockSync`: it reads wall-clock time from JavaScript's `Date` and ticks
once per second via an `embassy_time::Timer`. There is no NTP sync — the
operating system clock is already correct — so the NTP/WiFi machinery the esp32
build uses is simply not present here. `Instant::now()` and the `Timer` both run
on `embassy-time`'s built-in `wasm` feature (a `setTimeout`-backed driver),
enabled only in this final crate.

Because `skeleton_clock` pulls device-envoy-core's `ClockSync` (→
embassy-executor), this crate selects `embassy-executor`'s `platform-wasm`
feature so the executor's `__pender` symbol resolves; the executor itself is
never run (futures are driven by `spawn_local`).
