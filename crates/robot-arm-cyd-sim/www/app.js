import init, { CydSim } from "./pkg/robot_arm_cyd_sim_v3.js?v=shared-fps-3";

const canvas = document.querySelector("#screen");
const context = canvas.getContext("2d");

await init();

const sim = new CydSim();
const image = context.createImageData(sim.width(), sim.height());
let animationFrame = null;
let previousAnimationTimestamp = null;
const FPS_LIMIT = 11;
const FRAME_INTERVAL_MS = 1000 / FPS_LIMIT;
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

  // Throttle to 11 fps
  if (timestamp - lastFrameTime < FRAME_INTERVAL_MS) {
    scheduleReverseKinematics();
    return;
  }
  lastFrameTime = timestamp;

  const dtSeconds =
    previousAnimationTimestamp === null
      ? 1 / FPS_LIMIT
      : (timestamp - previousAnimationTimestamp) / 1000;
  previousAnimationTimestamp = timestamp;

  sim.set_frame_dt_seconds(dtSeconds);
  const running = sim.tick_reverse_kinematics(dtSeconds);
  render();
  if (running) {
    scheduleReverseKinematics();
  } else {
    previousAnimationTimestamp = null;
  }
}
