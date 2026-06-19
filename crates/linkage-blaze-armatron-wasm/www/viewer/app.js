import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import init, { linkage_points } from "../pkg/linkage_blaze_armatron_wasm.js";

// todo0000 why is this here????
const PARAMS = [
  { name: "raise hand", value: 0.5 },
  { name: "bend elbow", value: 0.5 },
  { name: "close hand", value: 0.0 },
  { name: "lower arm", value: 0.5 },
  { name: "spin whole arm", value: 0.5 },
  { name: "spin hand", value: 0.5 },
];

const DEFAULT_PARAMS = PARAMS.map((param) => param.value);
const canvas = document.querySelector("#arm-canvas");
const sliders = document.querySelector("#sliders");
const resetView = document.querySelector("#reset-view");
const resetParams = document.querySelector("#reset-params");

let selectedIndex = 0;
let points = [];
let firstFit = true;

const renderer = new THREE.WebGLRenderer({
  canvas,
  antialias: true,
  alpha: false,
});
renderer.setClearColor(0xffffff, 1);
renderer.setPixelRatio(window.devicePixelRatio || 1);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0xffffff);

const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000);
camera.up.set(0, 0, 1);

const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.dampingFactor = 0.08;
controls.screenSpacePanning = true;
controls.enableZoom = false;
controls.minDistance = 0.75;
controls.maxDistance = 80;
controls.mouseButtons = {
  LEFT: THREE.MOUSE.ROTATE,
  MIDDLE: THREE.MOUSE.PAN,
  RIGHT: THREE.MOUSE.PAN,
};

const grid = new THREE.GridHelper(20, 20, 0x9ca8b4, 0xd7dce2);
grid.rotation.x = Math.PI / 2;
scene.add(grid);

const armColor = 0x156082;
const rodRadius = 0.08;
const jointRadius = rodRadius * 2;
const rodMaterial = new THREE.MeshBasicMaterial({ color: armColor });
const jointMaterial = new THREE.MeshBasicMaterial({ color: armColor });
const jointGeometry = new THREE.SphereGeometry(jointRadius, 24, 16);
const rodMeshes = [];
const joints = [];

await init();
buildControls();
resize();
update();
animate();

window.addEventListener("resize", () => {
  resize();
  render();
});

renderer.domElement.addEventListener(
  "wheel",
  (event) => {
    event.preventDefault();
    event.stopImmediatePropagation();
    zoomCamera(event.deltaY);
    render();
  },
  { capture: true, passive: false },
);

resetView.addEventListener("click", () => {
  fitView();
  render();
});

resetParams.addEventListener("click", () => {
  for (let index = 0; index < PARAMS.length; index += 1) {
    PARAMS[index].value = DEFAULT_PARAMS[index];
  }
  syncControls();
  update();
});

window.addEventListener("keydown", (event) => {
  const sliderFocused = document.activeElement?.tagName === "INPUT";
  if (!sliderFocused && event.key >= "1" && event.key <= "6") {
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
    "Left drag orbits. Right drag pans. Mouse wheel zooms. Number keys select a parameter; arrow keys nudge it.";
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
  points = unpackPoints(linkage_points(PARAMS.map((param) => param.value)));
  updateGeometry();
  if (firstFit) {
    fitView();
    firstFit = false;
  }
  render();
}

function unpackPoints(flatPoints) {
  const nextPoints = [];
  for (let index = 0; index < flatPoints.length; index += 3) {
    nextPoints.push(new THREE.Vector3(flatPoints[index], flatPoints[index + 1], -flatPoints[index + 2]));
  }
  return nextPoints;
}

function updateGeometry() {
  const segmentCount = Math.max(0, points.length - 1);
  while (rodMeshes.length < segmentCount) {
    const rod = new THREE.Mesh(new THREE.CylinderGeometry(rodRadius, rodRadius, 1, 16), rodMaterial);
    rodMeshes.push(rod);
    scene.add(rod);
  }
  while (rodMeshes.length > segmentCount) {
    const rod = rodMeshes.pop();
    scene.remove(rod);
    rod.geometry.dispose();
  }
  for (let index = 0; index < segmentCount; index += 1) {
    positionRod(rodMeshes[index], points[index], points[index + 1]);
  }

  while (joints.length < points.length) {
    const joint = new THREE.Mesh(jointGeometry, jointMaterial);
    joints.push(joint);
    scene.add(joint);
  }
  while (joints.length > points.length) {
    const joint = joints.pop();
    scene.remove(joint);
  }
  for (let index = 0; index < points.length; index += 1) {
    joints[index].position.copy(points[index]);
  }
}

function positionRod(rod, start, end) {
  const direction = end.clone().sub(start);
  const length = direction.length();
  rod.visible = length > 0;
  if (!rod.visible) {
    return;
  }
  rod.position.copy(start).add(end).multiplyScalar(0.5);
  rod.scale.set(1, length, 1);
  rod.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), direction.normalize());
}

function resize() {
  const bounds = canvas.getBoundingClientRect();
  renderer.setSize(Math.max(1, bounds.width), Math.max(1, bounds.height), false);
  camera.aspect = bounds.width / Math.max(1, bounds.height);
  camera.updateProjectionMatrix();
}

function fitView() {
  if (points.length === 0) {
    return;
  }
  const bounds = new THREE.Box3().setFromPoints(points);
  const center = bounds.getCenter(new THREE.Vector3());
  const size = bounds.getSize(new THREE.Vector3());
  const radius = Math.max(size.length() * 0.5, 2);

  controls.target.copy(center);
  camera.position.copy(center).add(new THREE.Vector3(radius * 1.8, -radius * 2.2, radius * 1.4));
  camera.near = Math.max(0.01, radius / 100);
  camera.far = radius * 100;
  controls.minDistance = Math.max(0.25, radius * 0.2);
  controls.maxDistance = Math.max(20, radius * 8);
  camera.updateProjectionMatrix();
  controls.update();
}

function zoomCamera(deltaY) {
  const direction = camera.position.clone().sub(controls.target);
  const currentDistance = direction.length();
  if (currentDistance === 0) {
    return;
  }

  const zoomFactor = Math.exp(clamp(deltaY, -80, 80) * 0.0015);
  const nextDistance = clamp(currentDistance * zoomFactor, controls.minDistance, controls.maxDistance);
  camera.position.copy(controls.target).add(direction.multiplyScalar(nextDistance / currentDistance));
  camera.updateProjectionMatrix();
  controls.update();
}

function animate() {
  requestAnimationFrame(animate);
  controls.update();
  render();
}

function render() {
  renderer.render(scene, camera);
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}
