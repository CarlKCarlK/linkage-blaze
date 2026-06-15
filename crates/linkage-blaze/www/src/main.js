import init, { default_program, render_program_json } from "../pkg/linkage_blaze.js?v=builder-chain-2";

const source = document.querySelector("#source");
const error = document.querySelector("#error");
const canvas = document.querySelector("#view");
const cameraReadout = document.querySelector("#camera-readout");
const context = canvas.getContext("2d");

await init();

source.value = default_program();

let primitives = [];
const AXIS_LENGTH = 2.4;
const AXIS_LABEL_DISTANCE = 2.65;

let yaw = degreesToRadians(-13.7);
let pitch = degreesToRadians(-80.2);
let zoom = 52;
let panX = 0;
let panY = 0;
let dragging = false;
let dragButton = 0;
let lastPointerX = 0;
let lastPointerY = 0;
let renderTimer = null;

source.addEventListener("input", () => {
  window.clearTimeout(renderTimer);
  renderTimer = window.setTimeout(updatePreview, 140);
});

canvas.addEventListener("pointerdown", (event) => {
  dragging = true;
  dragButton = event.button;
  lastPointerX = event.clientX;
  lastPointerY = event.clientY;
  canvas.setPointerCapture(event.pointerId);
});

canvas.addEventListener("pointermove", (event) => {
  if (!dragging) {
    return;
  }

  const dx = event.clientX - lastPointerX;
  const dy = event.clientY - lastPointerY;
  lastPointerX = event.clientX;
  lastPointerY = event.clientY;

  if (dragButton === 2 || event.shiftKey) {
    panX += dx;
    panY += dy;
  } else {
    yaw += dx * 0.01;
    pitch = clamp(pitch + dy * 0.01, -1.4, 1.4);
  }
  draw();
});

canvas.addEventListener("pointerup", (event) => {
  dragging = false;
  canvas.releasePointerCapture(event.pointerId);
});

canvas.addEventListener("pointercancel", () => {
  dragging = false;
});

canvas.addEventListener("contextmenu", (event) => event.preventDefault());

canvas.addEventListener("wheel", (event) => {
  event.preventDefault();
  zoom = clamp(zoom * (event.deltaY > 0 ? 0.9 : 1.1), 12, 220);
  draw();
}, { passive: false });

window.addEventListener("resize", () => {
  resize();
  draw();
});

resize();
updatePreview();

function updatePreview() {
  try {
    const data = JSON.parse(render_program_json(source.value));
    primitives = data.primitives;
    error.textContent = "";
    draw();
  } catch (caught) {
    error.textContent = String(caught);
  }
}

function draw() {
  const width = canvas.width;
  const height = canvas.height;
  context.clearRect(0, 0, width, height);
  context.fillStyle = "#0d1118";
  context.fillRect(0, 0, width, height);

  drawGrid();
  drawAxes();

  for (const primitive of primitives) {
    if (primitive.type === "segment") {
      drawSegment(primitive);
    } else if (primitive.type === "disk") {
      drawDisk(primitive);
    } else if (primitive.type === "circle") {
      drawCircle(primitive);
    }
  }

  updateCameraReadout();
}

function drawGrid() {
  context.lineWidth = 1;
  context.strokeStyle = "#27313f";
  for (let value = -6; value <= 6; value += 1) {
    drawLine([value, -6, 0], [value, 6, 0]);
    drawLine([-6, value, 0], [6, value, 0]);
  }
}

function drawAxes() {
  context.lineWidth = 2;
  context.strokeStyle = "#ef5454";
  drawLine([0, 0, 0], [AXIS_LENGTH, 0, 0]);
  drawLabel("x", [AXIS_LABEL_DISTANCE, 0, 0], "#ef5454");
  context.strokeStyle = "#54ef8a";
  drawLine([0, 0, 0], [0, AXIS_LENGTH, 0]);
  drawLabel("y", [0, AXIS_LABEL_DISTANCE, 0], "#54ef8a");
  context.strokeStyle = "#54a8ef";
  drawLine([0, 0, 0], [0, 0, AXIS_LENGTH]);
  drawLabel("z", [0, 0, AXIS_LABEL_DISTANCE], "#54a8ef");
}

