import init, { linkage0_points } from "./pkg/robot_arm_wasm.js";

const PARAMS = [
  { name: "lower hand", value: 0.7514501463 },
  { name: "bend elbow", value: 0.5002003842 },
  { name: "close hand", value: 0.5 },
  { name: "lower arm", value: 1.0 },
  { name: "spin whole arm", value: 0.6254387123 },
  { name: "spin hand", value: 0.0 },
];

const DEFAULT_PARAMS = PARAMS.map((param) => param.value);
const canvas = document.querySelector("#arm-canvas");
const context = canvas.getContext("2d");
const sliders = document.querySelector("#sliders");
const resetView = document.querySelector("#reset-view");
const resetParams = document.querySelector("#reset-params");

let selectedIndex = 0;
let points = [];
let view = {
  scale: 28,
  offsetX: 0,
  offsetY: 0,
};
let dragging = false;
let lastPointer = { x: 0, y: 0 };

await init();
buildControls();
resize();
update();

window.addEventListener("resize", () => {
  resize();
  draw();
});

resetView.addEventListener("click", () => {
  fitView();
  draw();
});

resetParams.addEventListener("click", () => {
  for (let index = 0; index < PARAMS.length; index += 1) {
    PARAMS[index].value = DEFAULT_PARAMS[index];
  }
  syncControls();
  update();
});

canvas.addEventListener("pointerdown", (event) => {
  canvas.setPointerCapture(event.pointerId);
  dragging = true;
  canvas.classList.add("dragging");
  lastPointer = { x: event.clientX, y: event.clientY };
});

canvas.addEventListener("pointermove", (event) => {
  if (!dragging) {
    return;
  }
  const dx = event.clientX - lastPointer.x;
  const dy = event.clientY - lastPointer.y;
  view.offsetX += dx;
  view.offsetY += dy;
  lastPointer = { x: event.clientX, y: event.clientY };
  draw();
});

canvas.addEventListener("pointerup", (event) => {
  canvas.releasePointerCapture(event.pointerId);
  dragging = false;
  canvas.classList.remove("dragging");
});

canvas.addEventListener("pointercancel", () => {
  dragging = false;
  canvas.classList.remove("dragging");
});

canvas.addEventListener(
  "wheel",
  (event) => {
    event.preventDefault();
    const before = screenToWorld(event.offsetX, event.offsetY);
    const zoom = Math.exp(-event.deltaY * 0.001);
    view.scale = clamp(view.scale * zoom, 8, 220);
    const after = screenToWorld(event.offsetX, event.offsetY);
    view.offsetX += (after.x - before.x) * view.scale;
    view.offsetY -= (after.y - before.y) * view.scale;
    draw();
  },
  { passive: false },
);

canvas.addEventListener("dblclick", () => {
  fitView();
  draw();
});

window.addEventListener("keydown", (event) => {
  const sliderTag = document.activeElement?.tagName === "INPUT";
  if (!sliderTag && event.key >= "1" && event.key <= "6") {
    selectedIndex = Number(event.key) - 1;
    focusSelectedSlider();
    return;
  }
  if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
    return;
  }
  event.preventDefault();
  const direction = event.key === "ArrowRight" ? 1 : -1;
  const step = event.shiftKey ? 0.05 : 0.01;
  PARAMS[selectedIndex].value = clamp(PARAMS[selectedIndex].value + direction * step, 0, 1);
  syncControls();
  update();
});

function buildControls() {
  for (let index = 0; index < PARAMS.length; index += 1) {
    const param = PARAMS[index];
    const label = document.createElement("label");
    label.className = "slider";

    const header = document.createElement("span");
    header.className = "slider-header";

    const name = document.createElement("span");
    name.className = "slider-name";
    name.textContent = param.name;

    const value = document.createElement("span");
    value.className = "slider-value";
    value.dataset.valueFor = String(index);

    const input = document.createElement("input");
    input.type = "range";
    input.min = "0";
    input.max = "1";
    input.step = "0.001";
    input.value = String(param.value);
    input.dataset.paramIndex = String(index);

    input.addEventListener("input", () => {
      selectedIndex = index;
      param.value = Number(input.value);
      syncControls();
      update();
    });
    input.addEventListener("focus", () => {
      selectedIndex = index;
    });

    header.append(name, value);
    label.append(header, input);
    sliders.append(label);
  }

  const hint = document.createElement("p");
  hint.className = "hint";
  hint.textContent =
    "Drag to pan. Mouse wheel zooms. Double click resets view. Number keys select a parameter; arrow keys nudge it.";
  sliders.append(hint);
  syncControls();
}

