import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { CSS2DRenderer, CSS2DObject } from "three/addons/renderers/CSS2DRenderer.js";
import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from "https://esm.sh/@codemirror/view@6";
import { history, historyKeymap, defaultKeymap, toggleLineComment, indentWithTab } from "https://esm.sh/@codemirror/commands@6";
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput } from "https://esm.sh/@codemirror/language@6";
import { closeBrackets, closeBracketsKeymap } from "https://esm.sh/@codemirror/autocomplete@6";
import { rust } from "https://esm.sh/@codemirror/lang-rust@6";
import { oneDark } from "https://esm.sh/@codemirror/theme-one-dark@6";

const editorSetup = [
  lineNumbers(),
  history(),
  drawSelection(),
  highlightActiveLine(),
  indentOnInput(),
  syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
  bracketMatching(),
  closeBrackets(),
  keymap.of([
    ...defaultKeymap,
    ...historyKeymap,
    ...closeBracketsKeymap,
    { key: "Ctrl-/", mac: "Cmd-/", run: toggleLineComment },
    indentWithTab,
  ]),
];
import init, { default_program, render_program_with_params_json } from "../pkg/linkage_blaze_editor.js?v=linkage-macro-1";

const error = document.querySelector("#error");
const canvas = document.querySelector("#view");
const paramsList = document.querySelector("#params-list");
const cameraReadout = document.querySelector("#camera-readout");
const viewMode = document.querySelector("#view-mode");

await init();

let primitives = [];
let paramValues = new Map();
let renderTimer = null;
let firstFit = true;
let fitOnNextRebuild = false;

// ---- CodeMirror editor ----
const STORAGE_KEY = "linkage-blaze-source";
const editor = new EditorView({
  doc: localStorage.getItem(STORAGE_KEY) ?? default_program(),
  extensions: [
    editorSetup,
    rust(),
    oneDark,
    keymap.of([{ key: "Ctrl-/", mac: "Cmd-/", run: toggleLineComment }]),
    EditorView.theme({
      "&": { height: "100%", minHeight: "0" },
      ".cm-scroller": { overflow: "auto" },
    }),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        localStorage.setItem(STORAGE_KEY, update.state.doc.toString());
        clearTimeout(renderTimer);
        renderTimer = setTimeout(updatePreview, 140);
      }
    }),
  ],
  parent: document.querySelector("#source"),
});

const getSource = () => editor.state.doc.toString();

// ---- Three.js setup ----
const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

const labelRenderer = new CSS2DRenderer();
labelRenderer.domElement.style.position = "absolute";
labelRenderer.domElement.style.top = "0";
labelRenderer.domElement.style.pointerEvents = "none";
canvas.parentElement.appendChild(labelRenderer.domElement);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0d1118);

const perspectiveCamera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000);
const orthographicCamera = new THREE.OrthographicCamera(-8, 8, 8, -8, 0.1, 1000);
let camera = perspectiveCamera;
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

// Dolly camera on scroll (zoom UI label, exponential factor)
canvas.addEventListener("wheel", (event) => {
  event.preventDefault();
  event.stopImmediatePropagation();
  const factor = Math.exp(clamp(event.deltaY, -80, 80) * 0.0015);
  if (camera.isOrthographicCamera) {
    camera.zoom = clamp(camera.zoom / factor, 0.1, 20);
    camera.updateProjectionMatrix();
  } else {
    const dir = camera.position.clone().sub(controls.target);
    camera.position.copy(controls.target).addScaledVector(dir, factor);
  }
  controls.update();
}, { capture: true, passive: false });

// Grid in the Z=0 plane
const grid = new THREE.GridHelper(12, 12, 0x27313f, 0x27313f);
grid.rotation.x = Math.PI / 2;
scene.add(grid);

// Axes with labels
const AXIS_LENGTH = 2.4;
scene.add(axisLine([0, 0, 0], [AXIS_LENGTH, 0, 0], 0xef5454));
scene.add(axisLabel("x", [AXIS_LENGTH + 0.25, 0, 0], "#ef5454"));
scene.add(axisLine([0, 0, 0], [0, AXIS_LENGTH, 0], 0x54ef8a));
scene.add(axisLabel("y", [0, AXIS_LENGTH + 0.25, 0], "#54ef8a"));
scene.add(axisLine([0, 0, 0], [0, 0, AXIS_LENGTH], 0x54a8ef));
scene.add(axisLabel("z", [0, 0, AXIS_LENGTH + 0.25], "#54a8ef"));

