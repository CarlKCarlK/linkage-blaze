import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import init, { default_program, render_program_with_params_json } from "../pkg/linkage_blaze.js?v=builder-chain-8";

const source = document.querySelector("#source");
const error = document.querySelector("#error");
const canvas = document.querySelector("#view");
const paramsList = document.querySelector("#params-list");

await init();
source.value = default_program();

let primitives = [];
let paramValues = new Map();
let renderTimer = null;

// ---- Three.js setup ----
const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0d1118);

const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000);
camera.up.set(0, 0, 1); // Z is up

const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.dampingFactor = 0.08;
controls.screenSpacePanning = true;
controls.mouseButtons = {
  LEFT: THREE.MOUSE.ROTATE,
  MIDDLE: THREE.MOUSE.PAN,
  RIGHT: THREE.MOUSE.PAN,
};

// Scroll to zoom (exponential, matching robot-arm-wasm)
canvas.addEventListener("wheel", (event) => {
  event.preventDefault();
  event.stopImmediatePropagation();
  const factor = Math.exp(clamp(event.deltaY, -80, 80) * 0.0015);
  const dir = camera.position.clone().sub(controls.target);
  camera.position.copy(controls.target).addScaledVector(dir, factor);
  controls.update();
}, { capture: true, passive: false });

// Grid in the Z=0 plane
const grid = new THREE.GridHelper(12, 12, 0x27313f, 0x27313f);
grid.rotation.x = Math.PI / 2;
scene.add(grid);

// Axes
const AXIS_LENGTH = 2.4;
scene.add(axisLine([0, 0, 0], [AXIS_LENGTH, 0, 0], 0xef5454)); // X red
scene.add(axisLine([0, 0, 0], [0, AXIS_LENGTH, 0], 0x54ef8a)); // Y green
scene.add(axisLine([0, 0, 0], [0, 0, AXIS_LENGTH], 0x54a8ef)); // Z blue

// Group holding all linkage primitives; cleared on each re-render
const linkageGroup = new THREE.Group();
scene.add(linkageGroup);

// Initial camera position (same rough angle as the old default)
camera.position.set(6, -14, 8);
controls.target.set(0, 0, 3);
controls.update();

function animate() {
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}
animate();

window.addEventListener("resize", resize);
resize();

// ---- Editor ----
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

updatePreview();

function updatePreview() {
  try {
    const overrides = buildOverridesJson();
    const data = JSON.parse(render_program_with_params_json(source.value, overrides));
    const nextValues = new Map(data.params.map(({ name, value }) => [name, value]));
    for (const [name] of nextValues) {
      if (paramValues.has(name)) nextValues.set(name, paramValues.get(name));
    }
    paramValues = nextValues;
    rebuildSliders(data.params);
    primitives = data.primitives;
    error.textContent = "";
    rebuildLinkage();
  } catch (caught) {
    error.textContent = String(caught);
  }
}

function buildOverridesJson() {
  const entries = [...paramValues.entries()].map(([name, value]) => `"${name}":${value}`);
  return `{${entries.join(",")}}`;
}

// ---- Three.js primitive rendering ----
function rebuildLinkage() {
  linkageGroup.traverse((obj) => {
    if (obj.isMesh || obj.isLine) {
      obj.geometry.dispose();
      obj.material.dispose();
    }
  });
  linkageGroup.clear();

  for (const p of primitives) {
    if (p.type === "segment") addSegment(p);
    else if (p.type === "disk") addDisk(p);
    else if (p.type === "ring") addRing(p);
    else if (p.type === "sphere") addSphere(p);
  }
}

function addSegment(p) {
  const start = new THREE.Vector3(...p.start);
  const end = new THREE.Vector3(...p.end);
  const dir = end.clone().sub(start);
  const length = dir.length();
  if (length < 1e-6) return;
  const radius = Math.max((p.width ?? 1) * 0.025, 0.015);
  const geom = new THREE.CylinderGeometry(radius, radius, length, 8);
  const mat = new THREE.MeshBasicMaterial({ color: threeColor(p.color) });
  const mesh = new THREE.Mesh(geom, mat);
  mesh.position.copy(start).lerp(end, 0.5);
  mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir.normalize());
  linkageGroup.add(mesh);
}

