import init, { start } from "./pkg/linkage_blaze_armatron_wasm.js";

if ("serviceWorker" in navigator) {
  if (location.hostname === "localhost" || location.hostname === "127.0.0.1") {
    navigator.serviceWorker.getRegistrations().then((registrations) => {
      for (const reg of registrations) reg.unregister();
    });
  } else {
    navigator.serviceWorker.register("./sw.js");
  }
}

const { canvas } = ensureFramedLayout();
const context = canvas.getContext("2d");

try {
  await init();
  start("screen");
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

function ensureFramedLayout() {
  const canvas = document.querySelector("#screen");
  if (!(canvas instanceof HTMLCanvasElement)) {
    throw new Error("missing #screen canvas");
  }

  let stage = document.querySelector(".stage");
  if (!(stage instanceof HTMLDivElement)) {
    stage = document.createElement("div");
    stage.className = "stage";
    canvas.replaceWith(stage);
    stage.appendChild(canvas);
  }

  let cord = stage.querySelector(".cord");
  if (!(cord instanceof HTMLDivElement)) {
    cord = document.createElement("div");
    cord.className = "cord";
    stage.prepend(cord);
  }

  let caseImage = stage.querySelector(".case");
  if (!(caseImage instanceof HTMLImageElement)) {
    caseImage = document.createElement("img");
    caseImage.className = "case";
    caseImage.src = "./case.png";
    caseImage.alt = "CYD device case";
    stage.insertBefore(caseImage, canvas);
  }

  return { canvas };
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