// Group holding all linkage primitives; cleared on each re-render
const linkageGroup = new THREE.Group();
scene.add(linkageGroup);

const VIEW_STORAGE_KEY = "linkage-blaze-view";
const savedView = localStorage.getItem(VIEW_STORAGE_KEY) ?? "perspective-x-forward";
viewMode.value = savedView;
applyViewMode(savedView);
viewMode.addEventListener("change", () => {
  localStorage.setItem(VIEW_STORAGE_KEY, viewMode.value);
  applyViewMode(viewMode.value);
  fitView();
});

function animate() {
  requestAnimationFrame(animate);
  controls.update();
  updateCameraReadout();
  renderer.render(scene, camera);
  labelRenderer.render(scene, camera);
}
animate();

window.addEventListener("resize", resize);
resize();

// ---- Editor ----
updatePreview();

function updatePreview() {
  try {
    const overrides = buildOverridesJson();
    const data = JSON.parse(render_program_with_params_json(getSource(), overrides));
    const nextValues = new Map(data.params.map(({ name, value }) => [name, value]));
    for (const [name] of nextValues) {
      if (paramValues.has(name)) nextValues.set(name, paramValues.get(name));
    }
    paramValues = nextValues;
    rebuildSliders(data.params);
    primitives = data.primitives;
    error.textContent = "";
    fitOnNextRebuild = true;
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
  if (firstFit || fitOnNextRebuild) { fitView(); firstFit = false; fitOnNextRebuild = false; }
}

function addSegment(p) {
  const start = new THREE.Vector3(...p.start);
  const end = new THREE.Vector3(...p.end);
  const dir = end.clone().sub(start);
  const length = dir.length();
  if (length < 1e-6) return;
  const radius = modelWidthRadius(p.width);
  const geom = new THREE.CylinderGeometry(radius, radius, length, 8);
  const mat = new THREE.MeshBasicMaterial({ color: threeColor(p.color) });
  const mesh = new THREE.Mesh(geom, mat);
  mesh.position.copy(start).lerp(end, 0.5);
  mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir.normalize());
  linkageGroup.add(mesh);

  addSegmentCap(start, radius, mat);
  addSegmentCap(end, radius, mat);
}

function addSegmentCap(position, radius, material) {
  const geom = new THREE.SphereGeometry(radius, 12, 8);
  const mesh = new THREE.Mesh(geom, material.clone());
  mesh.position.copy(position);
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
  const hw = modelWidthRadius(p.width);
  const geom = new THREE.TorusGeometry(p.radius, hw, 8, 64);
  const mat = new THREE.MeshBasicMaterial({ color: threeColor(p.color), side: THREE.DoubleSide });
  const mesh = new THREE.Mesh(geom, mat);
  mesh.position.set(...p.center);
  orientToNormal(mesh, p.normal);
  linkageGroup.add(mesh);
}

function modelWidthRadius(width) {
  return modelVisibleWidth(width) / 2;
}

