import init, {
  start,
  show_case_alignment_controls,
} from "./pkg/linkage_blaze_skeleton_clock_wasm.js";
import { mountCydSimulator } from "./cyd-simulator.js";

try {
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `skeleton_clock` render loop, which paces itself via
  // `requestAnimationFrame` inside Rust and ticks once per second from the
  // browser clock. CSS stretches the canvas over the case's screen area, so no
  // JS animation loop or sizing is needed here.
  await mountCydSimulator({
    wasm: { init, start },
    app: {
      orientation: "portrait",
      galleryUrl: "../../",
    },
  });
  if (show_case_alignment_controls()) {
    await import("./controls.js");
  }
} catch (error) {
  console.error(error);
  throw error;
}
