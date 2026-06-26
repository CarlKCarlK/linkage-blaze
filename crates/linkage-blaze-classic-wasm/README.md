# linkage-blaze-classic-wasm

Browser build of the "classic" CYD `ballet` example: the same motion-captured
pirouette that runs on the esp32 CYD, rendered onto an HTML canvas.

## Run

```sh
just run-ballet-wasm
```

Then open <http://localhost:8085>. The recipe checks the crate, runs
`wasm-pack build`, and serves the `www/` directory. (On WSL2, if `localhost`
won't connect from Windows, use the WSL VM's IP or enable
`networkingMode=mirrored` in `.wslconfig`.)

## The async infinite-loop design

The render logic in [`linkage-blaze-example-core`](../linkage-blaze-example-core)
is a single, device-agnostic `async fn ballet` written as a plain
`loop { for sample { draw; cyd_frame.flush_at(..).await?; } }`. It is **not**
inverted into a manual `tick()` state machine: `flush_at(..).await` is the frame
boundary, and the `async` compiler lowers the loop into the state machine for us.
On the esp32 the flush completes synchronously (and could `yield_now().await` to
share the executor); in the browser it awaits the next `requestAnimationFrame`,
blits the frame to the canvas, then resolves — so the identical loop paces itself
to each platform's natural present point. The browser future is driven by
`wasm_bindgen_futures::spawn_local`; the `requestAnimationFrame` wrapper
([`linkage-blaze-cyd-wasm`](../linkage-blaze-cyd-wasm)) owns its closure (the
closure captures only an `Rc` to shared state, never the reverse) so there is no
reference cycle and no per-frame leak.

## The WASM time driver

`ballet` measures per-frame duration with `embassy_time::Instant::now()`, which
needs a registered time driver. Rather than hand-roll one, this crate enables
`embassy-time`'s built-in `wasm` feature (a `setTimeout`-backed driver with full
timer-queue support via `generic-queue-64`). It is enabled **only here, in the
final WASM crate** — embassy time drivers are global and there must be exactly
one, so the esp32 build keeps using its HAL-provided driver and never compiles
this crate. Using the real driver (instead of a now-only stub) means future uses
of embassy `Timer`s on WASM will work rather than silently trap.
