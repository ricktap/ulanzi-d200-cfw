// The Theme block under the device: the full theme, partial themes, and the
// palette colours of the theme (the core variables every key reads).
import { $, h, put } from "./dom.js";
import { state, edit, hint, settingsOf } from "./store.js";

/** Palette field → core variable it sets, and the theme's own value. */
const PALETTE = [
  ["active", "accent", "#69E0C1"],
  ["info", "info", "#B79AF7"],
  ["attention", "warning", "#FFBF69"],
  ["error", "danger", "#FF8278"],
  ["background", "background", "#12151A"],
];
const BASE = "#F4F1E8";
const MUTED = "#87909E";

export function renderTheme() {
  const t = settingsOf(state.config).theme || {};
  const pal = t.palette || {};
  const col = (k, d) => pal[k] || d;
  const swatch = h("span", { class: "swatch" }, [col("background", "#12151A"), BASE, col("active", "#69E0C1"), MUTED].map((c) => h("span", { style: `background:${c}` })));
  const vars = Object.keys(t.variables || {});
  put($("theme"),
    h("span", { class: "label" }, "Theme"),
    h("div", { class: "theme-pick", title: "Built-in theme" }, swatch,
      h("div", { class: "grow" }, h("div", {}, "Night Signal"), h("div", { class: "sub mono" }, "com.deckling.core"))),
    h("p", { class: "sub" }, "Partial themes that set single variables come with theme packages. Until then, the colours below and plugins.core.config.theme.variables in deck.json play that part."),
    vars.length ? h("p", { class: "sub" }, `deck.json sets ${vars.length} variable${vars.length > 1 ? "s" : ""}: ${vars.join(", ")}.`) : null,
    h("div", { class: "colors" }, PALETTE.map(([k, v, d]) => color(k, v, d, pal[k]))));
}

function color(field, variable, def, cur) {
  const input = h("input", { type: "color", value: cur || def, title: variable });
  input.oninput = () => edit(`Colour ${variable}`, (c) => {
    const t = (settingsOf(c).theme ||= {});
    (t.palette ||= {})[field] = input.value;
  }, { redraw: false, coalesce: `theme:${field}` });
  input.onchange = () => { hint(`${variable} is ${input.value} now.`); renderTheme(); };
  return h("div", { class: "field" },
    h("span", { class: "label" }, variable),
    input,
    cur ? h("button", { type: "button", class: "link", onclick: () => {
      edit(`Colour ${variable}`, (c) => { delete settingsOf(c).theme.palette[field]; });
      renderTheme();
    } }, "reset") : h("span", { class: "sub" }, "theme"));
}
