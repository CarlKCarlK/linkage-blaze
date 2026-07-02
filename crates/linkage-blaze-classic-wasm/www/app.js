import init, {
  start,
  show_case_alignment_controls,
} from "./pkg/linkage_blaze_classic_wasm.js";

trackBrowserZoom();

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
} catch (error) {
  console.error(error);
  throw error;
}

function trackBrowserZoom() {
  const initialDevicePixelRatio = window.devicePixelRatio || 1;

  const updateBrowserZoom = () => {
    const browserZoom = (window.devicePixelRatio || 1) / initialDevicePixelRatio;
    document.documentElement.style.setProperty("--browser-zoom", String(browserZoom));
  };

  updateBrowserZoom();
  window.addEventListener("resize", updateBrowserZoom);
  window.visualViewport?.addEventListener("resize", updateBrowserZoom);
}
