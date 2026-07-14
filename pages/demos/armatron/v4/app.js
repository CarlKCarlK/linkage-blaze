import init, { start } from "./pkg/linkage_blaze_armatron_wasm.js";
import { mountCydSimulator } from "./cyd-simulator.js";

if ("serviceWorker" in navigator) {
  if (location.hostname === "localhost" || location.hostname === "127.0.0.1") {
    navigator.serviceWorker.getRegistrations().then((registrations) => {
      for (const reg of registrations) reg.unregister();
    });
  } else {
    navigator.serviceWorker.register("./sw.js");
  }
}

const canvas = document.querySelector("#screen");
if (!(canvas instanceof HTMLCanvasElement)) {
  throw new Error("missing #screen canvas");
}
const context = canvas.getContext("2d");

try {
  await mountCydSimulator({
    wasm: { init, start },
    app: {
      title: "Armatron",
      orientation: "landscape",
      galleryUrl: "../../",
      previewLine: "A six-joint robot arm driven by inverse kinematics.",
      descriptionHtml:
        "<p>A robot arm with six joints, modeled as a linkage and driven by inverse kinematics. The solver steers the claw toward the red target dot on the grid; you can also pose every joint yourself.</p>",
      controlsHtml:
        "<p>Drag any yellow-dot slider: <strong>raise hand</strong>, <strong>bend elbow</strong>, <strong>close hand</strong>, <strong>lower arm</strong>, <strong>spin whole arm</strong>, <strong>spin hand</strong> pose the arm; <strong>z zoom</strong> and <strong>x/y view</strong> move the camera. Press <strong>\u25b6</strong> to let the solver walk the claw to the target, <strong>\u25b6|</strong> to single-step it, and <strong>prev / next</strong> to pick a target. <strong>cal</strong> (or the physical BOOT button) re-runs the same touch calibration exercise used on real hardware.</p>",
      coreCodeUrl:
        "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze-example-core/src/armatron/main.rs",
      // A real touchscreen samples continuously while held; a browser
      // pointerdown only reports one position. The shared calibration flow
      // discards its first few samples as settling noise and then averages
      // at least a few more (see MIN_SAMPLES_PER_POINT in device-envoy-core),
      // so a single tap needs several synthetic samples or a tap is
      // silently dropped and calibration never advances. Matches DNS
      // Tester's touchDownSamples.
      touchDownSamples: 9,
    },
  });
  maybeAutoBootPreview();
} catch (e) {
  console.error(e);
  context.fillStyle = "#111418";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = "#ff4444";
  context.font = "12px monospace";
  context.fillText("Load failed:", 8, 20);
  context.fillText(String(e), 8, 38);
  throw e;
}

function maybeAutoBootPreview() {
  const searchParams = new URLSearchParams(window.location.search);
  if (searchParams.get("preview") !== "1") {
    return;
  }

  const bootButton = document.querySelector("#boot-button");
  if (!(bootButton instanceof HTMLElement)) {
    return;
  }

  const dispatchPointer = (type) => {
    bootButton.dispatchEvent(
      new PointerEvent(type, {
        bubbles: true,
        cancelable: true,
        composed: true,
        pointerId: 1,
        pointerType: "mouse",
        isPrimary: true,
      }),
    );
  };

  window.setTimeout(() => {
    dispatchPointer("pointerdown");
    window.setTimeout(() => {
      dispatchPointer("pointerup");
    }, 120);
  }, 220);
}
