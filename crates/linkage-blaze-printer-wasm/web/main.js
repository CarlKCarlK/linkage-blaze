import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import init, { PrinterSimWasm, printerDrawItems } from "../pkg/linkage_blaze_printer_wasm.js";

// ── Printer build volume (mm) ─────────────────────────────────────────────────
const BUILD_X = 220;
const BUILD_Y = 220;
const BUILD_Z = 250;

// Floats per draw item: [type, x0,y0,z0, x1,y1,z1, r,g,b, size1, size2]
const STRIDE = 12;

// ── DOM refs ──────────────────────────────────────────────────────────────────
const canvas = document.querySelector("#printer-canvas");
const fileInput = document.querySelector("#file-input");
const resetBtn = document.querySelector("#reset-btn");
const playBtn = document.querySelector("#play-btn");
const speedSlider = document.querySelector("#speed-slider");
const speedDisplay = document.querySelector("#speed-display");
const stepBtn = document.querySelector("#step-btn");
const showTravelCheck = document.querySelector("#show-travel");
const showPrinterCheck = document.querySelector("#show-printer");
const showFrameCheck = document.querySelector("#show-frame");
const layerDisplay = document.querySelector("#layer-display");
const progressDisplay = document.querySelector("#progress-display");
const progressBar = document.querySelector("#progress-bar");

// ── Three.js core ─────────────────────────────────────────────────────────────
const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
renderer.setClearColor(0xffffff, 1);
renderer.setPixelRatio(window.devicePixelRatio || 1);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0xffffff);

// G-code (x, y, z) → Three.js (x, y, z): Z is visually up.
const camera = new THREE.PerspectiveCamera(45, 1, 0.5, 10000);
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

// Ground grid at the build-plate level (Z = 0)
const grid = new THREE.GridHelper(500, 50, 0xb0b8c4, 0xdde0e6);
grid.rotation.x = Math.PI / 2;
grid.position.set(BUILD_X / 2, BUILD_Y / 2, 0);
scene.add(grid);

// ── Materials ─────────────────────────────────────────────────────────────────
const extrusionMaterial = new THREE.LineBasicMaterial({ color: 0x156082 });
const travelMaterial = new THREE.LineBasicMaterial({ color: 0xadb5bd });
const frameMaterial = new THREE.LineBasicMaterial({ color: 0xced4da });
const bedMaterial = new THREE.MeshBasicMaterial({ color: 0xdde2e8, side: THREE.DoubleSide });

// ── Printer structure group ────────────────────────────────────────────────────
// Static parts (bed + right Z rail) live here alongside dynamic draw-item objects.
const printerGroup = new THREE.Group();
scene.add(printerGroup);

// Build plate — flat slab at Z = 0
const bedMesh = new THREE.Mesh(new THREE.BoxGeometry(BUILD_X, BUILD_Y, 4), bedMaterial);
bedMesh.position.set(BUILD_X / 2, BUILD_Y / 2, -2);
printerGroup.add(bedMesh);


// Sub-group for dynamic draw items from the linkage (rebuilt every frame)
const dynamicGroup = new THREE.Group();
printerGroup.add(dynamicGroup);

// ── G-code path geometry ──────────────────────────────────────────────────────
// Incremental approach: pre-allocate buffers sized for all segments at load time.
// Each frame we append only new vertices and update drawRange.count.
// This avoids O(n) malloc/free on every frame that eventually exhausts the WASM heap.
let extrusionLines = null;
let travelLines = null;
let extrusionBuf = null;   // Float32Array backing extrusionLines geometry
let travelBuf = null;      // Float32Array backing travelLines geometry
let extrusionVertCount = 0; // floats written so far
let travelVertCount = 0;
let lastGeomIndex = 0;     // sim segment index as of last geometry update
let frameBox = null;

// ── WASM + simulation ─────────────────────────────────────────────────────────
await init();
let sim = new PrinterSimWasm();
let playing = false;
let firstFit = true;
let segAccum = 0.0;  // fractional segment accumulator for sub-1 speeds
let lastGcode = "";  // kept for sim recovery after a WASM panic

updatePrinterFromDrawItems(0, 0, 0);
updateSpeedDisplay();

