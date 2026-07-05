import init, {
  start,
  set_time_of_day,
  show_case_alignment_controls,
} from "./pkg/linkage_blaze_clock_wasm.js";
import { setupDemoUx } from "./demo-ux.js";

try {
  await init();
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `clock` render loop, which paces itself via `requestAnimationFrame`
  // inside Rust and ticks once per second from the browser clock. CSS stretches
  // the canvas over the case's screen area, so no JS animation loop or sizing is
  // needed here.
  start("screen");
  setupDemoUx({
    title: "Clock",
    orientation: "landscape",
    previewLine: "An analog linkage clock with a digital strip and WiFi status.",
    descriptionHtml:
      "<p>An analog clock whose hands are a tiny linkage posed by the time of day, drawn with embedded-graphics over a WiFi-status and digital-time readout. It free-runs at the panel's native 320x240.</p>",
    controlsHtml:
      "<p>It follows your local clock by default. Open the time setter to scrub to any time of day. Press <strong>Live</strong> to return to real time.</p>",
    coreCodeUrl:
      "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze-example-core/src/clock.rs",
    timeSetter: { setTimeOfDay: set_time_of_day },
  });
  if (show_case_alignment_controls()) {
    await import("./controls.js");
  }
} catch (error) {
  console.error(error);
  throw error;
}
