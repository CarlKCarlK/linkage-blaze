import init, { default_program, render_program_json, render_program_with_params_json } from "../pkg/linkage_blaze.js?v=builder-chain-8";

const source = document.querySelector("#source");
const error = document.querySelector("#error");
const canvas = document.querySelector("#view");
const cameraReadout = document.querySelector("#camera-readout");
const context = canvas.getContext("2d");

await init();

source.value = default_program();

let primitives = [];
// name → current value (0..1); preserved across source edits by name-matching
let paramValues = new Map();
const paramsList = document.querySelector("#params-list");

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

source.addEventListener("keydown", (event) => {
  if (event.key === "/" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    toggleLineComments();
  }
});

function toggleLineComments() {
  const start = source.selectionStart;
  const end = source.selectionEnd;
  const text = source.value;

  // Find the start of the first selected line and end of the last selected line
  const lineStart = text.lastIndexOf("\n", start - 1) + 1;
  const lineEnd = text.indexOf("\n", end - 1);
  const block = text.slice(lineStart, lineEnd === -1 ? undefined : lineEnd);
  const lines = block.split("\n");

  // If every non-empty line starts with //, uncomment; otherwise comment
  const allCommented = lines.every((line) => line.trim() === "" || line.trimStart().startsWith("//"));

  const toggled = lines
    .map((line) => {
      if (allCommented) {
        // Remove the first // (and optional single space after it)
        return line.replace(/^(\s*)\/\/ ?/, "$1");
      } else {
        // Add // preserving leading whitespace
        return line.replace(/^(\s*)/, "$1// ");
      }
    })
    .join("\n");

  const after = text.slice(lineEnd === -1 ? text.length : lineEnd);
  source.value = text.slice(0, lineStart) + toggled + after;

  // Restore selection covering the same lines
  source.selectionStart = lineStart;
  source.selectionEnd = lineStart + toggled.length;

  source.dispatchEvent(new Event("input"));
}

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
    pitch = clamp(pitch + dy * 0.01, -Math.PI / 2, Math.PI / 2);
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
    const overrides = buildOverridesJson();
    const data = JSON.parse(render_program_with_params_json(source.value, overrides));
    // Merge returned params: keep existing slider values by name, use returned value for new params
    const nextValues = new Map(data.params.map(({ name, value }) => [name, value]));
    for (const [name] of nextValues) {
      if (paramValues.has(name)) {
        nextValues.set(name, paramValues.get(name));
      }
    }
    paramValues = nextValues;
    rebuildSliders(data.params);
    primitives = data.primitives;
    error.textContent = "";
    draw();
  } catch (caught) {
    error.textContent = String(caught);
  }
}

function buildOverridesJson() {
  const entries = [...paramValues.entries()].map(
    ([name, value]) => `"${name}":${value}`
  );
  return `{${entries.join(",")}}`;
}

function rebuildSliders(params) {
  if (params.length === 0) {
    paramsList.innerHTML = '<div class="params-empty">No parameters defined</div>';
    return;
  }

  // Diff: reuse existing slider elements where name matches to avoid focus loss
  const existing = new Map(
    [...paramsList.querySelectorAll(".param-item")].map((el) => [
      el.dataset.name,
      el,
    ])
  );
  const incoming = new Set(params.map((p) => p.name));

  // Remove stale
  for (const [name, el] of existing) {
    if (!incoming.has(name)) el.remove();
  }

  // Insert/update in order
  let insertBefore = null;
  for (let i = params.length - 1; i >= 0; i--) {
    const { name } = params[i];
    if (existing.has(name)) {
      insertBefore = existing.get(name);
    } else {
      const item = createSliderItem(name, paramValues.get(name) ?? 0.5);
      paramsList.insertBefore(item, insertBefore);
      insertBefore = item;
    }
  }

  // Update value displays (slider positions stay as-is since we preserve paramValues)
  for (const { name } of params) {
    const el = paramsList.querySelector(`[data-name="${CSS.escape(name)}"]`);
    if (el) {
      el.querySelector(".param-value").textContent = (paramValues.get(name) ?? 0).toFixed(3);
    }
  }
}