// ── Sim recovery ──────────────────────────────────────────────────────────────
// wasm-bindgen's RefCell is permanently locked after any Rust panic.
// Drop the broken object and create a fresh one, re-loading from lastGcode.
function recoverSim() {
  let fresh = new PrinterSimWasm();
  if (lastGcode) {
    try {
      fresh.load(lastGcode);
      fresh.reset();
      initGCodeGeometry();
      refreshAll();
    } catch (e) {
      console.error("Sim recovery load also failed:", e.message);
      fresh = new PrinterSimWasm();
    }
  }
  return fresh;
}

// ── Controls ──────────────────────────────────────────────────────────────────
fileInput.addEventListener("change", async () => {
  const file = fileInput.files[0];
  if (!file) return;
  const text = await file.text();
  try {
    sim.load(text);
  } catch (e) {
    // wasm-bindgen permanently locks the RefCell on any Rust panic.
    // Recreate the sim object to get a fresh, unlocked RefCell.
    console.error("G-code load failed (recreating sim):", e.message);
    sim = new PrinterSimWasm();
    return;
  }
  lastGcode = text;
  sim.reset();
  playing = false;
  segAccum = 0;
  updatePlayBtn();
  playBtn.disabled = false;
  stepBtn.disabled = false;
  firstFit = true;
  initGCodeGeometry();
  refreshAll();
});

resetBtn.addEventListener("click", () => {
  try {
    sim.reset();
  } catch (e) {
    console.error("WASM reset error — recreating sim:", e.message);
    sim = recoverSim();
  }
  playing = false;
  segAccum = 0;
  updatePlayBtn();
  // Re-init geometry to reset draw ranges back to zero without re-parsing.
  initGCodeGeometry();
  refreshAll();
});

playBtn.addEventListener("click", () => {
  if (sim.isDone()) sim.reset();
  playing = !playing;
  segAccum = 0;
  updatePlayBtn();
});

stepBtn.addEventListener("click", () => {
  playing = false;
  updatePlayBtn();
  try {
    advanceAndRefresh(1);
  } catch (e) {
    console.error("WASM step error — recreating sim:", e.message);
    sim = recoverSim();
  }
});

speedSlider.addEventListener("input", updateSpeedDisplay);

showTravelCheck.addEventListener("change", () => {
  if (travelLines) travelLines.visible = showTravelCheck.checked;
});

showPrinterCheck.addEventListener("change", () => {
  printerGroup.visible = showPrinterCheck.checked;
});

showFrameCheck.addEventListener("change", () => {
  if (frameBox) frameBox.visible = showFrameCheck.checked;
});

window.addEventListener("resize", () => { resize(); });

renderer.domElement.addEventListener("wheel", (event) => {
  event.preventDefault();
  event.stopImmediatePropagation();
  dollyCamera(event.deltaY);
}, { capture: true, passive: false });

// ── Animation loop ────────────────────────────────────────────────────────────
resize();
fitDefault();

(function loop() {
  requestAnimationFrame(loop);
  controls.update();

  if (playing) {
    // Slider 1–400 maps to 0.05–20 segs/frame (value/20).
    // Fractional accumulator lets speed go below 1 seg/frame.
    segAccum += Number(speedSlider.value) / 20;
    const toAdvance = Math.floor(segAccum);
    if (toAdvance > 0) {
      segAccum -= toAdvance;
      try {
        advanceAndRefresh(toAdvance);
      } catch (e) {
        // WASM RefCell may be permanently locked; stop playback and attempt recovery.
        console.error("WASM advance error:", e.message);
        playing = false;
        updatePlayBtn();
        sim = recoverSim();
      }
      if (playing && sim.isDone()) {
        playing = false;
        updatePlayBtn();
      }
    }
  }

  renderer.render(scene, camera);
})();

