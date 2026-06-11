import init, { CydSim } from "./pkg/robot_arm_cyd_sim.js?v=play-step-2";

const canvas = document.querySelector("#screen");
const context = canvas.getContext("2d");

await init();

const sim = new CydSim();
const image = context.createImageData(sim.width(), sim.height());
let animationFrame = null;
let previousAnimationTimestamp = null;

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
  if (animationFrame !== null || !sim.is_reverse_kinematics_running()) {
    return;
  }
  animationFrame = requestAnimationFrame(tickReverseKinematics);
}

function tickReverseKinematics(timestamp) {
  animationFrame = null;
  const dtSeconds =
    previousAnimationTimestamp === null
      ? 1 / 60
      : (timestamp - previousAnimationTimestamp) / 1000;
  previousAnimationTimestamp = timestamp;

  const running = sim.tick_reverse_kinematics(dtSeconds);
  render();
  if (running) {
    scheduleReverseKinematics();
  } else {
    previousAnimationTimestamp = null;
  }
}
