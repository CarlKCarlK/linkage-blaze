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
let lastSecond = -1;
const fixedTime = fixedTimeFromQuery();
const fixedParams = fixedParamsFromQuery();

status.textContent = `${sim.width()} x ${sim.height()}`;
requestAnimationFrame(tick);

function tick() {
  if (fixedParams) {
    sim.renderParams(fixedParams);
    image.data.set(sim.rgba());
    context.putImageData(image, 0, 0);
    return;
  }

  const time = fixedTime ?? timeFromDate(new Date());
  if (time.seconds !== lastSecond) {
    lastSecond = time.seconds;
    sim.renderTime(time.hours, time.minutes, time.seconds);
    image.data.set(sim.rgba());
    context.putImageData(image, 0, 0);
  }
  requestAnimationFrame(tick);
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