// ── Draw-items printer update ─────────────────────────────────────────────────
// Calls printerDrawItems() from WASM and rebuilds dynamicGroup each frame.
// Each draw item is 12 floats: [type, x0,y0,z0, x1,y1,z1, r,g,b, size1, size2]
//   type 0 = Stroke   (x0..z0 = start, x1..z1 = end, size1 = width)
//   type 1 = Sphere   (x0..z0 = center, size1 = radius)
//   type 2 = Disk     (x0..z0 = center, size1 = radius)  [unused by printer]
//   type 3 = Ring     (x0..z0 = center, size1 = radius, size2 = width) [unused]
function updatePrinterFromDrawItems(toolX, toolY, toolZ) {
  // Dispose and remove all old dynamic objects
  for (let i = dynamicGroup.children.length - 1; i >= 0; i--) {
    const obj = dynamicGroup.children[i];
    if (obj.geometry) obj.geometry.dispose();
    if (obj.material) obj.material.dispose();
    dynamicGroup.remove(obj);
  }

  const items = printerDrawItems(toolX, toolY, toolZ);

  for (let i = 0; i + STRIDE <= items.length; i += STRIDE) {
    const type = items[i];
    const x0 = items[i + 1], y0 = items[i + 2], z0 = items[i + 3];
    const x1 = items[i + 4], y1 = items[i + 5], z1 = items[i + 6];
    const r = items[i + 7] / 255, g = items[i + 8] / 255, b = items[i + 9] / 255;
    const size1 = items[i + 10];

    const color = new THREE.Color(r, g, b);
    const mat = new THREE.MeshBasicMaterial({ color });

    if (type === 0) {
      // Stroke — render as a cylinder tube between the two endpoints
      const start = new THREE.Vector3(x0, y0, z0);
      const end   = new THREE.Vector3(x1, y1, z1);
      const length = start.distanceTo(end);
      if (length < 0.01) continue;          // skip degenerate zero-length segments
      const geo = new THREE.CylinderGeometry(2, 2, length, 6);
      const mesh = new THREE.Mesh(geo, mat);
      // CylinderGeometry is along Y; orient it along the segment direction
      const mid = start.clone().add(end).multiplyScalar(0.5);
      mesh.position.copy(mid);
      mesh.quaternion.setFromUnitVectors(
        new THREE.Vector3(0, 1, 0),
        end.clone().sub(start).normalize(),
      );
      dynamicGroup.add(mesh);
    } else if (type === 1) {
      // Sphere
      const geo = new THREE.SphereGeometry(size1, 10, 7);
      const mesh = new THREE.Mesh(geo, mat);
      mesh.position.set(x0, y0, z0);
      dynamicGroup.add(mesh);
    } else if (type === 2) {
      // Disk
      const geo = new THREE.CircleGeometry(size1, 16);
      const mesh = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color, side: THREE.DoubleSide }));
      mesh.position.set(x0, y0, z0);
      dynamicGroup.add(mesh);
    }
  }
}

function advanceAndRefresh(count) {
  sim.advance(count);
  rebuildGCodeGeometry();
  const [tx, ty, tz] = currentToolhead();
  updatePrinterFromDrawItems(tx, ty, tz);
  updateStatus();
}

function currentToolhead() {
  if (sim.segmentCount() === 0) return [0, 0, 0];
  const pos = sim.toolheadPosition();
  return [pos[0], pos[1], pos[2]];
}

// ── G-code geometry ───────────────────────────────────────────────────────────
function refreshAll() {
  rebuildGCodeGeometry();
  const [tx, ty, tz] = currentToolhead();
  updatePrinterFromDrawItems(tx, ty, tz);
  updateStatus();
  if (firstFit && sim.segmentCount() > 0) {
    fitToPrint();
    firstFit = false;
  }
}

// Called after sim.load() to create pre-allocated geometry buffers.
function initGCodeGeometry() {
  // Tear down any existing geometry
  if (extrusionLines) { scene.remove(extrusionLines); extrusionLines.geometry.dispose(); extrusionLines = null; }
  if (travelLines)    { scene.remove(travelLines);    travelLines.geometry.dispose();    travelLines = null; }
  if (frameBox)       { scene.remove(frameBox);       frameBox.geometry.dispose();       frameBox = null; }

  extrusionVertCount = 0;
  travelVertCount = 0;
  lastGeomIndex = 0;

  const totalSegs = sim.segmentCount();
  if (totalSegs === 0) return;

  // Worst case: every segment is extrusion or travel (6 floats each).
  extrusionBuf = new Float32Array(totalSegs * 6);
  travelBuf    = new Float32Array(totalSegs * 6);

  const extGeo = new THREE.BufferGeometry();
  const extAttr = new THREE.BufferAttribute(extrusionBuf, 3);
  extAttr.setUsage(THREE.DynamicDrawUsage);
  extGeo.setAttribute("position", extAttr);
  extGeo.setDrawRange(0, 0);
  extrusionLines = new THREE.LineSegments(extGeo, extrusionMaterial);
  scene.add(extrusionLines);

  const trvGeo = new THREE.BufferGeometry();
  const trvAttr = new THREE.BufferAttribute(travelBuf, 3);
  trvAttr.setUsage(THREE.DynamicDrawUsage);
  trvGeo.setAttribute("position", trvAttr);
  trvGeo.setDrawRange(0, 0);
  travelLines = new THREE.LineSegments(trvGeo, travelMaterial);
  travelLines.visible = showTravelCheck.checked;
  scene.add(travelLines);

  rebuildFrameBox();
}

