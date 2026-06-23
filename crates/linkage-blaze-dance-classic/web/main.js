import init, { DanceClockSim } from "./pkg/linkage_blaze_dance_classic.js";

const canvas = document.querySelector("#screen");
const context = canvas.getContext("2d");
const status = document.querySelector("#status");

try {
  await init();
} catch (error) {
  status.textContent = `load failed: ${String(error)}`;
  throw error;
}

const sim = new DanceClockSim();
canvas.width = sim.width();
canvas.height = sim.height();
const image = context.createImageData(sim.width(), sim.height());
let lastKey = "";
const fixedTime = fixedTimeFromQuery();
const fixedParams = fixedParamsFromQuery();

// Test-only scrub slider: covers midnight (0) to noon (43200 seconds). Once the
// user moves it, the clock runs forward from the scrubbed point until "Live" is
// pressed. SCRUB_SPAN seconds = 12 hours.
const SCRUB_SPAN = 43200;
const slider = document.querySelector("#time-slider");
const scrubTime = document.querySelector("#scrub-time");
const scrubLive = document.querySelector("#scrub-live");
// null = follow the real clock; otherwise the seconds-since-midnight the slider
// was last set to, plus the wall-clock time when it was set.
let scrubBase = null;
let scrubSetAt = 0;

slider.addEventListener("input", () => {
  scrubBase = Number(slider.value);
  scrubSetAt = performance.now();
});

scrubLive.addEventListener("click", () => {
  scrubBase = null;
});

status.textContent = `${sim.width()} x ${sim.height()}`;
requestAnimationFrame(tick);

function tick() {
  if (fixedParams) {
    sim.renderParams(fixedParams);
    image.data.set(sim.rgba());
    context.putImageData(image, 0, 0);
    return;
  }

  const time = currentTime();
  const key = `${time.hours}:${time.minutes}:${time.seconds}`;
  if (key !== lastKey) {
    lastKey = key;
    sim.renderTime(time.hours, time.minutes, time.seconds);
    image.data.set(sim.rgba());
    context.putImageData(image, 0, 0);
  }
  requestAnimationFrame(tick);
}

function currentTime() {
  if (scrubBase !== null) {
    const elapsed = (performance.now() - scrubSetAt) / 1000;
    const total = (scrubBase + elapsed) % SCRUB_SPAN;
    slider.value = String(Math.floor(total));
    const time = secondsToTime(total);
    scrubTime.textContent = formatTime(time);
    return time;
  }

  const time = fixedTime ?? timeFromDate(new Date());
  scrubTime.textContent = formatTime(time);
  return time;
}

function secondsToTime(totalSeconds) {
  const whole = Math.floor(totalSeconds);
  return {
    hours: Math.floor(whole / 3600),
    minutes: Math.floor((whole % 3600) / 60),
    seconds: whole % 60,
  };
}

function formatTime(time) {
  const pad = (value) => String(value).padStart(2, "0");
  return `${pad(time.hours)}:${pad(time.minutes)}:${pad(time.seconds)}`;
}

function fixedParamsFromQuery() {
  const rawParams = new URLSearchParams(location.search).get("params");
  if (!rawParams) {
    return null;
  }
  const params = rawParams.split(",").map((part) => Number(part));
  if (params.length !== 3 || params.some((param) => !Number.isFinite(param))) {
    status.textContent = `bad params: ${rawParams}`;
    return null;
  }
  return params;
}

function fixedTimeFromQuery() {
  const rawTime = new URLSearchParams(location.search).get("time");
  if (!rawTime) {
    return null;
  }
  const match = rawTime.match(/^(\d{1,2}):(\d{2})(?::(\d{2}))?$/);
  if (!match) {
    status.textContent = `bad time: ${rawTime}`;
    return null;
  }
  return {
    hours: Number(match[1]),
    minutes: Number(match[2]),
    seconds: Number(match[3] ?? "0"),
  };
}

function timeFromDate(date) {
  return {
    hours: date.getHours(),
    minutes: date.getMinutes(),
    seconds: date.getSeconds(),
  };
}