function modelVisibleWidth(width) {
  return Math.max(width ?? 0.1, 0.05);
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

function axisLabel(text, position, color) {
  const div = document.createElement("div");
  div.textContent = text;
  div.style.cssText = `color:${color};font:bold 13px ui-monospace,monospace;user-select:none`;
  const obj = new CSS2DObject(div);
  obj.position.set(...position);
  return obj;
}

function threeColor([r, g, b]) {
  return new THREE.Color(r, g, b);
}

function resize() {
  const bounds = canvas.getBoundingClientRect();
  const w = Math.max(1, bounds.width);
  const h = Math.max(1, bounds.height);
  renderer.setSize(w, h, false);
  labelRenderer.setSize(w, h);
  perspectiveCamera.aspect = w / h;
  perspectiveCamera.updateProjectionMatrix();

  const aspect = w / h;
  const orthoHeight = 14;
  orthographicCamera.left = -orthoHeight * aspect / 2;
  orthographicCamera.right = orthoHeight * aspect / 2;
  orthographicCamera.top = orthoHeight / 2;
  orthographicCamera.bottom = -orthoHeight / 2;
  orthographicCamera.updateProjectionMatrix();
}

function clamp(value, low, high) {
  return Math.min(Math.max(value, low), high);
}

function fitView() {
  // setFromObject forces world-matrix update before computing bounds
  const box = new THREE.Box3().setFromObject(linkageGroup);
  if (box.isEmpty()) return;

  const center = box.getCenter(new THREE.Vector3());
  const radius = Math.max(box.getBoundingSphere(new THREE.Sphere()).radius, 0.5);

  controls.target.copy(center);

  if (camera.isPerspectiveCamera) {
    const fovY = camera.fov * (Math.PI / 180);
    const fovX = 2 * Math.atan(Math.tan(fovY / 2) * camera.aspect);
    const distance = (radius / Math.sin(Math.min(fovX, fovY) / 2)) * 1.2;
    const dir = camera.position.clone().sub(center);
    const len = dir.length();
    if (len < 0.01) dir.set(-1, -1, 0.6);
    camera.position.copy(center).addScaledVector(dir.normalize(), distance);
    camera.near = Math.max(0.1, distance - radius * 3);
    camera.far = distance + radius * 3;
  } else {
    const visH = orthographicCamera.top - orthographicCamera.bottom;
    const visW = orthographicCamera.right - orthographicCamera.left;
    // Project bounding box corners onto camera up/right to get actual screen extents
    const camUp = camera.up.clone().normalize();
    const camFwd = new THREE.Vector3();
    camera.getWorldDirection(camFwd);
    const camRight = new THREE.Vector3().crossVectors(camFwd, camUp).normalize();
    const corners = [
      [box.min.x, box.min.y, box.min.z], [box.max.x, box.min.y, box.min.z],
      [box.min.x, box.max.y, box.min.z], [box.max.x, box.max.y, box.min.z],
      [box.min.x, box.min.y, box.max.z], [box.max.x, box.min.y, box.max.z],
      [box.min.x, box.max.y, box.max.z], [box.max.x, box.max.y, box.max.z],
    ].map(([x, y, z]) => new THREE.Vector3(x, y, z));
    let minU = Infinity, maxU = -Infinity, minR = Infinity, maxR = -Infinity;
    let minF = Infinity, maxF = -Infinity;
    for (const c of corners) {
      const u = c.dot(camUp); const r = c.dot(camRight); const f = c.dot(camFwd);
      if (u < minU) minU = u; if (u > maxU) maxU = u;
      if (r < minR) minR = r; if (r > maxR) maxR = r;
      if (f < minF) minF = f; if (f > maxF) maxF = f;
    }
    const fitH = (maxU - minU) * 1.5;
    const fitW = (maxR - minR) * 1.5;
    camera.zoom = Math.min(visW / Math.max(fitW, 0.001), visH / Math.max(fitH, 0.001));
    // Place camera outside the bounding sphere so tilting geometry never clips
    const safeDistance = radius + 1;
    camera.position.copy(center).addScaledVector(camFwd, -safeDistance);
    camera.near = 0.1;
    camera.far = safeDistance * 2 + (maxF - minF) + 1;
  }

  camera.updateProjectionMatrix();
  controls.update();
}

function applyViewMode(mode) {
  const target = new THREE.Vector3(0, 0, 2);
  const viewDistance = 20;

  if (mode === "perspective-x-forward" || mode === "perspective-y-forward") {
    camera = perspectiveCamera;
    camera.up.set(0, 0, 1);
    if (mode === "perspective-x-forward") {
      camera.position.set(-14.2, -2.3, 6.1);
    } else {
      camera.position.set(-2.3, -14.2, 6.1);
    }
    camera.zoom = 1;
  } else {
    camera = orthographicCamera;
    camera.zoom = 1;
    if (mode === "front-y-left") {
      // Looking along +X: Y goes left, Z goes up
      camera.position.set(target.x - viewDistance, target.y, target.z);
      camera.up.set(0, 0, 1);
    } else if (mode === "side-x-right") {
      // Looking along +Y: X goes right, Z goes up
      camera.position.set(target.x, target.y - viewDistance, target.z);
      camera.up.set(0, 0, 1);
    } else {
      // Top-down views
      camera.position.set(target.x, target.y, target.z + viewDistance);
      if (mode === "top-x-right") {
        camera.up.set(0, 1, 0);
      } else {
        camera.up.set(1, 0, 0);
      }
    }
  }

  controls.object = camera;
  controls.target.copy(target);
  camera.lookAt(controls.target);
  camera.updateProjectionMatrix();
  controls.update();
}

function updateCameraReadout() {
  const camera_to_target = controls.target.clone().sub(camera.position);
  const horizontal = Math.hypot(camera_to_target.x, camera_to_target.y);
  const yaw = radiansToDegrees(Math.atan2(camera_to_target.y, camera_to_target.x));
  const pitch = radiansToDegrees(Math.atan2(camera_to_target.z, horizontal));
  const xScreen = screenDirection(new THREE.Vector3(1, 0, 0));
  const yScreen = screenDirection(new THREE.Vector3(0, 1, 0));
  const zScreen = screenDirection(new THREE.Vector3(0, 0, 1));

  cameraReadout.textContent =
    `yaw   ${formatDegrees(yaw)}\n` +
    `pitch ${formatDegrees(pitch)}\n` +
    `x screen ${formatScreenVector(xScreen)}\n` +
    `y screen ${formatScreenVector(yScreen)}\n` +
    `z screen ${formatScreenVector(zScreen)}`;
}

function screenDirection(axis) {
  const origin = new THREE.Vector3(0, 0, 0).project(camera);
  const endpoint = axis.clone().project(camera);
  return {
    x: endpoint.x - origin.x,
    y: endpoint.y - origin.y,
  };
}

function radiansToDegrees(radians) {
  return radians * 180 / Math.PI;
}

function formatDegrees(degrees) {
  return `${degrees.toFixed(1)} deg`;
}

function formatScreenVector(vector) {
  return `(${vector.x.toFixed(2)}, ${vector.y.toFixed(2)})`;
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
      const data = JSON.parse(render_program_with_params_json(getSource(), buildOverridesJson()));
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

// ---- File handling ----
const FILE_TYPES = [{ description: "Linkage Blaze", accept: { "text/plain": [".lb.rs", ".rs"] } }];
const hasFilePicker = typeof window.showOpenFilePicker === "function";
let currentFileHandle = null;

async function dbGet(key) {
  return new Promise((res, rej) => {
    const req = indexedDB.open("linkage-blaze-files", 1);
    req.onupgradeneeded = e => e.target.result.createObjectStore("files");
    req.onsuccess = e => {
      const tx = e.target.result.transaction("files", "readonly");
      const r = tx.objectStore("files").get(key);
      r.onsuccess = () => res(r.result ?? []);
      r.onerror = () => rej(r.error);
    };
    req.onerror = () => rej(req.error);
  });
}

async function dbPut(key, value) {
  return new Promise((res, rej) => {
    const req = indexedDB.open("linkage-blaze-files", 1);
    req.onupgradeneeded = e => e.target.result.createObjectStore("files");
    req.onsuccess = e => {
      const tx = e.target.result.transaction("files", "readwrite");
      tx.objectStore("files").put(value, key);
      tx.oncomplete = res;
      tx.onerror = () => rej(tx.error);
    };
    req.onerror = () => rej(req.error);
  });
}

async function getRecent() {
  try { return await dbGet("recent"); } catch { return []; }
}

async function pushRecent(handle) {
  let list = await getRecent();
  list = list.filter(f => f.name !== handle.name);
  list.unshift({ name: handle.name, handle });
  list = list.slice(0, 5);
  try { await dbPut("recent", list); } catch { /* ignore */ }
  populateRecentSelect(list);
}

function populateRecentSelect(list) {
  const sel = document.querySelector("#recent-select");
  while (sel.options.length > 1) sel.remove(1);
  for (const { name } of list) sel.add(new Option(name, name));
}

async function loadHandle(handle) {
  const text = await (await handle.getFile()).text();
  editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: text } });
  currentFileHandle = handle;
  document.title = `${handle.name} — Linkage Blaze`;
  await pushRecent(handle);
}

