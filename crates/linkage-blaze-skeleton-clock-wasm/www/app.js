import init, { start } from "./pkg/linkage_blaze_skeleton_clock_wasm.js";

const status = document.querySelector("#status");

try {
  await init();
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `skeleton_clock` render loop, which paces itself via
  // `requestAnimationFrame` inside Rust and ticks once per second from the
  // browser clock. CSS stretches the canvas over the case's screen area, so no
  // JS animation loop or sizing is needed here.
  start("screen");
  status.textContent = "skeleton-clock running";
} catch (error) {
  status.textContent = `load failed: ${String(error)}`;
  throw error;
}
