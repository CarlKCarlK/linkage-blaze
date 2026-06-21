import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import init, { MocapClipWasm } from "../pkg/linkage_blaze_mocap_wasm.js";

const STRIDE = 12;
const canvas = document.querySelector("#mocap-canvas");
const loadSampleBtn = document.querySelector("#load-sample");
const playBtn = document.querySelector("#play");
const stepBtn = document.querySelector("#step");
const frameSlider = document.querySelector("#frame");
const frameLabel = document.querySelector("#frame-label");
const speedSlider = document.querySelector("#speed");
const speedLabel = document.querySelector("#speed-label");
const frameCountLabel = document.querySelector("#frame-count");
const paramCountLabel = document.querySelector("#param-count");
const error = document.querySelector("#error");

const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
renderer.setPixelRatio(window.devicePixelRatio || 1);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x0d1117);

const CAMERA_NEAR = 0.1;
const CAMERA_FAR = 10000;
const perspectiveCamera = new THREE.PerspectiveCamera(45, 1, CAMERA_NEAR, CAMERA_FAR);
const orthographicCamera = new THREE.OrthographicCamera(-10, 10, 10, -10, CAMERA_NEAR, CAMERA_FAR);
let camera = perspectiveCamera;
camera.up.set(0, 0, 1);

const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.dampingFactor = 0.08;
controls.screenSpacePanning = true;
controls.enableZoom = false;

const grid = new THREE.GridHelper(40, 40, 0x263244, 0x1c2634);
grid.rotation.x = Math.PI / 2;
scene.add(grid);

const skeletonGroup = new THREE.Group();
scene.add(skeletonGroup);

let clip = null;
let frameIndex = 0;
let playing = false;
let frameAccumulator = 0;
let lastTime = performance.now();

await init();
resize();
applyView("perspective");
updateSpeedLabel();

loadSampleBtn.addEventListener("click", loadSample);
playBtn.addEventListener("click", () => {
  playing = !playing;
  updatePlayButton();
});
stepBtn.addEventListener("click", () => {
  playing = false;
  updatePlayButton();
  setFrame(frameIndex + 1);
});
frameSlider.addEventListener("input", () => {
  playing = false;
  updatePlayButton();
  setFrame(Number(frameSlider.value));
});
speedSlider.addEventListener("input", updateSpeedLabel);
document.querySelectorAll(".view-btn").forEach((button) => {
  button.addEventListener("click", () => {
    document.querySelectorAll(".view-btn").forEach((item) => item.classList.remove("active"));
    button.classList.add("active");
    applyView(button.dataset.view);
    fitView();
  });
});
window.addEventListener("resize", resize);
canvas.addEventListener("wheel", handleWheel, { capture: true, passive: false });

requestAnimationFrame(loop);

async function loadSample() {
  try {
    error.textContent = "";
    const [asf, amc] = await Promise.all([
      fetch("./samples/cmu_01.asf").then(requireOk).then((response) => response.text()),
      fetch("./samples/cmu_01_01.amc").then(requireOk).then((response) => response.text()),
    ]);
    clip = new MocapClipWasm(asf, amc);
    frameIndex = 0;
    frameSlider.max = String(Math.max(clip.frameCount() - 1, 0));
    frameSlider.disabled = clip.frameCount() === 0;
    playBtn.disabled = clip.frameCount() === 0;
    stepBtn.disabled = clip.frameCount() === 0;
    frameCountLabel.textContent = String(clip.frameCount());
    paramCountLabel.textContent = String(clip.parameterCount());
    renderFrame();
    fitView();
  } catch (caught) {
    error.textContent = String(caught);
  }
}

function requireOk(response) {
  if (!response.ok) throw new Error(`${response.url}: ${response.status}`);
  return response;
}

function loop(time) {
  requestAnimationFrame(loop);
  const deltaSeconds = Math.min((time - lastTime) / 1000, 0.1);
  lastTime = time;

  if (playing && clip) {
    frameAccumulator += deltaSeconds * Number(speedSlider.value);
    const framesToAdvance = Math.floor(frameAccumulator);
    if (framesToAdvance > 0) {
      frameAccumulator -= framesToAdvance;
      setFrame(frameIndex + framesToAdvance);
    }
  }

  controls.update();
  renderer.render(scene, camera);
}

function setFrame(nextFrame) {
  if (!clip) return;
  frameIndex = nextFrame % Math.max(clip.frameCount(), 1);
  renderFrame();
}

function renderFrame() {
  if (!clip) return;
  const data = clip.renderFrame(frameIndex);
  rebuildSkeleton(data);
  frameSlider.value = String(frameIndex);
  frameLabel.textContent = `${frameIndex + 1} / ${clip.frameCount()}`;
}

function rebuildSkeleton(data) {
  skeletonGroup.traverse((object) => {
    if (object.isMesh || object.isLine) {
      object.geometry.dispose();
      object.material.dispose();
    }
  });
  skeletonGroup.clear();

  for (let offset = 0; offset < data.length; offset += STRIDE) {
    const type = data[offset];
    if (type === 0) addSegment(data, offset);
    else if (type === 1) addSphere(data, offset);
  }
}

