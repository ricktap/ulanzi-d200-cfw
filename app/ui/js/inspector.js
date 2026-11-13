// The inspector for the selected key, top to bottom: what it shows (app and its
// settings, icons, text, colours), then what it does (actions), test press and clear.
import { $, h, put, ask } from "./dom.js";
import {
  state, STATUS_KEY, edit, hint, render, profile, keyObj, ensureKey, ensureState, keyKind, settingsOf } from "./store.js";
import { appDef, iconDef, withNs, shortRole, pluginOn, nsOf, isSetIcon } from "./catalog.js";
import { schemaField } from "./fields.js";
import { textField } from "./controls.js";
import { actionSections, testPress } from "./actions.js";
import { appHelp, clearKey, placeApp } from "./apps.js";
import { keyEl } from "./deck.js";
import { iconImg } from "./library.js";
import { pickIcon, niceIcon } from "./picker.js";

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
  // the image arrives with the key's preview (deck.js renderPreview fills it in)
  return h("div", { class: "insp-thumb" + (k === STATUS_KEY ? " wide" : "") }, h("img", { alt: "", src: src || "data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==" }));
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
    return put(el, head(k, `Key ${k}`, "Empty key"),
      h("div", { class: "box" },
        h("b", {}, "Give this key something to show"),
        h("p", { class: "sub" }, "Pick an app from the list on the left: tap it, or drag it onto the key."),
        h("p", { class: "sub" }, "For a key that only runs actions (open a program, switch the page), start with a plain button."),
        h("button", { class: "btn", type: "button", onclick: () => placeApp("core:button", k) }, "Make it a plain button")));
  }
  const app = kind === "app" ? appDef(key.app) : null;
  if (kind === "app" && (!app || !pluginOn(nsOf(key.app)))) return put(el, head(k, `Key ${k}`, "Plugin missing"), missing(k, key, !!app));
  put(el,
    head(k, `Key ${k}`, app ? app.name : "Button"),
    step("Shows"),
    showsBox(k, key, app),
    app ? null : statesBox(k, key),
    iconBox(k, key, app),
    textBox(k, key, app),
    app ? null : propsBox(k, key),
    coloursBox(k, key, app),
    step("Does", app ? "Actions run in addition to what the app does by itself." : "What happens when the key is pressed."),
    actionSections(keyHolder(k), { where: keyWhere(k) }),
    h("div", { class: "row wrap" },
      h("button", { class: "btn", type: "button", onclick: testPress }, "Test press"),
      h("button", { class: "btn danger", type: "button", onclick: () => clearKey(k) }, "Clear key")),
    detailsBox(key, app));
}

/** A heading between the two halves of the inspector: what the key shows, what it does. */
const step = (title, sub) => h("div", { class: "step-title" }, h("h3", {}, title), sub ? h("span", { class: "sub" }, sub) : null);

/** The app with its description and its own settings. */
function showsBox(k, key, app) {
  if (!app) {
    return h("div", { class: "box" },
      h("b", {}, "Button"),
      h("p", { class: "sub" }, "An icon and a text of your choice. It does what you add under Does."));
  }
  const help = appHelp(app);
  const fields = app.config || [];
  return h("div", { class: "box" },
    h("b", {}, app.name),
    help ? h("p", { class: "sub" }, help) : null,
    fields.map((f) => configField(k, app, f)),
    h("p", { class: "sub" }, "Another app: pick it on the left. The label and your actions stay."));
}

const configField = (k, app, f) => schemaField(f, () => keyObj(k)?.config?.[f.key], (v, typing) => {
  edit(`${app.name} · ${f.label || f.key}`, () => {
    const key = ensureKey(k);
    if (v === undefined) {
      if (key.config) delete key.config[f.key];
      if (key.config && !Object.keys(key.config).length) delete key.config;
    } else (key.config ||= {})[f.key] = v;
  }, { redraw: false, coalesce: typing ? `config:${k}:${f.key}` : null });
}, { labelled: true });

/** Every app key: the text under the icon (the status pill or the value). */
const TEXT_BELOW = {
  key: "pill", label: "Under the icon", type: "select", default: "auto",
  values: [
    { label: "Automatic", value: "auto" },
    { label: "Nothing", value: "none" },
    { label: "On / Off", value: "state" },
    { label: "The value (Off while off)", value: "value" },
  ],
};

