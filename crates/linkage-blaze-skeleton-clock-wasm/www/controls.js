// Live tuner for the case-photo fit. Builds a small panel of sliders + number
// inputs bound to the `.case` element's width/height/left/top (browser px), with
// a copyable CSS output box. Remove the <script> include to drop it.

const caseEl = document.querySelector(".case");
if (caseEl) {
  const FIELDS = [
    { key: "width", min: 600, max: 1600 },
    { key: "height", min: 800, max: 2000 },
    { key: "left", min: -700, max: 200 },
    { key: "top", min: -900, max: 200 },
  ];

  const panel = document.createElement("div");
  panel.style.cssText = [
    "position:fixed", "top:8px", "right:8px", "z-index:10",
    "background:rgba(20,24,30,0.92)", "color:#dfe7f2", "padding:10px 12px",
    "border-radius:8px", "font:12px/1.5 monospace", "box-shadow:0 4px 16px rgba(0,0,0,0.5)",
  ].join(";");

  const inputs = {};
  const read = (key) => parseFloat(getComputedStyle(caseEl)[key]);

  const output = document.createElement("textarea");
  output.readOnly = true;
  output.rows = 6;
  output.style.cssText = "width:230px;margin-top:6px;background:#0d1117;color:#9ee37d;border:1px solid #333;font:11px/1.4 monospace;resize:vertical";

  function refreshOutput() {
    const v = (k) => Math.round(read(k));
    output.value =
      ".stage .case {\n" +
      `  width: ${v("width")}px;\n` +
      `  height: ${v("height")}px;\n` +
      `  left: ${v("left")}px;\n` +
      `  top: ${v("top")}px;\n` +
      "}";
  }

  function setField(key, value) {
    caseEl.style[key] = `${value}px`;
    inputs[key].range.value = value;
    inputs[key].number.value = value;
    refreshOutput();
  }

  for (const { key, min, max } of FIELDS) {
    const row = document.createElement("label");
    row.style.cssText = "display:flex;align-items:center;gap:6px;white-space:nowrap";

    const name = document.createElement("span");
    name.textContent = key.padEnd(6);
    name.style.width = "44px";

    const range = document.createElement("input");
    range.type = "range";
    range.min = min; range.max = max; range.step = 1;
    range.style.width = "120px";

    const number = document.createElement("input");
    number.type = "number";
    number.min = min; number.max = max; number.step = 1;
    number.style.cssText = "width:56px;background:#0d1117;color:#dfe7f2;border:1px solid #333";

    const current = Math.round(read(key));
    range.value = current; number.value = current;

    range.addEventListener("input", () => setField(key, Number(range.value)));
    number.addEventListener("input", () => setField(key, Number(number.value)));

    inputs[key] = { range, number };
    row.append(name, range, number);
    panel.append(row);
  }

  panel.append(output);
  document.body.append(panel);
  refreshOutput();
}
