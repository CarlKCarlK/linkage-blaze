import init, { CydSim } from "./pkg/robot_arm_cyd_sim.js?v=xy-start-30deg-1";

const canvas = document.querySelector("#screen");
const context = canvas.getContext("2d");

await init();

const sim = new CydSim();
const image = context.createImageData(sim.width(), sim.height());

render();

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