function syncControls() {
  for (let index = 0; index < PARAMS.length; index += 1) {
    const param = PARAMS[index];
    const input = sliders.querySelector(`[data-param-index="${index}"]`);
    const value = sliders.querySelector(`[data-value-for="${index}"]`);
    input.value = String(param.value);
    value.textContent = param.value.toFixed(3);
  }
}

function focusSelectedSlider() {
  sliders.querySelector(`[data-param-index="${selectedIndex}"]`)?.focus();
}

function update() {
  points = unpackPoints(linkage0_points(PARAMS.map((param) => param.value)));
  if (view.offsetX === 0 && view.offsetY === 0) {
    fitView();
  }
  draw();
}

function unpackPoints(flatPoints) {
  const nextPoints = [];
  for (let index = 0; index < flatPoints.length; index += 3) {
    nextPoints.push({
      x: flatPoints[index],
      y: flatPoints[index + 1],
      z: flatPoints[index + 2],
    });
  }
  return nextPoints;
}

function resize() {
  const bounds = canvas.getBoundingClientRect();
  const pixelRatio = window.devicePixelRatio || 1;
  canvas.width = Math.max(1, Math.round(bounds.width * pixelRatio));
  canvas.height = Math.max(1, Math.round(bounds.height * pixelRatio));
  context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
}

function fitView() {
  if (points.length === 0) {
    return;
  }
  const bounds = points.reduce(
    (accumulator, point) => ({
      minX: Math.min(accumulator.minX, point.x),
      maxX: Math.max(accumulator.maxX, point.x),
      minY: Math.min(accumulator.minY, point.y),
      maxY: Math.max(accumulator.maxY, point.y),
    }),
    { minX: points[0].x, maxX: points[0].x, minY: points[0].y, maxY: points[0].y },
  );
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  const spanX = Math.max(1, bounds.maxX - bounds.minX);
  const spanY = Math.max(1, bounds.maxY - bounds.minY);
  view.scale = Math.min(width / spanX, height / spanY) * 0.72;
  view.offsetX = -(bounds.minX + bounds.maxX) * 0.5 * view.scale;
  view.offsetY = (bounds.minY + bounds.maxY) * 0.5 * view.scale;
}

function draw() {
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  context.clearRect(0, 0, width, height);
  drawGrid(width, height);

  if (points.length === 0) {
    return;
  }

  context.lineWidth = 3;
  context.lineCap = "round";
  context.lineJoin = "round";
  context.strokeStyle = "#156082";
  context.beginPath();
  for (let index = 0; index < points.length; index += 1) {
    const screen = worldToScreen(points[index]);
    if (index === 0) {
      context.moveTo(screen.x, screen.y);
    } else {
      context.lineTo(screen.x, screen.y);
    }
  }
  context.stroke();

  for (const point of points) {
    const screen = worldToScreen(point);
    context.beginPath();
    context.arc(screen.x, screen.y, 4, 0, Math.PI * 2);
    context.fillStyle = "#156082";
    context.fill();
  }
}

function drawGrid(width, height) {
  context.fillStyle = "#ffffff";
  context.fillRect(0, 0, width, height);
  context.strokeStyle = "#d7dce2";
  context.lineWidth = 1;

  const step = view.scale;
  const origin = worldToScreen({ x: 0, y: 0, z: 0 });
  for (let x = origin.x % step; x < width; x += step) {
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, height);
    context.stroke();
  }
  for (let y = origin.y % step; y < height; y += step) {
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(width, y);
    context.stroke();
  }

  context.strokeStyle = "#9ca8b4";
  context.beginPath();
  context.moveTo(origin.x, 0);
  context.lineTo(origin.x, height);
  context.moveTo(0, origin.y);
  context.lineTo(width, origin.y);
  context.stroke();
}

function worldToScreen(point) {
  return {
    x: canvas.clientWidth * 0.5 + view.offsetX + point.x * view.scale,
    y: canvas.clientHeight * 0.5 + view.offsetY - point.y * view.scale,
  };
}

function screenToWorld(x, y) {
  return {
    x: (x - canvas.clientWidth * 0.5 - view.offsetX) / view.scale,
    y: -(y - canvas.clientHeight * 0.5 - view.offsetY) / view.scale,
  };
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}