async function openFile() {
  if (hasFilePicker) {
    try {
      const [handle] = await window.showOpenFilePicker({ types: FILE_TYPES });
      await loadHandle(handle);
    } catch (e) { if (e.name !== "AbortError") console.error(e); }
  } else {
    document.querySelector("#file-input").click();
  }
}

async function insertHandle(handle) {
  const text = await (await handle.getFile()).text();
  const pos = editor.state.selection.main.head;
  editor.dispatch({ changes: { from: pos, insert: text } });
  await pushRecent(handle);
}

async function insertFile() {
  if (hasFilePicker) {
    try {
      const [handle] = await window.showOpenFilePicker({ types: FILE_TYPES });
      await insertHandle(handle);
    } catch (e) { if (e.name !== "AbortError") console.error(e); }
  } else {
    document.querySelector("#file-input-insert").click();
  }
}

async function saveFile() {
  if (currentFileHandle) {
    const w = await currentFileHandle.createWritable();
    await w.write(getSource()); await w.close();
  } else {
    await saveFileAs();
  }
}

async function saveFileAs() {
  if (hasFilePicker) {
    try {
      const suggestedName = currentFileHandle ? currentFileHandle.name : "untitled.lb.rs";
      const handle = await window.showSaveFilePicker({ suggestedName, types: FILE_TYPES });
      const w = await handle.createWritable();
      await w.write(getSource()); await w.close();
      currentFileHandle = handle;
      document.title = `${handle.name} — Linkage Blaze`;
      await pushRecent(handle);
    } catch (e) { if (e.name !== "AbortError") console.error(e); }
  } else {
    const a = Object.assign(document.createElement("a"), {
      href: URL.createObjectURL(new Blob([getSource()], { type: "text/plain" })),
      download: "untitled.lb.rs",
    });
    a.click(); URL.revokeObjectURL(a.href);
  }
}

