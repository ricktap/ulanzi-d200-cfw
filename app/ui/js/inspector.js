// The inspector for the selected key: app, its settings, icon and label, the
// action sections, test press and clear.
import { $, h, put, ask } from "./dom.js";
import {
  state, STATUS_KEY, edit, hint, render, profile, keyObj, ensureKey, ensureState, keyKind, settingsOf } from "./store.js";
import { appDef, iconDef, withNs, shortRole, pluginOn, nsOf } from "./catalog.js";
import { schemaField } from "./fields.js";
import { textField } from "./controls.js";
import { actionSections, testPress } from "./actions.js";
import { appHelp, clearKey } from "./apps.js";
import { keyEl } from "./deck.js";

/** The object that carries `on` for key k (state 0 of an app key, the shown state of a button, status_key). */
export function keyHolder(k) {
  return (create) => {
    if (k === STATUS_KEY) {
      const c = state.config;
      if (!c.status_key) { if (!create) return null; c.status_key = {}; }
      return c.status_key;
    }
    const key = create ? ensureKey(k) : keyObj(k);
    const idx = keyKind(key) === "app" ? 0 : state.selState;
    if (!create) return key?.states?.[idx];
    return ensureState(key, idx);
  };
}
export const keyWhere = (k) => (k === STATUS_KEY ? "the wide key" : `key ${k}`);

const thumb = (k) => {
  const src = keyEl(k)?.querySelector("img")?.src;
  return h("div", { class: "insp-thumb" + (k === STATUS_KEY ? " wide" : "") }, src ? h("img", { alt: "", src }) : null);
};

function head(k, eyebrow, title) {
  return h("div", { class: "insp-head" }, thumb(k),
    h("div", { class: "grow" }, h("div", { class: "label" }, eyebrow), h("h2", {}, title)));
}

export function renderInspector() {
  const k = state.selected;
  const el = $("insp");
  if (state.view.remote) return put(el, head(k, "Remote page", `Key ${k}`), remoteInfo(k));
  if (k === STATUS_KEY) return put(el, head(k, "Wide key", profile().name), wideKey());
  const key = keyObj(k);
  const kind = keyKind(key);
  if (kind === "empty") {
    return put(el, head(k, `Key ${k}`, "Empty"),
      h("p", { class: "note" }, "Tap an app in the library to put it here, or drag one onto the key. An action from the library makes it a plain button."));
  }
  const app = kind === "app" ? appDef(key.app) : null;
  if (kind === "app" && (!app || !pluginOn(nsOf(key.app)))) return put(el, head(k, `Key ${k}`, "Plugin missing"), missing(k, key, !!app));
  put(el,
    head(k, `Key ${k}`, app ? app.name : "Button"),
    appBox(key, app),
    app ? settingsBox(k, key, app) : null,
    app ? null : statesBox(k, key),
    iconBox(k, key, app),
    app ? null : propsBox(k, key),
    coloursBox(k, key, app),
    actionSections(keyHolder(k), { where: keyWhere(k) }),
    h("div", { class: "row wrap" },
      h("button", { class: "btn", type: "button", onclick: testPress }, "Test press"),
      h("button", { class: "btn danger", type: "button", onclick: () => clearKey(k) }, "Clear key")));
}

function appBox(key, app) {
  const roles = app ? [...new Set([...app.roles.map(shortRole), "button"])] : ["button"];
  const help = appHelp(app);
  return h("div", { class: "box" },
    h("div", { class: "head" }, h("span", { class: "label grow" }, "App"), h("span", { class: "sub mono" }, app ? app.id : "core:button")),
    h("span", { class: "sub" }, "Fulfils: " + roles.join(", ")),
    help ? h("p", { class: "sub" }, help) : null);
}

function settingsBox(k, key, app) {
  if (!app.config?.length) return null;
  return h("div", { class: "box" },
    h("span", { class: "label" }, "Settings"),
    app.config.map((f) => schemaField(f, () => keyObj(k)?.config?.[f.key], (v, typing) => {
      edit(`${app.name} · ${f.label || f.key}`, () => {
        const key = ensureKey(k);
        if (v === undefined) {
          if (key.config) delete key.config[f.key];
          if (key.config && !Object.keys(key.config).length) delete key.config;
        } else (key.config ||= {})[f.key] = v;
      }, { redraw: false, coalesce: typing ? `config:${k}:${f.key}` : null });
    }, { labelled: true })));
}

/** The richest role the icon shares with the app. */
function readsRole(iconId, app) {
  const icon = iconDef(iconId);
  if (!icon) return null;
  const roles = [...(app?.roles || []), "button"];
  return roles.find((r) => icon.roles.includes(r)) || null;
}

