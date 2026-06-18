import init, { CydSim } from "./pkg/linkage_blaze_armatron_wasm.js";


const canvas = document.querySelector("#screen");
const context = canvas.getContext("2d");

try {
  await init();
} catch (e) {
  context.fillStyle = "#111418";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = "#ff4444";
  context.font = "12px monospace";
  context.fillText("Load failed:", 8, 20);
  context.fillText(String(e), 8, 38);
  throw e;
}

const sim = new CydSim();
const image = context.createImageData(sim.width(), sim.height());
let animationFrame = null;

render();
scheduleFrame();

canvas.addEventListener("pointerdown", (event) => {
  canvas.setPointerCapture(event.pointerId);
  const point = eventToScreen(event);
  sim.touch_down(point.x, point.y);
  render();
});

canvas.addEventListener("pointermove", (event) => {
  if (!(event.buttons & 1)) {
    return;
  }
  const point = eventToScreen(event);
  sim.touch_move(point.x, point.y);
  render();
});

canvas.addEventListener("pointerup", (event) => {
  canvas.releasePointerCapture(event.pointerId);
  sim.touch_up();
  render();
});

canvas.addEventListener("pointercancel", () => {
  sim.touch_up();
  render();
});

function render() {
  image.data.set(sim.rgba());
  context.putImageData(image, 0, 0);
}

function eventToScreen(event) {
  const bounds = canvas.getBoundingClientRect();
  return {
    x: ((event.clientX - bounds.left) * canvas.width) / bounds.width,
    y: ((event.clientY - bounds.top) * canvas.height) / bounds.height,
  };
}

const fullscreenBtn = document.querySelector("#fullscreen-btn");
fullscreenBtn.addEventListener("click", () => {
  if (document.fullscreenElement) {
    document.exitFullscreen();
  } else {
    document.documentElement.requestFullscreen();
  }
});
document.addEventListener("fullscreenchange", () => {
  fullscreenBtn.textContent = document.fullscreenElement ? "✕" : "⛶";
});

function scheduleFrame() {
  if (animationFrame !== null) {
    return;
  }
  animationFrame = requestAnimationFrame(tickFrame);
}

function tickFrame(timestamp) {
  animationFrame = null;

  if (sim.tick_at(Math.round(timestamp * 1000))) {
    render();
  }
  scheduleFrame();
}