document.querySelector("#btn-open").addEventListener("click", openFile);
document.querySelector("#btn-insert").addEventListener("click", insertFile);
document.querySelector("#btn-save").addEventListener("click", saveFile);
document.querySelector("#btn-save-as").addEventListener("click", saveFileAs);

document.querySelector("#file-input").addEventListener("change", async function () {
  const file = this.files[0]; if (!file) return;
  const text = await file.text();
  editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: text } });
  this.value = "";
});

document.querySelector("#file-input-insert").addEventListener("change", async function () {
  const file = this.files[0]; if (!file) return;
  const text = await file.text();
  const pos = editor.state.selection.main.head;
  editor.dispatch({ changes: { from: pos, insert: text } });
  this.value = "";
});

const recentSelect = document.querySelector("#recent-select");
const btnRecentOpen = document.querySelector("#btn-recent-open");
const btnRecentInsert = document.querySelector("#btn-recent-insert");

recentSelect.addEventListener("change", function () {
  const hasSelection = this.value !== "";
  btnRecentOpen.disabled = !hasSelection;
  btnRecentInsert.disabled = !hasSelection;
});

async function getSelectedRecentHandle() {
  const name = recentSelect.value;
  if (!name) return null;
  const item = (await getRecent()).find(f => f.name === name);
  if (!item) return null;
  try {
    if ((await item.handle.queryPermission({ mode: "read" })) !== "granted")
      if ((await item.handle.requestPermission({ mode: "read" })) !== "granted") return null;
    return item.handle;
  } catch (e) { console.error(e); return null; }
}

btnRecentOpen.addEventListener("click", async () => {
  const handle = await getSelectedRecentHandle();
  if (handle) { recentSelect.value = ""; btnRecentOpen.disabled = true; btnRecentInsert.disabled = true; await loadHandle(handle); }
});

btnRecentInsert.addEventListener("click", async () => {
  const handle = await getSelectedRecentHandle();
  if (handle) { recentSelect.value = ""; btnRecentOpen.disabled = true; btnRecentInsert.disabled = true; await insertHandle(handle); }
});

window.addEventListener("keydown", e => {
  if (!(e.ctrlKey || e.metaKey)) return;
  if (e.key === "s") { e.preventDefault(); saveFile(); }
  if (e.key === "o") { e.preventDefault(); openFile(); }
  if (e.key === "i") { e.preventDefault(); insertFile(); }
});

getRecent().then(populateRecentSelect);
