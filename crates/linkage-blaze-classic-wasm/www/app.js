import init, { start } from "./pkg/linkage_blaze_classic_wasm.js";

const status = document.querySelector("#status");

// On-screen scale factor. The Rust side sets the canvas pixel buffer to the real
// panel resolution; this only stretches the displayed (CSS) size.
const SCALE = 2;

try {
  await init();
  // `start` sizes the canvas pixel buffer and spawns the `ballet` render loop,
  // which paces itself via `requestAnimationFrame` inside Rust. No JS animation
  // loop needed.
  const canvas = document.querySelector("#screen");
  start("screen");
  canvas.style.width = `${canvas.width * SCALE}px`;
  canvas.style.height = `${canvas.height * SCALE}px`;
  status.textContent = "ballet running";
} catch (error) {
  status.textContent = `load failed: ${String(error)}`;
  throw error;
}
