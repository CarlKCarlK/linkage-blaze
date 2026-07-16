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
      orientation: "landscape",
      galleryUrl: "../../",
      // A real touchscreen samples continuously while held; synthetic browser
      // pointer input uses a few samples so held-touch behavior remains stable.
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
