// The Theme block under the device: the full theme, partial themes, and the
// palette colours of the theme (the core variables every key reads).
import { $, h, put, ask } from "./dom.js";
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
  put($("theme"),
    h("span", { class: "label" }, "Theme"),
    h("div", { class: "theme-pick", title: "Built-in theme" }, swatch,
      h("div", { class: "grow" }, h("div", {}, "Night Signal"), h("div", { class: "sub mono" }, "com.deckling.core"))),
    h("p", { class: "sub" }, "Partial themes come with theme packages. Until then, the palette and the variables below play their part."),
    h("div", { class: "colors" }, PALETTE.map(([k, v, d]) => color(k, v, d, pal[k]))),
    variablesEditor(t.variables || {}));
}

// ------------------------------------------------------------------ variables

const CORE_VARS = ["accent", "background", "base", "muted", "info", "warning", "danger"];
const isColorSignal = (s) => s.type === "color";

/**
 * Theme variables over the palette: a colour, or a binding to a signal. A colour signal is
 * used as it is; any other signal picks a colour from steps (e.g. 0 to 100 % in five colours).
 */
function variablesEditor(vars) {
  // templates like com.deckling.ha:{id}.color are not signals yet
  const signals = (state.catalog.signals || []).map((s) => ({ name: s.name || s.id, type: s.type || s.kind })).filter((s) => !s.name.includes("{"));
  const write = (name, value, label) => edit(label || `Variable ${name}`, (c) => {
    const t = (settingsOf(c).theme ||= {});
    t.variables ||= {};
    if (value === undefined) delete t.variables[name];
    else t.variables[name] = value;
  }, { redraw: false, coalesce: value === undefined ? null : `var:${name}` });
  const rows = Object.entries(vars).map(([name, v]) => varRow(name, v, signals, write));
  const pick = h("select", {}, h("option", { value: "" }, "Add a variable…"),
    CORE_VARS.filter((n) => !(n in vars)).map((n) => h("option", { value: n }, n)),
    h("option", { value: "*" }, "Other name…"));
  pick.onchange = async () => {
    let name = pick.value;
    if (name === "*") name = (await ask("Variable name, e.g. com.deckling.ha:ring", "")) || "";
    if (!name) return renderTheme();
    write(name, "#69E0C1", `Add variable ${name}`);
    hint(`${name} is set by this deck now.`);
    renderTheme();
  };
  return h("div", { class: "field" },
    h("span", { class: "label" }, "Variables"),
    h("p", { class: "sub" }, "Set a variable to a colour, or bind it to a signal: then every key that reads it follows the signal, without any plugin setting a colour."),
    rows,
    pick);
}

function varRow(name, v, signals, write) {
  const bound = v && typeof v === "object";
  const mode = h("select", {},
    h("option", { value: "color" }, "Colour"),
    h("option", { value: "signal" }, "From a signal"));
  mode.value = bound ? "signal" : "color";
  mode.onchange = () => {
    if (mode.value === "color") write(name, (bound && v.fallback) || "#69E0C1");
    else write(name, { bind: signals.find(isColorSignal)?.name || signals[0]?.name || "time.tick", fallback: typeof v === "string" ? v : "#69E0C1" });
    renderTheme();
  };
  const remove = h("button", { class: "icon-btn", type: "button", title: `Remove ${name}`, onclick: () => { write(name, undefined, `Remove variable ${name}`); renderTheme(); } }, "✕");
  let body;
  if (!bound) {
    const c = h("input", { type: "color", value: /^#[0-9a-f]{6}$/i.test(v) ? v : "#69E0C1" });
    c.oninput = () => write(name, c.value);
    body = [c];
  } else {
    const sig = h("select", {}, signals.map((s) => h("option", { value: s.name }, `${s.name}${s.type ? " (" + s.type + ")" : ""}`)));
    if (!signals.some((s) => s.name === v.bind)) sig.prepend(h("option", { value: v.bind }, v.bind));
    sig.value = v.bind;
    sig.onchange = () => { write(name, { ...v, bind: sig.value }); renderTheme(); };
    const fb = h("input", { type: "color", value: /^#[0-9a-f]{6}$/i.test(v.fallback || "") ? v.fallback : "#69E0C1", title: "Fallback: while the signal has no value" });
    fb.oninput = () => write(name, { ...v, fallback: fb.value });
    const sigType = signals.find((s) => s.name === v.bind)?.type;
    const steps = Array.isArray(v.steps) ? v.steps : [];
    const stepEls = steps.map((col, i) => {
      const c = h("input", { type: "color", value: col, title: `Step ${i + 1}` });
      c.oninput = () => { const st = [...steps]; st[i] = c.value; write(name, { ...v, steps: st }); };
      return c;
    });
    const addStep = h("button", { class: "btn small", type: "button", onclick: () => { write(name, { ...v, steps: [...steps, steps.at(-1) || "#69E0C1"] }, `Step for ${name}`); renderTheme(); } }, "+ Step");
    const dropStep = steps.length ? h("button", { class: "btn small", type: "button", onclick: () => {
      const st = steps.slice(0, -1);
      const nv = { ...v, steps: st };
      if (!st.length) delete nv.steps;
      write(name, nv, `Step for ${name}`);
      renderTheme();
    } }, "− Step") : null;
    body = [sig,
      h("div", { class: "row wrap" }, h("span", { class: "sub" }, "Fallback"), fb),
      sigType === "color" && !steps.length
        ? h("div", { class: "row wrap" }, h("span", { class: "sub" }, "The signal's colour is used."), addStep)
        : h("div", { class: "row wrap" }, h("span", { class: "sub" }, steps.length ? "Steps (low to high)" : "Pick colours by value:"), stepEls, addStep, dropStep)];
  }
  return h("div", { class: "box" },
    h("div", { class: "head" }, h("b", { class: "grow mono" }, name), mode, remove),
    body);
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