function iconBox(k, key, app) {
  const idx = app ? 0 : state.selState;
  const read = () => keyObj(k)?.states?.[idx];
  const holder = () => ensureState(ensureKey(k), idx);
  const chosen = read()?.icon;
  const shown = chosen ? withNs(chosen) : app?.icon || "";
  const role = shown ? readsRole(shown, app) : null;
  const where = `key ${k}`;
  return h("div", { class: "box" },
    h("div", { class: "head" }, h("span", { class: "label grow" }, "Icon"),
      h("button", { class: "btn small", type: "button", onclick: () => { state.libTab = "icons"; render("library"); } }, "Change icon")),
    h("span", { class: "sub mono" }, shown || "none", chosen ? "" : app ? " (default)" : ""),
    role ? h("span", { class: "sub" }, "Reads: " + shortRole(role)) : null,
    textField("Label", read, holder, "label", { area: true, rows: 2, ph: app ? "Empty: the app's own text" : "Text on the key", name: "Label", where }),
    app ? null : textField("Image", read, holder, "image", { ph: "Path to a PNG or JPEG; fills the key", name: "Image", where, dropEmpty: true }));
}

// ------------------------------------------------------------------ buttons: states, icon values, colours

const isNext = (a) => a.action === "core:state";

/** A button can have several states; each has its own look and actions. "Change state" switches. */
function statesBox(k, key) {
  const n = Math.max(1, key.states?.length || 0);
  if (state.selState >= n) state.selState = 0;
  const add = () => {
    edit(`Add state · key ${k}`, () => {
      const key = ensureKey(k);
      ensureState(key, 0);
      key.states.push(structuredClone(key.states[state.selState] || {}));
      // every state switches to the next on a press, unless it already changes state
      for (const st of key.states) {
        st.on ||= {};
        const all = Object.values(st.on).flat();
        if (!all.some(isNext)) (st.on.press ||= []).push({ action: "core:state", params: { to: "next" } });
      }
      state.selState = key.states.length - 1;
    });
    hint(`Key ${k} has ${n + 1} states now. A press switches to the next one.`);
  };
  const remove = async () => {
    if (!(await ask(`Remove state ${state.selState + 1} of key ${k}?`, null))) return;
    edit(`Remove state · key ${k}`, () => {
      const key = ensureKey(k);
      key.states.splice(state.selState, 1);
      state.selState = Math.max(0, state.selState - 1);
    });
  };
  return h("div", { class: "box" },
    h("div", { class: "head" }, h("span", { class: "label grow" }, n > 1 ? `States: you edit state ${state.selState + 1} of ${n}` : "States"),
      h("button", { class: "btn small", type: "button", onclick: add }, "+ State")),
    n > 1 ? h("div", { class: "tabs" }, Array.from({ length: n }, (_, i) => h("button", {
      type: "button", class: i === state.selState ? "on" : "", onclick: () => { state.selState = i; state.slot = "press"; render("inspector", "library"); },
    }, `${i + 1}${i === 0 ? " · start" : ""}`))) : h("p", { class: "sub" }, "Add a state for a key that toggles, e.g. On and Off. Each state has its own icon, label and actions."),
    n > 1 ? h("div", { class: "row wrap" },
      h("span", { class: "sub grow" }, "The action Change state switches between them."),
      h("button", { class: "btn small danger", type: "button", onclick: remove }, "Remove this state")) : null);
}

const PROP_TYPE = { flag: "toggle", percent: "number", number: "number", color: "color", text: "text" };

/** Values for a bound icon on a button (no app feeds it): one field per property of its roles. */
function propsBox(k, key) {
  const idx = state.selState;
  const st = key.states?.[idx];
  const icon = st?.icon ? iconDef(withNs(st.icon)) : null;
  if (!icon || !icon.bound) return null;
  const fields = [];
  for (const role of icon.roles) {
    const props = state.catalog.interfaces?.[role] || {};
    for (const [prop, type] of Object.entries(props)) {
      const name = `${role}.${prop}`;
      const f = { key: name, label: `${shortRole(role)} ${prop}`, type: PROP_TYPE[type] || "text" };
      const read = () => keyObj(k)?.states?.[idx]?.props?.[name];
      const write = (v, typing) => edit(`Icon value ${name} · key ${k}`, () => {
        const s = ensureState(ensureKey(k), idx);
        if (v === undefined || v === "") {
          if (s.props) delete s.props[name];
          if (s.props && !Object.keys(s.props).length) delete s.props;
        } else (s.props ||= {})[name] = v;
      }, { redraw: false, coalesce: typing ? `prop:${k}:${idx}:${name}` : null });
      fields.push(f.type === "color" ? colorInput(f.label, read(), (v) => write(v, true), () => write(undefined)) : schemaField(f, read, write, { labelled: true }));
    }
  }
  if (!fields.length) return null;
  return h("div", { class: "box" },
    h("span", { class: "label" }, "Icon values"),
    h("p", { class: "sub" }, "No app feeds this key, so the icon shows these fixed values."),
    fields);
}

