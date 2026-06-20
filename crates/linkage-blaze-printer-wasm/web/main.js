import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import init, { PrinterSimWasm } from "../pkg/linkage_blaze_printer_wasm.js";

// ── DOM refs ──────────────────────────────────────────────────────────────────
const canvas = document.querySelector("#printer-canvas");
const fileInput = document.querySelector("#file-input");
const resetBtn = document.querySelector("#reset-btn");
const playBtn = document.querySelector("#play-btn");
const speedSlider = document.querySelector("#speed-slider");
const speedDisplay = document.querySelector("#speed-display");
const showTravelCheck = document.querySelector("#show-travel");
const showFrameCheck = document.querySelector("#show-frame");
const layerDisplay = document.querySelector("#layer-display");
const progressDisplay = document.querySelector("#progress-display");
const progressBar = document.querySelector("#progress-bar");

// ── Three.js setup ────────────────────────────────────────────────────────────
const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
renderer.setClearColor(0xffffff, 1);
renderer.setPixelRatio(window.devicePixelRatio || 1);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0xffffff);

const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 5000);
camera.up.set(0, 0, 1);

const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.dampingFactor = 0.08;
controls.screenSpacePanning = true;
controls.enableZoom = false;
controls.mouseButtons = {
  LEFT: THREE.MOUSE.ROTATE,
  MIDDLE: THREE.MOUSE.PAN,
  RIGHT: THREE.MOUSE.PAN,
};

// Build plate grid (XY plane, Z up)
const grid = new THREE.GridHelper(220, 22, 0x9ca8b4, 0xd7dce2);
grid.rotation.x = Math.PI / 2;
grid.position.set(110, 110, 0);
scene.add(grid);

// Materials
const extrusionMaterial = new THREE.LineBasicMaterial({ color: 0x156082, linewidth: 1 });
const travelMaterial = new THREE.LineBasicMaterial({ color: 0xadb5bd, linewidth: 1 });
const toolheadMaterial = new THREE.MeshBasicMaterial({ color: 0xe63946 });
const frameMaterial = new THREE.LineBasicMaterial({ color: 0xced4da });

// Toolhead marker
const toolheadMesh = new THREE.Mesh(
  new THREE.SphereGeometry(1.5, 16, 12),
  toolheadMaterial,
);
scene.add(toolheadMesh);

// Line segment objects (rebuilt when geometry changes)
let extrusionLines = null;
let travelLines = null;
let frameBox = null;

// ── Simulation state ──────────────────────────────────────────────────────────
await init();
let sim = new PrinterSimWasm();
let playing = false;
let animFrameId = null;
let firstFit = true;

// ── Controls ──────────────────────────────────────────────────────────────────
fileInput.addEventListener("change", async () => {
  const file = fileInput.files[0];
  if (!file) return;
  const text = await file.text();
  sim.load(text);
  sim.reset();
  playing = false;
  updatePlayBtn();
  playBtn.disabled = false;
  firstFit = true;
  rebuildGeometry();
  updateStatus();
  fitView();
  render();
});

resetBtn.addEventListener("click", () => {
  sim.reset();
  playing = false;
  updatePlayBtn();
  rebuildGeometry();
  updateStatus();
  render();
});

playBtn.addEventListener("click", () => {
  if (sim.isDone()) {
    sim.reset();
    rebuildGeometry();
  }
  playing = !playing;
  updatePlayBtn();
  if (playing) {
    scheduleFrame();
  }
});

speedSlider.addEventListener("input", () => {
  speedDisplay.textContent = `${speedSlider.value} seg/frame`;
});

showTravelCheck.addEventListener("change", () => {
  if (travelLines) travelLines.visible = showTravelCheck.checked;
  render();
});

showFrameCheck.addEventListener("change", () => {
  if (frameBox) frameBox.visible = showFrameCheck.checked;
  render();
});

window.addEventListener("resize", () => {
  resize();
  render();
});

renderer.domElement.addEventListener(
  "wheel",
  (event) => {
    event.preventDefault();
    event.stopImmediatePropagation();
    dollyCamera(event.deltaY);
    render();
  },
  { capture: true, passive: false },
);

// ── Animation loop ────────────────────────────────────────────────────────────
function scheduleFrame() {
  if (animFrameId !== null) return;
  animFrameId = requestAnimationFrame(tick);
}

function tick() {
  animFrameId = null;
  controls.update();

  if (playing) {
    const count = Number(speedSlider.value);
    sim.advance(count);
    rebuildGeometry();
    updateStatus();
    if (sim.isDone()) {
      playing = false;
      updatePlayBtn();
    } else {
      scheduleFrame();
    }
  }

  render();
}

function render() {
  renderer.render(scene, camera);
}

// ── Geometry helpers ──────────────────────────────────────────────────────────
function rebuildGeometry() {
  rebuildLines("extrusion");
  rebuildLines("travel");
  updateToolhead();
  rebuildFrame();
  if (firstFit && sim.segmentCount() > 0) {
    fitView();
    firstFit = false;
  }
}