// Called every frame/step to append newly revealed segments.
function rebuildGCodeGeometry() {
  if (!extrusionLines) return;
  const curIdx = sim.currentIndex();
  if (curIdx === lastGeomIndex) return;

  const newExtru = sim.extrusionSegmentsSince(lastGeomIndex);
  const newTravel = sim.travelSegmentsSince(lastGeomIndex);
  lastGeomIndex = curIdx;

  if (newExtru.length > 0) {
    extrusionBuf.set(newExtru, extrusionVertCount);
    extrusionVertCount += newExtru.length;
    const attr = extrusionLines.geometry.getAttribute("position");
    attr.needsUpdate = true;
    extrusionLines.geometry.setDrawRange(0, extrusionVertCount / 3);
  }
  if (newTravel.length > 0) {
    travelBuf.set(newTravel, travelVertCount);
    travelVertCount += newTravel.length;
    const attr = travelLines.geometry.getAttribute("position");
    attr.needsUpdate = true;
    travelLines.geometry.setDrawRange(0, travelVertCount / 3);
  }
}

function rebuildFrameBox() {
  if (frameBox) {
    scene.remove(frameBox);
    frameBox.geometry.dispose();
    frameBox = null;
  }
  const bbox = sim.boundingBox();
  if (bbox[0] >= bbox[3]) return;

  const inner = new THREE.BoxGeometry(bbox[3] - bbox[0], bbox[4] - bbox[1], bbox[5] - bbox[2]);
  frameBox = new THREE.LineSegments(new THREE.EdgesGeometry(inner), frameMaterial);
  frameBox.position.set(
    (bbox[0] + bbox[3]) / 2,
    (bbox[1] + bbox[4]) / 2,
    (bbox[2] + bbox[5]) / 2,
  );
  frameBox.visible = showFrameCheck.checked;
  scene.add(frameBox);
  inner.dispose();
}

// ── Camera helpers ────────────────────────────────────────────────────────────
function fitDefault() {
  controls.target.set(BUILD_X / 2, BUILD_Y / 4, BUILD_Z / 4);
  camera.position.set(BUILD_X * 1.8, -BUILD_Y * 1.2, BUILD_Z * 1.0);
  camera.near = 1;
  camera.far = 5000;
  controls.minDistance = 30;
  controls.maxDistance = 3000;
  camera.updateProjectionMatrix();
  controls.update();
}

function fitToPrint() {
  const bbox = sim.boundingBox();
  if (bbox[0] >= bbox[3]) { fitDefault(); return; }
  const cx = (bbox[0] + bbox[3]) / 2;
  const cy = (bbox[1] + bbox[4]) / 2;
  const cz = (bbox[2] + bbox[5]) / 2;
  const diagonal = Math.sqrt((bbox[3]-bbox[0])**2 + (bbox[4]-bbox[1])**2 + (bbox[5]-bbox[2])**2);
  const r = Math.max(diagonal * 0.6, 20);
  controls.target.set(cx, cy, cz);
  camera.position.set(cx + r * 2.0, cy - r * 2.0, cz + r * 1.4);
  camera.near = Math.max(0.5, r / 100);
  camera.far = Math.max(3000, r * 80);
  controls.minDistance = Math.max(10, r * 0.3);
  controls.maxDistance = Math.max(500, r * 15);
  camera.updateProjectionMatrix();
  controls.update();
}

function dollyCamera(deltaY) {
  const dir = camera.position.clone().sub(controls.target);
  const dist = dir.length();
  if (dist === 0) return;
  const next = clamp(dist * Math.exp(clamp(deltaY, -80, 80) * 0.0015), controls.minDistance, controls.maxDistance);
  camera.position.copy(controls.target).add(dir.multiplyScalar(next / dist));
  camera.updateProjectionMatrix();
  controls.update();
}

// ── UI helpers ────────────────────────────────────────────────────────────────
function updateSpeedDisplay() {
  const segsPerFrame = Number(speedSlider.value) / 20;
  if (segsPerFrame >= 1) {
    speedDisplay.textContent = `${segsPerFrame.toFixed(segsPerFrame < 10 ? 1 : 0)} segs/frame`;
  } else {
    speedDisplay.textContent = `1 seg / ${Math.round(1 / segsPerFrame)} frames`;
  }
}

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