/** Background and text colour of the key (state 0 for an app key). Empty: the theme decides. */
function coloursBox(k, key, app) {
  const idx = app ? 0 : state.selState;
  const st = key.states?.[idx] || {};
  const set = (field, label) => (v) => edit(`${label} · key ${k}`, () => {
    const s = ensureState(ensureKey(k), idx);
    if (v === undefined) delete s[field];
    else s[field] = v;
  }, { redraw: false, coalesce: v === undefined ? null : `colour:${k}:${idx}:${field}` });
  return h("div", { class: "box" },
    h("span", { class: "label" }, "Colours"),
    h("div", { class: "colors" },
      colorInput("Background", st.bg, set("bg", "Background"), () => { set("bg", "Background")(undefined); render("inspector"); }),
      app ? null : colorInput("Text and icon", st.fg, set("fg", "Text colour"), () => { set("fg", "Text colour")(undefined); render("inspector"); })),
    h("p", { class: "sub" }, "Without a colour of its own, the key follows the theme."));
}

/** A colour field with "theme" when unset and "reset" when set. */
export function colorInput(label, cur, onInput, onReset) {
  const input = h("input", { type: "color", value: /^#[0-9a-f]{6}$/i.test(cur || "") ? cur : "#12151A" });
  const tail = h("button", { type: "button", class: "link" }, cur ? "reset" : "theme");
  input.oninput = () => { onInput(input.value); tail.textContent = "reset"; };
  tail.onclick = () => { if (tail.textContent === "reset") onReset(); };
  return h("div", { class: "field" }, h("span", { class: "label" }, label), input, tail);
}

function missing(k, key, installed) {
  const n = h("p", { class: "note warn" });
  n.append(installed ? "The plugin of " : "No installed package offers ", h("code", {}, key.app),
    installed ? " is switched off under Packages. The key keeps its assignment." : ". The key keeps its assignment and works again when the plugin is back.");
  return [n, h("div", { class: "row wrap" }, h("button", { class: "btn danger", type: "button", onclick: () => clearKey(k) }, "Clear key"))];
}

// ------------------------------------------------------------------ wide key

const WIDE_CONTENT = [["profile", "Profile and clock"], ["media", "Spotify or Music"], ["meeting", "Next meeting"], ["email", "Mail"], ["steam", "Steam game"], ["printer", "Printer"]];

function wideKey() {
  const sk = state.config.status_key || {};
  const app = h("select", {}, WIDE_CONTENT.map(([v, t]) => h("option", { value: v }, t)));
  app.value = sk.app || "profile";
  app.onchange = () => edit("Wide key content", (c) => { (c.status_key ||= {}).app = app.value; });
  const show = h("select", {}, [["auto", "The song while music plays, else the clock"], ["profile", "Always the clock"], ["media", "The song whenever one is loaded"]].map(([v, t]) => h("option", { value: v }, t)));
  show.value = sk.show || "auto";
  show.onchange = () => edit("Wide key content", (c) => { (c.status_key ||= {}).show = show.value; }, { redraw: false });
  return [
    h("p", { class: "sub" }, "The wide key shows one thing at a time: its content, or a card (meeting, printer, notification, mail) for a few seconds. While a card is up, a press opens it and a long press dismisses it."),
    h("div", { class: "box" },
      h("label", { class: "field" }, h("span", { class: "label" }, "Content"), app),
      (sk.app || "profile") === "profile" ? h("label", { class: "field" }, h("span", { class: "label" }, "Right side"), show) : null),
    actionSections(keyHolder(STATUS_KEY), { where: "the wide key", slots: [["press", "On press", false], ["long", "Long press", true]] }),
    h("button", { class: "btn", type: "button", onclick: testPress }, "Test press"),
  ];
}

// ------------------------------------------------------------------ remote page

function remoteInfo(k) {
  const base = settingsOf(state.config).mqtt?.base_topic || "d200";
  const cur = state.snap?.remote?.[String(k)];
  const example = `action: mqtt.publish
data:
  topic: ${base}/ha/remote-${k}/set
  retain: true
  payload: >
    {"label": "Kitchen", "icon": "light",
     "on": {{ is_state('light.kitchen', 'on') }}}`;
  const n = h("p", { class: "note" });
  n.append("Home Assistant sets this key with a retained message on ", h("code", {}, `${base}/ha/remote-${k}/set`),
    ". Presses come back as device triggers and on ", h("code", {}, `${base}/remote/${k}/action`), ".");
  return [
    n,
    h("div", { class: "field" }, h("span", { class: "label" }, "Now"), h("pre", { class: "code" }, cur ? JSON.stringify(cur, null, 2) : "Nothing set yet.")),
    h("div", { class: "field" }, h("span", { class: "label" }, "Example"), h("pre", { class: "code" }, example)),
    settingsOf(state.config).mqtt?.enabled ? null : h("p", { class: "note warn" }, "MQTT is off. Switch it on under Packages, Deck, MQTT."),
  ];
}

