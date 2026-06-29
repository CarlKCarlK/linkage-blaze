import init, {
  start,
  set_time_of_day,
  show_case_alignment_controls,
} from "./pkg/linkage_blaze_skeleton_clock_wasm.js";

const status = document.querySelector("#status");

try {
  await init();
  // `start` sets the canvas pixel buffer to the real panel resolution and spawns
  // the `skeleton_clock` render loop, which paces itself via
  // `requestAnimationFrame` inside Rust and ticks once per second from the
  // browser clock. CSS stretches the canvas over the case's screen area, so no
  // JS animation loop or sizing is needed here.
  start("screen");
  buildTimeOfDaySlider(set_time_of_day);
  if (show_case_alignment_controls()) {
    await import("./controls.js");
  }
  status.textContent = "skeleton-clock running";
} catch (error) {
  status.textContent = `load failed: ${String(error)}`;
  throw error;
}

// A vertical time-of-day slider pinned to the left edge of the page. It is
// independent of the case/screen calibration panel (controls.js): it overrides
// only the simulated clock's time of day from midnight (0) to midnight (86400),
// and a "Live" button releases the override back to the real browser clock.
function buildTimeOfDaySlider(setTimeOfDay) {
  const SECONDS_PER_DAY = 86400;

  const panel = document.createElement("div");
  panel.style.cssText = [
    "position:fixed", "left:8px", "top:50%", "transform:translateY(-50%)",
    "z-index:10", "display:flex", "flex-direction:column", "align-items:center",
    "gap:8px", "background:rgba(20,24,30,0.92)", "color:#dfe7f2",
    "padding:12px 10px", "border-radius:8px", "font:12px/1.4 monospace",
    "box-shadow:0 4px 16px rgba(0,0,0,0.5)",
  ].join(";");

  const readout = document.createElement("div");
  readout.style.cssText = "min-width:64px;text-align:center";

  // Vertical slider: top = midnight (0), bottom = next midnight. A rotated
  // horizontal range is the most portable way to get a tall vertical track.
  const range = document.createElement("input");
  range.type = "range";
  range.min = 0;
  range.max = SECONDS_PER_DAY;
  range.step = 60; // one-minute granularity
  range.value = 12 * 3600; // noon
  range.style.cssText = "writing-mode:vertical-lr;direction:rtl;width:24px;height:360px";

  const live = document.createElement("button");
  live.textContent = "Live";
  live.style.cssText = "background:#0d1117;color:#9ee37d;border:1px solid #333;border-radius:4px;font:12px monospace;padding:4px 8px;cursor:pointer";

  const formatTime = (secondsOfDay) => {
    const hour = Math.floor(secondsOfDay / 3600);
    const minute = Math.floor((secondsOfDay % 3600) / 60);
    const suffix = hour < 12 ? "AM" : "PM";
    const hour12 = hour % 12 === 0 ? 12 : hour % 12;
    return `${String(hour12).padStart(2, " ")}:${String(minute).padStart(2, "0")} ${suffix}`;
  };

  const applyOverride = () => {
    const secondsOfDay = Number(range.value);
    setTimeOfDay(secondsOfDay);
    readout.textContent = formatTime(secondsOfDay);
  };

  range.addEventListener("input", applyOverride);
  live.addEventListener("click", () => {
    setTimeOfDay(-1); // release the override; resume the real clock
    readout.textContent = "Live";
  });

  panel.append(readout, range, live);
  document.body.append(panel);

  // Start live (following the real clock) rather than forcing noon on load.
  readout.textContent = "Live";
}
