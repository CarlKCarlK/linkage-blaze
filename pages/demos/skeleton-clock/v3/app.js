import init, {
  start,
  set_time_of_day,
  show_case_alignment_controls,
} from "./pkg/linkage_blaze_skeleton_clock_wasm.js";
import { setupDemoUx } from "./demo-ux.js";

try {
  await init();
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `skeleton_clock` render loop, which paces itself via
  // `requestAnimationFrame` inside Rust and ticks once per second from the
  // browser clock. CSS stretches the canvas over the case's screen area, so no
  // JS animation loop or sizing is needed here.
  start("screen");
  setupDemoUx({
    title: "Skeleton Clock",
    orientation: "portrait",
    previewLine: "A motion-captured figure holds the hour and minute on placards.",
    descriptionHtml:
      "<p>A clock told by a motion-captured figure: placards hanging from its hands show the hour and minute, and the figure shifts its pose as time passes.</p>",
    controlsHtml:
      "<p>It follows your local clock by default. Open the time setter to scrub to any time of day and watch the figure re-pose. Press <strong>Live</strong> to return to real time.</p>",
    coreCodeUrl:
      "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze-example-core/src/skeleton_clock.rs",
    timeSetter: { setTimeOfDay: set_time_of_day },
  });
  if (show_case_alignment_controls()) {
    await import("./controls.js");
  }
} catch (error) {
  console.error(error);
  throw error;
}
