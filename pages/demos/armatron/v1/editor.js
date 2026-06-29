import init, { CydSim } from "./pkg/linkage_blaze_armatron_wasm.js";

await init();

const canvas = document.getElementById("screen");
const context = canvas.getContext("2d");
const paramsDiv = document.getElementById("params");
const rkToggle = document.getElementById("rk-toggle");
const rkStep = document.getElementById("rk-step");
const distanceDisplay = document.getElementById("distance-display");
const prevTarget = document.getElementById("prev-target");
const nextTarget = document.getElementById("next-target");
const targetLabel = document.getElementById("target-label");

const sim = new CydSim();
const image = context.createImageData(sim.width(), sim.height());
const paramCount = CydSim.paramCount();

// Build sliders from linkage param definitions
const sliders = [];
let targetSeed = 0;

// Split params into arm params (0..7) and target params (8..13)
const ARM_PARAM_END = 8;

for (let i = 0; i < paramCount; i++) {
  if (i === ARM_PARAM_END) {
    const sep = document.createElement("div");
    sep.className = "param-separator";
    paramsDiv.appendChild(sep);

    const label = document.createElement("div");
    label.className = "param-name";
    label.textContent = "— target params —";
    paramsDiv.appendChild(label);
  }

  const name = CydSim.paramName(i);
  const def = CydSim.paramDefault(i);

  const row = document.createElement("div");
  row.className = "param-row";

  const header = document.createElement("div");
  header.className = "param-header";

  const nameSpan = document.createElement("span");
  nameSpan.className = "param-name";
  nameSpan.textContent = name;

  const valueSpan = document.createElement("span");
  valueSpan.className = "param-value";
  valueSpan.textContent = def.toFixed(3);

  header.appendChild(nameSpan);
  header.appendChild(valueSpan);

  const slider = document.createElement("input");
  slider.type = "range";
  slider.min = 0;
  slider.max = 1;
  slider.step = 0.001;
  slider.value = def;

  slider.addEventListener("input", () => {
    const v = parseFloat(slider.value);
    sim.setParam(i, v);
    valueSpan.textContent = v.toFixed(3);
    render();
    updateDistance();
  });

  sliders.push({ slider, valueSpan });
  row.appendChild(header);
  row.appendChild(slider);
  paramsDiv.appendChild(row);
}

function syncSliders() {
  for (let i = 0; i < paramCount; i++) {
    const v = sim.getParam(i);
    sliders[i].slider.value = v;
    sliders[i].valueSpan.textContent = v.toFixed(3);
  }
}

function render() {
  sim.drawViewOnly();
  image.data.set(sim.rgba());
  context.putImageData(image, 0, 0);
}

function updateDistance() {
  distanceDisplay.textContent = `distance: ${sim.target_distance().toFixed(3)}`;
}

// IK controls
rkToggle.addEventListener("click", () => {
  if (sim.is_reverse_kinematics_running()) {
    sim.stop_reverse_kinematics();
    rkToggle.textContent = "▶ Run IK";
    rkToggle.classList.remove("active");
  } else {
    sim.start_reverse_kinematics();
    rkToggle.textContent = "⏹ Stop IK";
    rkToggle.classList.add("active");
  }
  render();
});

rkStep.addEventListener("click", () => {
  sim.stop_reverse_kinematics();
  rkToggle.textContent = "▶ Run IK";
  rkToggle.classList.remove("active");
  sim.tick_reverse_kinematics_at(performance.now() * 1000);
  syncSliders();
  render();
  updateDistance();
});

// Target controls
prevTarget.addEventListener("click", () => {
  targetSeed = (targetSeed - 1 + 256) % 256;
  updateTargetLabel();
  // Touch the sim's prev button via canvas event simulation is awkward;
  // instead expose a direct WASM call in the future.
  // For now, reload with new seed via the canvas-based prev button area.
  syncSliders();
  render();
  updateDistance();
});

nextTarget.addEventListener("click", () => {
  targetSeed = (targetSeed + 1) % 256;
  updateTargetLabel();
  syncSliders();
  render();
  updateDistance();
});

function updateTargetLabel() {
  targetLabel.textContent = `target #${targetSeed}`;
}

// Animation loop
function scheduleFrame() {
  requestAnimationFrame(tickFrame);
}

function tickFrame(timestamp) {
  const changed = sim.tick_at(Math.round(timestamp * 1000));
  if (changed) {
    syncSliders();
    render();
    updateDistance();
    if (!sim.is_reverse_kinematics_running()) {
      rkToggle.textContent = "▶ Run IK";
      rkToggle.classList.remove("active");
    }
  }
  scheduleFrame();
}

// Initial render
render();
updateDistance();
scheduleFrame();
