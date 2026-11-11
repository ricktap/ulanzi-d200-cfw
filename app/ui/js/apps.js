// App keys: placing an app, its settings (from the app's config schema), its
// icon, the "Plugin missing" case, and the Apps view.
import { $, h, put, toast } from "./dom.js";
import { state, edit, render, profile, keyObj, ensureKey, ensureState, editableKey } from "./store.js";
import { appDef, placeableApps, schemaDefaults, iconUrl, withNs } from "./catalog.js";
import { schemaField } from "./fields.js";
import { img, appIconPicker } from "./icons.js";
import { colorField } from "./controls.js";
import { openSettings, showThemeWidget } from "./settings.js";

const DEFAULT_BG = "#0f1216";

/** Plain text from the package; never HTML, since packages may come from anyone. */
export const appHelp = (app) => (app ? app.description || "" : "");

/**
 * Put app `id` on the selected key: sets `app` and `config` from the schema
 * defaults; state 0 keeps its label, colours and actions but loses the chosen
 * icon (the app's default applies). Other key fields stay.
 */
export function placeApp(id, k = state.selected) {
  const app = appDef(id);
  edit(`${app?.name || id} on key ${k}`, () => {
    const key = ensureKey(k);
    const wasApp = key.app && key.app !== "core:button";
    key.app = id;
    const config = schemaDefaults(app?.config);
    if (Object.keys(config).length) key.config = config;
    else delete key.config;
    if (wasApp && key.states?.length) {
      delete key.states[0].icon;
    } else {
      delete key.states; // a button's states mean nothing on an app key
    }
    state.selState = 0;
  });
}

/** Design tab content for an app key. */
export function appDesign(key) {
  const k = state.selected;
  const app = appDef(key.app);
  const parts = [];
  const sel = h("select", {},
    app ? null : h("option", { value: key.app, disabled: true }, `Missing: ${key.app}`),
    placeableApps().map((a) => h("option", { value: a.id }, a.name)));
  sel.value = key.app;
  sel.onchange = () => placeApp(sel.value);
  parts.push(h("label", { class: "field" }, h("span", { class: "label" }, "App"), sel));

  if (!app) {
    parts.push(
      h("p", { class: "note", html: `<b>Plugin missing.</b> This key uses <code></code>, which no installed package offers. The key is kept as it is.` }),
      h("div", { class: "row wrap" }, h("button", { class: "btn small danger-ghost", type: "button", onclick: () => clearKey() }, "Clear key")),
    );
    parts[1].querySelector("code").textContent = key.app;
    return parts;
  }

  const help = appHelp(app);
  if (help) parts.push(h("p", { class: "note" }, help));

  // Settings from the app's config schema.
  const fields = (app.config || []).map((f) =>
    schemaField(f, () => keyObj(k)?.config?.[f.key], (v, typing) => {
      edit(`${app.name} · ${f.label || f.key}`, () => {
        const key = ensureKey(k);
        if (v === undefined) {
          if (key.config) delete key.config[f.key];
          if (key.config && !Object.keys(key.config).length) delete key.config;
        } else (key.config ||= {})[f.key] = v;
      }, { redraw: false, coalesce: typing ? `config:${k}:${f.key}` : null });
    }, { labelled: true }));
  if (fields.length) parts.push(h("div", { class: "box" }, h("span", { class: "label" }, "Settings"), fields));

  parts.push(appIconField(app, key));
  parts.push(h("div", { class: "row" }, colorField("Background", () => keyObj(k)?.states?.[0], () => ensureState(ensureKey(k), 0), "bg", DEFAULT_BG, `key ${k}`)));

  const wid = themeWidget(key);
  if (wid) parts.push(h("p", { class: "sub" }, "Colours, light and layout: ",
    h("a", { onclick: async () => { await openSettings(); showThemeWidget(wid); } }, "Theme → " + wid)));
  return parts;
}

function themeWidget(key) {
  if (key.app === "com.deckling.core:clock") return "clock";
  if (key.app !== "com.deckling.core:sysmon") return null;
  const m = key.config?.modes;
  return Array.isArray(m) && m.length === 1 ? (m[0] === "net" ? "network" : m[0]) : "cpu";
}

/** The chosen icon of an app key: only icons sharing a role with the app. */
function appIconField(app, key) {
  const k = state.selected;
  const chosen = key.states?.[0]?.icon;
  const box = h("div", { hidden: true });
  const pick = (id) => {
    edit(`Icon · key ${k}`, () => {
      const key = ensureKey(k);
      const st = ensureState(key, 0);
      if (id) st.icon = id;
      else delete st.icon;
      if (key.states.length === 1 && !Object.keys(st).length) delete key.states;
    });
  };
  return h("div", { class: "field" },
    h("span", { class: "label" }, "Icon"),
    h("span", { class: "pathbox" },
      h("img", { class: "ithumb", alt: "", src: iconUrl(chosen) || iconUrl(app.icon) || iconUrl("grid") }),
      h("span", { class: "grow sub" }, chosen ? withNs(chosen) : `Default · ${app.icon || "built-in"}`),
      h("button", { class: "btn small", type: "button", onclick: () => {
        if (!box.firstChild) box.append(appIconPicker(app, k, chosen, pick));
        box.hidden = !box.hidden;
      } }, "Change icon…")),
    box);
}

export function clearKey(k = state.selected) {
  edit(`Clear key ${k}`, () => {
    delete profile().pages[state.view.page][String(k)];
    state.selState = 0;
  });
}

// ------------------------------------------------------------------ Apps view

export function renderApps() {
  const k = state.selected;
  $("apps-target").textContent = editableKey() ? `Selected: key ${k}` : "Select one of keys 0–12 first";
  put($("app-cards"), placeableApps().map((a) =>
    h("div", { class: "card acard" }, img(a.icon),
      h("div", { class: "grow" }, h("h3", {}, a.name), h("p", { class: "sub" }, appHelp(a)),
        h("button", { class: "btn small", disabled: !editableKey(), onclick: () => {
          placeApp(a.id);
          toast(`${a.name} on key ${state.selected}`);
          render("pane:editor");
        } }, `Use on key ${k}`)))));
}
