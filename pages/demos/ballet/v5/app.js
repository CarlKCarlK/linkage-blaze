import init, { start } from "./pkg/linkage_blaze_classic_wasm.js";
import { mountCydSimulator } from "./cyd-simulator.js";

try {
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `ballet` render loop, which paces itself via `requestAnimationFrame`
  // inside Rust. CSS stretches the canvas over the case's screen area, so no JS
  // animation loop or sizing is needed here.
  await mountCydSimulator({
    wasm: { init, start },
    app: {
      orientation: "portrait",
      galleryUrl: "../../",
    },
  });
} catch (error) {
  console.error(error);
  throw error;
}
