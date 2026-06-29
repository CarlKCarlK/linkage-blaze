import init, {
  start,
  show_case_alignment_controls,
} from "./pkg/linkage_blaze_classic_wasm.js";

const status = document.querySelector("#status");

try {
  await init();
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `ballet` render loop, which paces itself via `requestAnimationFrame`
  // inside Rust. CSS stretches the canvas over the case's screen area, so no JS
  // animation loop or sizing is needed here.
  start("screen");
  if (show_case_alignment_controls()) {
    await import("./controls.js");
  }
  status.textContent = "ballet running";
} catch (error) {
  status.textContent = `load failed: ${String(error)}`;
  throw error;
}