function addSegment(data, offset) {
  const start = new THREE.Vector3(data[offset + 1], data[offset + 2], data[offset + 3]);
  const end = new THREE.Vector3(data[offset + 4], data[offset + 5], data[offset + 6]);
  const direction = end.clone().sub(start);
  const length = direction.length();
  if (length < 0.0001) return;

  const radius = Math.max(data[offset + 10] * 0.06, 0.045);
  const material = new THREE.MeshBasicMaterial({ color: 0xe6edf3 });
  const geometry = new THREE.CylinderGeometry(radius, radius, length, 8);
  const mesh = new THREE.Mesh(geometry, material);
  mesh.position.copy(start).lerp(end, 0.5);
  mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), direction.normalize());
  skeletonGroup.add(mesh);

  addJoint(start, radius * 1.35);
  addJoint(end, radius * 1.35);
}

function addSphere(data, offset) {
  const position = new THREE.Vector3(data[offset + 1], data[offset + 2], data[offset + 3]);
  addJoint(position, Math.max(data[offset + 10], 0.08));
}

function addJoint(position, radius) {
  const geometry = new THREE.SphereGeometry(radius, 12, 8);
  const material = new THREE.MeshBasicMaterial({ color: 0x8b949e });
  const mesh = new THREE.Mesh(geometry, material);
  mesh.position.copy(position);
  skeletonGroup.add(mesh);
}

function updatePlayButton() {
  playBtn.textContent = playing ? "Pause" : "Play";
  playBtn.classList.toggle("active", playing);
}

function updateSpeedLabel() {
  speedLabel.textContent = `${speedSlider.value} fps`;
}

function handleWheel(event) {
  event.preventDefault();
  event.stopImmediatePropagation();

  const factor = Math.exp(clamp(event.deltaY, -120, 120) * 0.002);
  if (camera.isOrthographicCamera) {
    camera.zoom = clamp(camera.zoom / factor, 0.03, 80);
  } else {
    const direction = camera.position.clone().sub(controls.target);
    const distance = clamp(direction.length() * factor, 2, 500);
    camera.position.copy(controls.target).addScaledVector(direction.normalize(), distance);
  }

  camera.near = CAMERA_NEAR;
  camera.far = CAMERA_FAR;
  camera.updateProjectionMatrix();
  controls.update();
}

function clamp(value, low, high) {
  return Math.min(Math.max(value, low), high);
}

function resize() {
  const width = canvas.clientWidth || 1;
  const height = canvas.clientHeight || 1;
  renderer.setSize(width, height, false);
  perspectiveCamera.aspect = width / height;
  perspectiveCamera.updateProjectionMatrix();
  orthographicCamera.left = -width / height * 10;
  orthographicCamera.right = width / height * 10;
  orthographicCamera.updateProjectionMatrix();
}

function fitView() {
  const box = new THREE.Box3().setFromObject(skeletonGroup);
  if (box.isEmpty()) return;
  const center = box.getCenter(new THREE.Vector3());
  const radius = Math.max(box.getBoundingSphere(new THREE.Sphere()).radius, 1);
  controls.target.copy(center);

  if (camera.isPerspectiveCamera) {
    const direction = camera.position.clone().sub(center).normalize();
    camera.position.copy(center).addScaledVector(direction, radius * 2.8);
  } else {
    camera.zoom = 10 / radius;
    const direction = camera.position.clone().sub(controls.target).normalize();
    camera.position.copy(center).addScaledVector(direction, radius * 2.5);
  }

  camera.near = CAMERA_NEAR;
  camera.far = CAMERA_FAR;
  camera.updateProjectionMatrix();
  controls.update();
}

function applyView(view) {
  const target = controls.target.lengthSq() > 0 ? controls.target : new THREE.Vector3(0, 0, 0);
  const distance = 60;

  if (view === "perspective") {
    camera = perspectiveCamera;
    camera.up.set(0, 0, 1);
    camera.position.copy(target).add(new THREE.Vector3(-35, -35, 25));
  } else {
    camera = orthographicCamera;
    camera.zoom = 1;
    if (view === "top") {
      camera.position.copy(target).add(new THREE.Vector3(0, 0, distance));
      camera.up.set(0, 1, 0);
    } else if (view === "front") {
      camera.position.copy(target).add(new THREE.Vector3(0, -distance, 0));
      camera.up.set(0, 0, 1);
    } else {
      camera.position.copy(target).add(new THREE.Vector3(-distance, 0, 0));
      camera.up.set(0, 0, 1);
    }
  }

  controls.object = camera;
  controls.target.copy(target);
  camera.lookAt(controls.target);
  camera.near = CAMERA_NEAR;
  camera.far = CAMERA_FAR;
  camera.updateProjectionMatrix();
}
