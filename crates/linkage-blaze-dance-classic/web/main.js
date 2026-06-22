import init, { DanceClockSim } from "./pkg/linkage_blaze_dance_classic.js";

const canvas = document.querySelector("#screen");
const context = canvas.getContext("2d");
const status = document.querySelector("#status");

try {
  await init();
} catch (error) {
  status.textContent = `load failed: ${String(error)}`;
  throw error;
}

const sim = new DanceClockSim();
canvas.width = sim.width();
canvas.height = sim.height();
const image = context.createImageData(sim.width(), sim.height());
let lastSecond = -1;

status.textContent = "";
requestAnimationFrame(tick);

function tick() {
  const now = new Date();
  if (now.getSeconds() !== lastSecond) {
    lastSecond = now.getSeconds();
    sim.renderTime(now.getHours(), now.getMinutes(), now.getSeconds());
    image.data.set(sim.rgba());
    context.putImageData(image, 0, 0);
  }
  requestAnimationFrame(tick);
}
