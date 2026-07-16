import init, { start } from "./pkg/linkage_blaze_clock_wasm.js";
import { mountCydSimulator } from "./cyd-simulator.js";

try {
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `clock` render loop, which paces itself via `requestAnimationFrame`
  // inside Rust and ticks once per second from the browser clock. CSS stretches
  // the canvas over the case's screen area, so no JS animation loop or sizing is
  // needed here.
  await mountCydSimulator({
    wasm: { init, start },
    app: {
      orientation: "landscape",
      galleryUrl: "../../",
    },
  });
} catch (error) {
  console.error(error);
  throw error;
}