/** Label, and for apps what stands under the icon. */
function textBox(k, key, app) {
  const idx = app ? 0 : state.selState;
  const read = () => keyObj(k)?.states?.[idx];
  const holder = () => ensureState(ensureKey(k), idx);
  return h("div", { class: "box" },
    h("span", { class: "label" }, "Text"),
    textField("Label", read, holder, "label", { area: true, rows: 2, ph: app ? "Empty: the app's own text" : "Text on the key", name: "Label", where: `key ${k}` }),
    app ? configField(k, app, TEXT_BELOW) : null);
}

/** One line of the Icon box: what it is for, the icon now, Change and back to the default. */
function iconRow({ title, note, icon, k, live, placeholder, onChange, onReset, resetLabel }) {
  return h("div", { class: "icon-row" },
    h("span", { class: "thumb" }, icon ? (isSetIcon(icon) || iconDef(icon) ? iconImg(icon, k, live) : h("span", { class: "emoji" }, icon)) : h("span", { class: "noicon" }, "–")),
    h("span", { class: "grow" }, h("div", {}, title), h("div", { class: "sub" }, note || (icon ? niceIcon(icon) : placeholder))),
    h("button", { class: "btn small", type: "button", onclick: onChange }, "Change"),
    onReset ? h("button", { class: "btn small", type: "button", onclick: onReset }, resetLabel) : null);
}

/**
 * The key's icons. An app key has an icon for on and, if the app can be off, an optional one
 * for off; apps that show more states (weather, presence) list those too. A button has one
 * icon per state.
 */
function iconBox(k, key, app) {
  if (!app) return buttonIconBox(k, key);
  const st = key.states?.[0] || {};
  const oo = app.on_off;
  const canBeOff = app.roles.includes("toggle");
  const set = (field, label) => (id) => {
    edit(`${label} · key ${k}`, () => {
      const s = ensureState(ensureKey(k), 0);
      if (id) s[field] = id;
      else delete s[field];
    });
    hint(id ? `Key ${k}: ${label.toLowerCase()} is ${niceIcon(id)} now.` : `Key ${k}: ${label.toLowerCase()} is the default again.`);
  };
  const onLabel = canBeOff ? `Icon · ${oo ? oo.on.label : "On"}` : "Icon";
  const offLabel = `Icon · ${oo ? oo.off.label : "Off"}`;
  const onIcon = st.icon ? withNs(st.icon) : oo?.on.icon || app.icon || null;
  const offIcon = st.icon_off ? withNs(st.icon_off) : st.icon ? withNs(st.icon) : oo?.off.icon || null;
  const change = async (field, label, opts) => {
    const id = await pickIcon({ app, k, page: field === "icon", ...opts });
    if (id !== undefined) set(field, label)(id);
  };
  const rows = [
    iconRow({
      title: onLabel, icon: onIcon, k, live: true,
      note: st.icon ? null : onIcon ? `${niceIcon(onIcon)} · the app's default` : "Chosen by the app",
      placeholder: "Chosen by the app",
      onChange: () => change("icon", onLabel, {
        title: `${onLabel} · ${app.name}`, current: st.icon ? withNs(st.icon) : null,
        reset: { label: "The app's default", icon: oo?.on.icon || app.icon, note: oo?.on.icon || app.icon ? niceIcon(oo?.on.icon || app.icon) : "The app chooses it" },
      }),
      onReset: st.icon ? () => set("icon", onLabel)(null) : null, resetLabel: "Default",
    }),
    canBeOff ? iconRow({
      title: offLabel, icon: offIcon, k, live: false,
      note: st.icon_off ? null : st.icon || !oo ? "Optional. Now: the same icon, in its off look" : `${niceIcon(offIcon)} · the app's default`,
      placeholder: "Optional",
      onChange: () => change("icon_off", offLabel, {
        title: `${offLabel} · ${app.name}`, current: st.icon_off ? withNs(st.icon_off) : null,
        sub: "Shown while the app is off. Without one, the on icon is used, greyed out if it can.",
        reset: { label: "No icon of its own for off", icon: null, note: "The on icon is used" },
      }),
      onReset: st.icon_off ? () => set("icon_off", offLabel)(null) : null, resetLabel: "Remove",
    }) : null,
  ];
  // more states of the app's own display (weather, presence), while it draws it
  const chosen = key.config?.state_icons || {};
  const more = app.states?.length && !st.icon ? app.states.map((s) => iconRow({
    title: s.label, icon: chosen[s.key] || s.icon, k,
    note: chosen[s.key] ? null : `${niceIcon(s.icon)} · the app's default`,
    onChange: async () => {
      const id = await pickIcon({ k, title: `Icon · ${s.label} · ${app.name}`, current: chosen[s.key] || null, reset: { label: "The app's default", icon: s.icon, note: niceIcon(s.icon) } });
      if (id !== undefined) setStateIcon(k, s.key, id);
    },
    onReset: chosen[s.key] ? () => setStateIcon(k, s.key, null) : null, resetLabel: "Default",
  })) : null;
  return h("div", { class: "box" },
    h("span", { class: "label" }, "Icon"),
    rows,
    more ? [h("span", { class: "label" }, "Icon per state"), more] : null);
}