function createSliderItem(name, value) {
  const item = document.createElement("div");
  item.className = "param-item";
  item.dataset.name = name;

  const label = document.createElement("div");
  label.className = "param-label";

  const nameSpan = document.createElement("span");
  nameSpan.className = "param-name";
  nameSpan.textContent = name;

  const valueSpan = document.createElement("span");
  valueSpan.className = "param-value";
  valueSpan.textContent = value.toFixed(3);

  label.appendChild(nameSpan);
  label.appendChild(valueSpan);

  const slider = document.createElement("input");
  slider.type = "range";
  slider.className = "param-slider";
  slider.min = 0;
  slider.max = 1;
  slider.step = 0.001;
  slider.value = value;

  slider.addEventListener("input", () => {
    const v = parseFloat(slider.value);
    paramValues.set(name, v);
    valueSpan.textContent = v.toFixed(3);
    try {
      const data = JSON.parse(render_program_with_params_json(source.value, buildOverridesJson()));
      primitives = data.primitives;
      error.textContent = "";
      draw();
    } catch (caught) {
      error.textContent = String(caught);
    }
  });

  item.appendChild(label);
  item.appendChild(slider);
  return item;
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
    } else if (primitive.type === "ring") {
      drawRing(primitive);
    } else if (primitive.type === "sphere") {
      drawSphere(primitive);
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
  context.fillStyle = cssColor(primitive.color);
  drawProjectedCircle(primitive.center, primitive.normal, primitive.radius, true);
}

function drawRing(primitive) {
  context.lineWidth = Math.max((primitive.width ?? 1) * 2, 1);
  context.strokeStyle = cssColor(primitive.color);
  drawProjectedCircle(primitive.center, primitive.normal, primitive.radius, false);
}

function drawSphere(primitive) {
  const center = project(primitive.center);
  context.fillStyle = cssColor(primitive.color);
  context.beginPath();
  context.arc(center.x, center.y, primitive.radius * zoom, 0, Math.PI * 2);
  context.fill();
}

function drawProjectedCircle(center, normal, radius, filled) {
  const center2 = project(center);
  const basis = planeBasis(normal);
  const segments = 80;
  context.beginPath();

  for (let pointIndex = 0; pointIndex <= segments; pointIndex += 1) {
    const angle = (pointIndex / segments) * Math.PI * 2;
    const point = [
      center[0] + (basis.a[0] * Math.cos(angle) + basis.b[0] * Math.sin(angle)) * radius,
      center[1] + (basis.a[1] * Math.cos(angle) + basis.b[1] * Math.sin(angle)) * radius,
      center[2] + (basis.a[2] * Math.cos(angle) + basis.b[2] * Math.sin(angle)) * radius,
    ];
    const projected = project(point);
    if (pointIndex === 0) {
      context.moveTo(projected.x, projected.y);
    } else {
      context.lineTo(projected.x, projected.y);
    }
  }

  context.closePath();
  if (filled) {
    context.fill();
  } else {
    context.stroke();
  }
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

function planeBasis(normal) {
  const n = normalize(normal);
  const reference = Math.abs(n[2]) > 0.8 ? [0, 1, 0] : [0, 0, 1];
  const a = normalize(cross(reference, n));
  const b = normalize(cross(n, a));
  return { a, b };
}

function normalize(vector) {
  const length = Math.hypot(vector[0], vector[1], vector[2]) || 1;
  return [vector[0] / length, vector[1] / length, vector[2] / length];
}

function cross(left, right) {
  return [
    left[1] * right[2] - left[2] * right[1],
    left[2] * right[0] - left[0] * right[2],
    left[0] * right[1] - left[1] * right[0],
  ];
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
