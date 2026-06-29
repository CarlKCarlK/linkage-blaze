// Live tuner for the framed device UI. Builds a panel of sliders + number inputs
// bound to the `.case` photo and the `.cord` extension (browser px), with a
// copyable CSS output box. Remove the <script> include to drop it.

const GROUPS = [
  {
    label: ".stage .case",
    selector: ".case",
    fields: [
      { key: "width", min: 600, max: 1600 },
      { key: "height", min: 800, max: 2000 },
      { key: "left", min: -700, max: 200 },
      { key: "top", min: -900, max: 200 },
    ],
  },
  {
    label: ".stage .cord",
    selector: ".cord",
    fields: [
      { key: "left", min: 100, max: 360 },
      { key: "width", min: 4, max: 120 },
    ],
  },
];

const present = GROUPS.filter((group) => document.querySelector(group.selector));
if (present.length) {
  const panel = document.createElement("div");
  panel.style.cssText = [
    "position:fixed", "top:8px", "right:8px", "z-index:10",
    "background:rgba(20,24,30,0.92)", "color:#dfe7f2", "padding:10px 12px",
    "border-radius:8px", "font:12px/1.5 monospace", "box-shadow:0 4px 16px rgba(0,0,0,0.5)",
  ].join(";");

  const output = document.createElement("textarea");
  output.readOnly = true;
  output.rows = 4 + present.reduce((n, g) => n + g.fields.length, 0);
  output.style.cssText = "width:236px;margin-top:6px;background:#0d1117;color:#9ee37d;border:1px solid #333;font:11px/1.4 monospace;resize:vertical";

  const read = (el, key) => Math.round(parseFloat(getComputedStyle(el)[key]));

  function refreshOutput() {
    output.value = present
      .map((group) => {
        const el = document.querySelector(group.selector);
        const lines = group.fields.map(({ key }) => `  ${key}: ${read(el, key)}px;`);
        return `${group.label} {\n${lines.join("\n")}\n}`;
      })
      .join("\n");
  }

  for (const group of present) {
    const el = document.querySelector(group.selector);

    const header = document.createElement("div");
    header.textContent = group.label;
    header.style.cssText = "margin-top:6px;color:#9aa7b8";
    panel.append(header);

    for (const { key, min, max } of group.fields) {
      const row = document.createElement("label");
      row.style.cssText = "display:flex;align-items:center;gap:6px;white-space:nowrap";

      const name = document.createElement("span");
      name.textContent = key;
      name.style.width = "44px";

      const range = document.createElement("input");
      range.type = "range";
      range.min = min; range.max = max; range.step = 1;
      range.style.width = "120px";

      const number = document.createElement("input");
      number.type = "number";
      number.min = min; number.max = max; number.step = 1;
      number.style.cssText = "width:56px;background:#0d1117;color:#dfe7f2;border:1px solid #333";

      const current = read(el, key);
      range.value = current; number.value = current;

      const set = (value) => {
        el.style[key] = `${value}px`;
        range.value = value;
        number.value = value;
        refreshOutput();
      };
      range.addEventListener("input", () => set(Number(range.value)));
      number.addEventListener("input", () => set(Number(number.value)));

      row.append(name, range, number);
      panel.append(row);
    }
  }

  panel.append(output);
  document.body.append(panel);
  refreshOutput();
}