function buttonIconBox(k, key) {
  const idx = state.selState;
  const read = () => keyObj(k)?.states?.[idx];
  const holder = () => ensureState(ensureKey(k), idx);
  const cur = read()?.icon;
  const set = (id) => {
    edit(`Icon · key ${k}`, () => {
      const s = holder();
      if (id) { s.icon = id; delete s.image; } else delete s.icon;
    });
    hint(id ? `Key ${k} shows ${niceIcon(id)} now.` : `Key ${k} has no icon now.`);
  };
  return h("div", { class: "box" },
    h("span", { class: "label" }, "Icon"),
    iconRow({
      title: "Icon", icon: cur ? withNs(cur) : null, k, live: true, placeholder: "None: the label fills the key",
      onChange: async () => {
        const id = await pickIcon({ k, emoji: true, title: `Icon · key ${k}`, current: cur ? withNs(cur) : null, reset: { label: "No icon", icon: null, note: "The label fills the key" } });
        if (id !== undefined) set(id);
      },
      onReset: cur ? () => set(null) : null, resetLabel: "Remove",
    }),
    textField("Image instead", read, holder, "image", { ph: "Path to a PNG or JPEG; fills the key", name: "Image", where: `key ${k}`, dropEmpty: true }));
}

/** Set the icon of one more state of an app's display (`config.state_icons`). */
export function setStateIcon(k, stateKey, id) {
  const app = appDef(keyObj(k)?.app);
  const st = app?.states?.find((s) => s.key === stateKey);
  if (!st) return;
  edit(`${app.name} · ${st.label} icon`, () => {
    const c = (ensureKey(k).config ||= {});
    const m = (c.state_icons ||= {});
    if (id) m[st.key] = id;
    else delete m[st.key];
    if (!Object.keys(m).length) delete c.state_icons;
    if (!Object.keys(c).length) delete ensureKey(k).config;
  });
  hint(id ? `Key ${k}: ${st.label} shows ${niceIcon(id)} now.` : `Key ${k}: ${st.label} shows its default icon again.`);
}

/** The ids behind the names, for packages, deck.json and Home Assistant. */
function detailsBox(key, app) {
  const st = key.states?.[app ? 0 : state.selState] || {};
  const roles = app ? [...new Set([...app.roles.map(shortRole), "button"])] : ["button"];
  const row = (name, value) => (value ? h("div", { class: "kv" }, h("span", { class: "sub" }, name), h("code", {}, value)) : null);
  return h("details", { class: "more" },
    h("summary", {}, "Details"),
    row("App", app ? app.id : "core:button"),
    row("Offers to icons", roles.join(", ")),
    row("Icon", st.icon ? withNs(st.icon) : app?.icon),
    row("Icon for off", st.icon_off ? withNs(st.icon_off) : null));
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
    h("div", { class: "head" }, h("span", { class: "label grow" }, n > 1 ? `States: you are editing state ${state.selState + 1} of ${n}` : "States"),
      h("button", { class: "btn small", type: "button", onclick: add }, "+ State")),
    n > 1 ? h("div", { class: "tabs" }, Array.from({ length: n }, (_, i) => h("button", {
      type: "button", class: i === state.selState ? "on" : "", onclick: () => { state.selState = i; state.slot = "press"; render("inspector"); },
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
    step("Does"),
    actionSections(keyHolder(STATUS_KEY), { where: "the wide key", slots: [["press", "Press", false], ["long", "Long press", true], ["double", "Double press", true]] }),
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