function addDisk(p) {
  const geom = new THREE.CircleGeometry(p.radius, 64);
  const mat = new THREE.MeshBasicMaterial({ color: threeColor(p.color), side: THREE.DoubleSide });
  const mesh = new THREE.Mesh(geom, mat);
  mesh.position.set(...p.center);
  orientToNormal(mesh, p.normal);
  linkageGroup.add(mesh);
}

function addRing(p) {
  const hw = Math.max((p.width ?? 1) * 0.025, 0.015);
  const geom = new THREE.RingGeometry(p.radius - hw, p.radius + hw, 64);
  const mat = new THREE.MeshBasicMaterial({ color: threeColor(p.color), side: THREE.DoubleSide });
  const mesh = new THREE.Mesh(geom, mat);
  mesh.position.set(...p.center);
  orientToNormal(mesh, p.normal);
  linkageGroup.add(mesh);
}

function addSphere(p) {
  const geom = new THREE.SphereGeometry(p.radius, 24, 16);
  const mat = new THREE.MeshBasicMaterial({ color: threeColor(p.color) });
  const mesh = new THREE.Mesh(geom, mat);
  mesh.position.set(...p.center);
  linkageGroup.add(mesh);
}

function orientToNormal(mesh, normal) {
  const n = new THREE.Vector3(...normal).normalize();
  if (n.lengthSq() > 0) {
    mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), n);
  }
}

function axisLine(from, to, color) {
  const geom = new THREE.BufferGeometry().setFromPoints([
    new THREE.Vector3(...from),
    new THREE.Vector3(...to),
  ]);
  return new THREE.Line(geom, new THREE.LineBasicMaterial({ color }));
}

function threeColor([r, g, b]) {
  return new THREE.Color(r, g, b);
}

function resize() {
  const bounds = canvas.getBoundingClientRect();
  const w = Math.max(1, bounds.width);
  const h = Math.max(1, bounds.height);
  renderer.setSize(w, h, false);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
}

function clamp(value, low, high) {
  return Math.min(Math.max(value, low), high);
}

// ---- Sliders ----
function rebuildSliders(params) {
  if (params.length === 0) {
    paramsList.innerHTML = '<div class="params-empty">No parameters defined</div>';
    return;
  }

  const existing = new Map(
    [...paramsList.querySelectorAll(".param-item")].map((el) => [el.dataset.name, el])
  );
  const incoming = new Set(params.map((p) => p.name));

  for (const [name, el] of existing) {
    if (!incoming.has(name)) el.remove();
  }

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
      rebuildLinkage();
    } catch (caught) {
      error.textContent = String(caught);
    }
  });

  item.appendChild(label);
  item.appendChild(slider);
  return item;
}

// ---- Comment toggle ----
function toggleLineComments() {
  const start = source.selectionStart;
  const end = source.selectionEnd;
  const text = source.value;

  const lineStart = text.lastIndexOf("\n", start - 1) + 1;
  const lineEnd = text.indexOf("\n", end - 1);
  const block = text.slice(lineStart, lineEnd === -1 ? undefined : lineEnd);
  const lines = block.split("\n");

  const allCommented = lines.every((line) => line.trim() === "" || line.trimStart().startsWith("//"));

  const toggled = lines
    .map((line) => {
      if (allCommented) {
        return line.replace(/^(\s*)\/\/ ?/, "$1");
      } else {
        return line.replace(/^(\s*)/, "$1// ");
      }
    })
    .join("\n");

  const after = text.slice(lineEnd === -1 ? text.length : lineEnd);
  source.value = text.slice(0, lineStart) + toggled + after;

  source.selectionStart = lineStart;
  source.selectionEnd = lineStart + toggled.length;

  source.dispatchEvent(new Event("input"));
}
