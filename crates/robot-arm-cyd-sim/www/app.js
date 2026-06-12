import init, { CydSim } from "./pkg/robot_arm_cyd_sim_v3.js?v=static-report-1";

const canvas = document.querySelector("#screen");
const context = canvas.getContext("2d");

await init();

const sim = new CydSim();
const image = context.createImageData(sim.width(), sim.height());
let animationFrame = null;
const ANIMATION_INTERVAL_MS = 1000 / 11;
let lastFrameTime = 0;

render();

canvas.addEventListener("pointerdown", (event) => {
  canvas.setPointerCapture(event.pointerId);
  const point = eventToScreen(event);
  sim.touch_down(point.x, point.y);
  render();
  scheduleReverseKinematics();
});

canvas.addEventListener("pointermove", (event) => {
  if (!(event.buttons & 1)) {
    return;
  }
  const point = eventToScreen(event);
  sim.touch_move(point.x, point.y);
  render();
  scheduleReverseKinematics();
});

canvas.addEventListener("pointerup", (event) => {
  canvas.releasePointerCapture(event.pointerId);
  sim.touch_up();
  render();
  scheduleReverseKinematics();
});

canvas.addEventListener("pointercancel", () => {
  sim.touch_up();
  render();
  scheduleReverseKinematics();
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

function scheduleReverseKinematics() {
  if (animationFrame !== null) {
    return;
  }
  animationFrame = requestAnimationFrame(tickReverseKinematics);
}

function tickReverseKinematics(timestamp) {
  animationFrame = null;

  if (timestamp - lastFrameTime < ANIMATION_INTERVAL_MS) {
    scheduleReverseKinematics();
    return;
  }
  lastFrameTime = timestamp;

  const running = sim.tick_reverse_kinematics_at(Math.round(timestamp * 1000));
  render();
  if (running) {
    scheduleReverseKinematics();
  }
}