function rebuildLines(kind) {
  const flat = kind === "extrusion" ? sim.extrusionSegments() : sim.travelSegments();
  const material = kind === "extrusion" ? extrusionMaterial : travelMaterial;
  const ref_var = kind === "extrusion" ? "extrusionLines" : "travelLines";

  // Remove old
  if (kind === "extrusion" && extrusionLines) {
    scene.remove(extrusionLines);
    extrusionLines.geometry.dispose();
    extrusionLines = null;
  }
  if (kind === "travel" && travelLines) {
    scene.remove(travelLines);
    travelLines.geometry.dispose();
    travelLines = null;
  }

  if (flat.length === 0) return;

  const positions = new Float32Array(flat.length);
  for (let index = 0; index < flat.length; index += 6) {
    // G-code (x, y, z) → Three.js (x, y, -z) with camera.up = Z
    positions[index]     = flat[index];
    positions[index + 1] = flat[index + 1];
    positions[index + 2] = -flat[index + 2];
    positions[index + 3] = flat[index + 3];
    positions[index + 4] = flat[index + 4];
    positions[index + 5] = -flat[index + 5];
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));

  const lines = new THREE.LineSegments(geometry, material);
  if (kind === "extrusion") {
    extrusionLines = lines;
    scene.add(extrusionLines);
  } else {
    travelLines = lines;
    travelLines.visible = showTravelCheck.checked;
    scene.add(travelLines);
  }
}

function updateToolhead() {
  const pos = sim.toolheadPosition();
  toolheadMesh.position.set(pos[0], pos[1], -pos[2]);
}

function rebuildFrame() {
  if (frameBox) {
    scene.remove(frameBox);
    frameBox.geometry.dispose();
    frameBox = null;
  }

  const bbox = sim.boundingBox();
  if (bbox[0] >= bbox[3]) return; // empty

  const width = bbox[3] - bbox[0];
  const height = bbox[4] - bbox[1];
  const depth = bbox[5] - bbox[2];
  const geometry = new THREE.BoxGeometry(width, height, depth);
  frameBox = new THREE.LineSegments(new THREE.EdgesGeometry(geometry), frameMaterial);
  frameBox.position.set(
    (bbox[0] + bbox[3]) * 0.5,
    (bbox[1] + bbox[4]) * 0.5,
    -((bbox[2] + bbox[5]) * 0.5),
  );
  frameBox.visible = showFrameCheck.checked;
  scene.add(frameBox);
  geometry.dispose();
}

// ── Camera helpers ────────────────────────────────────────────────────────────
function fitView() {
  const bbox = sim.boundingBox();
  if (bbox[0] >= bbox[3]) {
    // No geometry yet — default view centred on build plate
    controls.target.set(110, 110, 0);
    camera.position.set(110, -220, 180);
    camera.near = 1;
    camera.far = 2000;
    controls.minDistance = 20;
    controls.maxDistance = 1000;
    camera.updateProjectionMatrix();
    controls.update();
    return;
  }

  const cx = (bbox[0] + bbox[3]) * 0.5;
  const cy = (bbox[1] + bbox[4]) * 0.5;
  const cz = (bbox[2] + bbox[5]) * 0.5;
  const diagonal = Math.sqrt(
    (bbox[3] - bbox[0]) ** 2 + (bbox[4] - bbox[1]) ** 2 + (bbox[5] - bbox[2]) ** 2,
  );
  const radius = Math.max(diagonal * 0.5, 10);

  controls.target.set(cx, cy, -cz);
  camera.position.set(cx + radius * 1.5, cy - radius * 2.0, -cz + radius * 1.2);
  camera.near = Math.max(0.1, radius / 100);
  camera.far = radius * 100;
  controls.minDistance = Math.max(5, radius * 0.2);
  controls.maxDistance = Math.max(200, radius * 8);
  camera.updateProjectionMatrix();
  controls.update();
}

function dollyCamera(deltaY) {
  const direction = camera.position.clone().sub(controls.target);
  const currentDistance = direction.length();
  if (currentDistance === 0) return;
  const factor = Math.exp(clamp(deltaY, -80, 80) * 0.0015);
  const nextDistance = clamp(currentDistance * factor, controls.minDistance, controls.maxDistance);
  camera.position.copy(controls.target).add(direction.multiplyScalar(nextDistance / currentDistance));
  camera.updateProjectionMatrix();
  controls.update();
}

// ── UI helpers ────────────────────────────────────────────────────────────────
function updatePlayBtn() {
  playBtn.textContent = playing ? "Pause" : "Play";
  playBtn.classList.toggle("playing", playing);
}

function updateStatus() {
  const layer = sim.currentLayer();
  const progress = sim.progress();
  layerDisplay.textContent = sim.segmentCount() === 0 ? "—" : String(layer);
  progressDisplay.textContent = sim.segmentCount() === 0 ? "—" : `${(progress * 100).toFixed(1)} %`;
  progressBar.value = progress;
}

function resize() {
  const bounds = canvas.getBoundingClientRect();
  renderer.setSize(Math.max(1, bounds.width), Math.max(1, bounds.height), false);
  camera.aspect = bounds.width / Math.max(1, bounds.height);
  camera.updateProjectionMatrix();
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

// ── Init ──────────────────────────────────────────────────────────────────────
resize();
fitView();
render();

// Orbit controls need continuous updates during damping
(function animateControls() {
  requestAnimationFrame(animateControls);
  if (controls.update()) render();
})();
