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

const isHex = (v) => typeof v === "string" && /^#[0-9a-f]{3,8}$/i.test(v);
const swatchOf = (cols) => h("span", { class: "swatch" }, cols.map((c) => h("span", { style: `background:${isHex(c) ? c : "#3A4350"}` })));

/** Colours of a theme for its swatch: background, base, accent, muted. */
function themeColors(th, pal) {
  if (!th) return [pal.background || "#12151A", BASE, pal.active || "#69E0C1", MUTED];
  const v = th.variables || {};
  return [v.background, v.base, v.accent, v.muted];
}

let themeOpen = false;

export function renderTheme() {
  const t = settingsOf(state.config).theme || {};
  const pal = t.palette || {};
  const themes = state.catalog.themes || [];
  const full = themes.filter((x) => x.kind === "full");
  const partial = themes.filter((x) => x.kind === "partial");
  const curId = state.config.theme || null;
  const cur = full.find((x) => x.id === curId) || null;
  const row = (th) => h("button", {
    type: "button", class: "theme-pick" + ((th?.id || null) === (cur?.id || null) ? " on" : ""),
    onclick: () => {
      edit(`Theme ${th ? th.name : "Night Signal"}`, (c) => { if (th) c.theme = th.id; else delete c.theme; });
      themeOpen = false;
      hint(`Theme ${th ? th.name : "Night Signal"}.`);
    },
  }, swatchOf(themeColors(th, pal)), h("div", { class: "grow" }, h("div", {}, th ? th.name : "Night Signal"), h("div", { class: "sub mono" }, th ? th.id : "built in, colours below")));
  const curRow = h("button", { type: "button", class: "theme-pick", "aria-expanded": themeOpen, onclick: () => { themeOpen = !themeOpen; renderTheme(); } },
    swatchOf(themeColors(cur, pal)),
    h("div", { class: "grow" }, h("div", {}, cur ? cur.name : "Night Signal"), h("div", { class: "sub mono" }, cur ? cur.id : "com.deckling.core")),
    h("span", { class: "sub" }, themeOpen ? "▴" : "▾"));
  if (curId && !cur) curRow.querySelector(".sub.mono").textContent = `${curId} (missing: Night Signal applies)`;
  put($("theme"),
    h("span", { class: "label" }, "Theme"),
    curRow,
    themeOpen ? h("div", { class: "theme-menu" }, row(null), full.map(row)) : null,
    overlaysEditor(partial),
    cur ? null : h("div", { class: "colors" }, PALETTE.map(([k, v, d]) => color(k, v, d, pal[k]))),
    variablesEditor(t.variables || {}));
}

// ------------------------------------------------------------------ overlays

/** Partial themes: a switch each, in the deck's order (drag or arrows); the lower one wins. */
function overlaysEditor(partial) {
  if (!partial.length) return null;
  const order = state.config.overlays || [];
  // active ones in their order first, then the others
  const list = [...order.map((id) => partial.find((p) => p.id === id) || { id, name: id, missing: true }), ...partial.filter((p) => !order.includes(p.id))];
  const setOrder = (ids, label) => edit(label, (c) => { if (ids.length) c.overlays = ids; else delete c.overlays; });
  const move = (id, to) => {
    const ids = order.filter((x) => x !== id);
    ids.splice(Math.max(0, Math.min(to, ids.length)), 0, id);
    setOrder(ids, `Overlay order`);
  };
  let dragged = null;
  return h("div", { class: "field" },
    h("span", { class: "label" }, "Partial themes"),
    h("p", { class: "sub" }, "Each sets single variables over the theme. When two set the same one, the lower one wins. Drag to change the order."),
    list.map((th) => {
      const on = order.includes(th.id);
      const i = order.indexOf(th.id);
      const sw = h("input", { type: "checkbox", class: "switch", checked: on, "aria-label": `${th.name} ${on ? "off" : "on"}` });
      sw.onchange = () => {
        setOrder(sw.checked ? [...order, th.id] : order.filter((x) => x !== th.id), `${th.name} ${sw.checked ? "on" : "off"}`);
        hint(`${th.name} is ${sw.checked ? "on" : "off"}.`);
      };
      const what = Object.keys(th.variables || {}).map((k) => {
        const v = th.variables[k];
        return typeof v === "object" && v?.bind ? `${k} from ${v.bind}` : `${k}`;
      }).join(", ");
      return h("div", {
        class: "overlay" + (on ? " on" : ""), draggable: on,
        ondragstart: (e) => { dragged = th.id; e.dataTransfer.setData("text/plain", "overlay:" + th.id); },
        ondragover: (e) => { if (on && dragged) e.preventDefault(); },
        ondrop: (e) => { e.preventDefault(); if (dragged && dragged !== th.id) move(dragged, i); dragged = null; },
      },
      sw,
      h("div", { class: "grow" }, h("div", {}, th.name + (th.missing ? " (missing)" : "")), h("div", { class: "sub" }, what ? "Sets " + what : th.id)),
      on && i > 0 ? h("button", { class: "icon-btn", type: "button", title: "Up", onclick: () => move(th.id, i - 1) }, "↑") : null,
      on && i < order.length - 1 ? h("button", { class: "icon-btn", type: "button", title: "Down", onclick: () => move(th.id, i + 1) }, "↓") : null);
    }));
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
