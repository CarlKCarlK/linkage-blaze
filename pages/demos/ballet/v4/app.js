import init, {
  start,
  show_case_alignment_controls,
} from "./pkg/linkage_blaze_classic_wasm.js";
import { mountCydSimulator } from "./cyd-simulator.js";

try {
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `ballet` render loop, which paces itself via `requestAnimationFrame`
  // inside Rust. CSS stretches the canvas over the case's screen area, so no JS
  // animation loop or sizing is needed here.
  await mountCydSimulator({
    wasm: { init, start },
    app: {
      title: "Ballet",
      orientation: "portrait",
      galleryUrl: "../../",
      previewLine: "A motion-captured pirouette replayed as a linkage skeleton.",
      descriptionHtml:
        "<p>A motion-captured pirouette: a BVH recording converted into a linkage skeleton and replayed full screen. The top line shows the frame counter, frames per second, and the slow-motion factor.</p>",
      controlsHtml:
        "<p>Sit back and watch.</p>",
      coreCodeUrl:
        "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze-example-core/src/ballet.rs",
    },
  });
  if (show_case_alignment_controls()) {
    await import("./controls.js");
  }
} catch (error) {
  console.error(error);
  throw error;
}