function drawSegment(primitive) {
  const start = project(primitive.start);
  const end = project(primitive.end);
  context.lineWidth = Math.max((primitive.width ?? 1) * 2, 1);
  context.lineCap = "round";
  context.strokeStyle = cssColor(primitive.color);
  context.beginPath();
  context.moveTo(start.x, start.y);
  context.lineTo(end.x, end.y);
  context.stroke();
}

function drawDisk(primitive) {
  const center = project(primitive.center);
  context.fillStyle = cssColor(primitive.color);
  context.beginPath();
  context.arc(center.x, center.y, primitive.radius * zoom, 0, Math.PI * 2);
  context.fill();
}

function drawCircle(primitive) {
  const center = project(primitive.center);
  context.lineWidth = Math.max((primitive.width ?? 1) * 2, 1);
  context.strokeStyle = cssColor(primitive.color);
  context.beginPath();
  context.arc(center.x, center.y, primitive.radius * zoom, 0, Math.PI * 2);
  context.stroke();
}

function drawLine(start, end) {
  const a = project(start);
  const b = project(end);
  context.beginPath();
  context.moveTo(a.x, a.y);
  context.lineTo(b.x, b.y);
  context.stroke();
}

function drawLabel(text, point, color) {
  const projected = project(point);
  context.fillStyle = color;
  context.font = "14px ui-monospace, SFMono-Regular, Consolas, monospace";
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillText(text, projected.x, projected.y);
}

function project(point) {
  const projected = projectDirection(point);
  return {
    x: canvas.width / 2 + panX + projected.x * zoom,
    y: canvas.height / 2 + panY - projected.y * zoom,
  };
}

function projectDirection(point) {
  const [x, y, z] = point;
  const cosYaw = Math.cos(yaw);
  const sinYaw = Math.sin(yaw);
  const cosPitch = Math.cos(pitch);
  const sinPitch = Math.sin(pitch);

  const x1 = x * cosYaw - y * sinYaw;
  const y1 = x * sinYaw + y * cosYaw;
  const z1 = z;
  const y2 = y1 * cosPitch - z1 * sinPitch;

  return {
    x: x1,
    y: y2,
  };
}

function updateCameraReadout() {
  const x = projectDirection([1, 0, 0]);
  const y = projectDirection([0, 1, 0]);
  const z = projectDirection([0, 0, 1]);
  cameraReadout.textContent =
    `yaw   ${formatAngle(yaw)}\n` +
    `pitch ${formatAngle(pitch)}\n` +
    `x screen ${formatVec2(x)}\n` +
    `y screen ${formatVec2(y)}\n` +
    `z screen ${formatVec2(z)}`;
}

function formatAngle(radians) {
  return `${((radians * 180) / Math.PI).toFixed(1)} deg`;
}

function formatVec2(vector) {
  return `(${vector.x.toFixed(2)}, ${vector.y.toFixed(2)})`;
}

function cssColor(color) {
  const [red, green, blue] = color.map((channel) => Math.round(clamp(channel, 0, 1) * 255));
  return `rgb(${red} ${green} ${blue})`;
}

function resize() {
  const bounds = canvas.getBoundingClientRect();
  const scale = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.max(Math.floor(bounds.width * scale), 1);
  canvas.height = Math.max(Math.floor(bounds.height * scale), 1);
  context.setTransform(scale, 0, 0, scale, 0, 0);
  canvas.width = Math.max(Math.floor(bounds.width), 1);
  canvas.height = Math.max(Math.floor(bounds.height), 1);
}

function clamp(value, low, high) {
  return Math.min(Math.max(value, low), high);
}

function degreesToRadians(degrees) {
  return (degrees * Math.PI) / 180;
}
