// The key editor on the right: header, Design / Actions / States tabs.
import { $, h, put, ask } from "./dom.js";
import {
  state, STATUS_KEY, edit, profile, keyObj, ensureKey, ensureState, keyKind,
} from "./store.js";
import { appDef, iconUrl, isSetIcon } from "./catalog.js";
import { img, iconPicker } from "./icons.js";
import { colorField, textField } from "./controls.js";
import { eventsEditor, holdThreshold } from "./actions.js";
import { appDesign, appHelp, placeApp, clearKey } from "./apps.js";
import { keyEl } from "./deck.js";
import { renderAutomation } from "./profiles.js";

const DEFAULT_BG = "#0f1216";
const DEFAULT_FG = "#f4f1e8";

function curState() {
  const k = keyObj();
  if (!k || !k.states || !k.states.length) return null;
  if (state.selState >= k.states.length) state.selState = 0;
  return k.states[state.selState];
}

export function renderHeader() {
  const thumb = $("insp-thumb");
  const { selected, view } = state;
  let eyebrow, title, pic;
  if (selected === STATUS_KEY) {
    eyebrow = "Status key";
    title = view.remote ? "Remote" : profile().name;
    pic = img("profile-switch");
  } else if (view.remote) {
    const cur = state.snap?.remote?.[String(selected)];
    eyebrow = `Remote key ${selected}`;
    title = cur?.look?.label || "Home Assistant";
    pic = img(isSetIcon(cur?.look?.icon) ? cur.look.icon : "home-assistant");
  } else {
    const key = keyObj();
    const kind = keyKind(key);
    eyebrow = `Editing key ${selected}`;
    if (kind === "app") {
      const a = appDef(key.app);
      const chosen = key.states?.[0]?.icon;
      title = a ? a.name : "Plugin missing";
      pic = img(isSetIcon(chosen) ? chosen : a?.icon || "grid");
    } else if (kind === "button") {
      const st = curState() || {};
      title = (st.label || "").replace(/\\n|\n/g, " ").trim() || "Button";
      pic = isSetIcon(st.icon) ? img(st.icon) : h("img", { class: "face", alt: "", src: keyEl(selected).querySelector("img").src });
    } else {
      title = "Empty key";
      pic = img("grid");
    }
  }
  put(thumb, pic);
  $("ed-eyebrow").textContent = eyebrow;
  $("ed-title").textContent = title;
}

export function renderInspector() {
  document.querySelectorAll("#insp-tabs button").forEach((b) => b.classList.toggle("on", b.dataset.tab === state.tab));
  renderHeader();
  const parts = state.tab === "design" ? designTab() : state.tab === "actions" ? actionsTab() : statesTab();
  put($("ed"), parts);
  renderAutomation();
  $("hint").textContent = state.view.remote
    ? "Remote page · Home Assistant sets these keys over MQTT"
    : "Select a key to edit · Drag artwork onto a key";
}

// ------------------------------------------------------------------ Design tab

/** Label, icon, image and colours of state `i` of the selected key (a button). */
function lookEditor(i) {
  const k = state.selected;
  const where = `key ${k}`;
  const read = () => keyObj(k)?.states?.[i];
  const holder = () => ensureState(ensureKey(k), i);
  const st = read() || {};
  const iconInput = h("input", { placeholder: "Icon name, emoji, or image path" });
  iconInput.value = st.icon || "";
  const thumb = h("img", { class: "ithumb", alt: "" });
  const showThumb = (v) => {
    const url = iconUrl(v);
    thumb.hidden = !url;
    if (url) thumb.src = url;
  };
  showThumb(st.icon);
  const setIcon = (v, typing) => {
    edit(`Icon · ${where}`, () => {
      const s = holder();
      if (v) s.icon = v;
      else delete s.icon;
    }, { redraw: false, coalesce: typing ? `icon:${where}:${i}` : null });
    showThumb(v);
  };
  iconInput.oninput = () => setIcon(iconInput.value, true);
  const palette = h("div", { hidden: true });
  const pick = (v) => {
    iconInput.value = v;
    palette.hidden = true;
    setIcon(v);
  };
  return [
    textField("Label", read, holder, "label", { area: true, ph: "Text on the key (Enter for a second line)", name: "Label", where: `${where} state ${i}` }),
    h("div", { class: "field" },
      h("span", { class: "label" }, "Icon"),
      h("span", { class: "pathbox" }, thumb, iconInput,
        h("button", { class: "btn small", type: "button", onclick: () => {
          if (!palette.firstChild) palette.append(iconPicker(pick));
          palette.hidden = !palette.hidden;
        } }, "Icons…")),
      palette),
    textField("Image <em>fills the key</em>", read, holder, "image", { ph: "Path to a PNG/JPEG, or drop one on the key", name: "Image", where: `${where} state ${i}`, dropEmpty: true }),
    h("div", { class: "row" },
      colorField("Background", read, holder, "bg", DEFAULT_BG, where),
      colorField("Text & icon", read, holder, "fg", DEFAULT_FG, where)),
  ];
}

function stateChips(key) {
  if (!key?.states || key.states.length < 2) return null;
  return h("div", { class: "states" },
    h("span", { class: "label", style: "margin-right:4px" }, "State"),
    key.states.map((_, i) => h("button", {
      type: "button", class: "st" + (i === state.selState ? " on" : ""),
      onclick: () => { state.selState = i; renderInspector(); },
    }, String(i + 1))));
}

const STATUS_APPS = [["profile", "Profile"], ["media", "Spotify / Music"], ["meeting", "Next meeting"], ["email", "Email"], ["steam", "Steam game"], ["printer", "Printer"]];
const STATUS_HELP = {
  profile: "The profile (and whether it follows the active app), Home Assistant, and the song or the clock. <b>Press / hold</b>: the actions on the Actions tab.",
  media: "<b>Press</b> plays / pauses, <b>hold</b> opens the player.",
  meeting: "From your Microsoft 365 calendar. <b>Press</b> opens the join link, <b>hold</b> dismisses the reminder.",
  email: "Unread mail from Microsoft 365. <b>Press</b> opens the message, <b>hold</b> dismisses the banner.",
  steam: "The game you are playing. <b>Press</b> opens it in Steam, <b>hold</b> returns to the profile.",
  printer: "From <code>d200/printer</code> on MQTT. Cards appear when a print completes or fails.",
};

function statusDesign() {
  const sk = state.config.status_key;
  const show = h("select", {}, [["auto", "Song while music plays, otherwise the clock"], ["profile", "Always the clock"], ["media", "The song whenever one is loaded"]].map(([v, t]) => h("option", { value: v }, t)));
  show.value = sk.show || "auto";
  show.onchange = () => edit("Status key content", (c) => { c.status_key.show = show.value; }, { redraw: false });
  const app = h("select", {}, STATUS_APPS.map(([v, t]) => h("option", { value: v }, t)));
  app.value = sk.app || "profile";
  app.onchange = () => edit("Status key content", (c) => { c.status_key.app = app.value; });
  return [
    h("p", { class: "note", html: "The wide key shows <b>one thing at a time</b>: the content below, or a card (meeting, printer, push, email) that takes over for 8 seconds. While a card is up, press opens it and hold dismisses it." }),
    h("label", { class: "field" }, h("span", { class: "label" }, "Default content"), app),
    h("p", { class: "sub", html: STATUS_HELP[sk.app || "profile"] || "" }),
    (sk.app || "profile") === "profile" ? h("label", { class: "field" }, h("span", { class: "label" }, "Right column"), show) : null,
  ];
}

function designTab() {
  if (state.selected === STATUS_KEY) return statusDesign();
  if (state.view.remote) return remoteInfo();
  const key = keyObj();
  const kind = keyKind(key);
  const seg = h("div", { class: "seg" },
    [["button", "Button"], ["app", "App"], ["empty", "Empty"]].map(([t, label]) =>
      h("button", { type: "button", class: kind === t ? "on" : "", onclick: () => setKind(t) }, label)));
  const parts = [h("div", { class: "field" }, h("span", { class: "label" }, "Type"), seg)];
  if (kind === "app") parts.push(...appDesign(key));
  else if (kind === "button") {
    parts.push(stateChips(key));
    curState();
    parts.push(...lookEditor(state.selState));
  } else parts.push(h("p", { class: "sub" }, "This key is blank. Choose Button or App, or pick an app in the Apps view."));
  return parts;
}

function remoteInfo() {
  const base = state.config.settings.mqtt.base_topic || "d200";
  const k = state.selected;
  const cur = state.snap?.remote?.[String(k)];
  const example = `action: mqtt.publish
data:
  topic: ${base}/remote/${k}/set
  retain: true
  payload: >
    {"label": "Kitchen", "icon": "light-on",
     "value": "{{ states('light.kitchen') }}"}`;
  return [
    h("p", { class: "note", html: `Home Assistant sets this key with a retained message on <code>${base}/remote/${k}/set</code>. Presses come back on <code>${base}/remote/${k}/action</code> and as device triggers in HA.` }),
    h("div", { class: "field" }, h("span", { class: "label" }, "Current"), h("pre", { class: "code" }, cur ? JSON.stringify(cur, null, 2) : "nothing set yet")),
    h("div", { class: "field" }, h("span", { class: "label" }, "Example"), h("pre", { class: "code" }, example)),
    state.config.settings.mqtt.enabled ? null : h("p", { class: "note", html: "MQTT is off. Turn it on in <b>Settings → Home Assistant</b>." }),
  ];
}

/** Switch the selected key between Button, App and Empty. */
export function setKind(t) {
  const k = state.selected;
  const old = keyObj(k);
  const kind = keyKind(old);
  if (t === kind) return;
  if (t === "empty") return clearKey(k);
  if (t === "app") {
    const id = old?.app && old.app !== "core:button" ? old.app : "com.deckling.core:sysmon";
    return placeApp(id, k);
  }
  edit(`Button on key ${k}`, () => {
    const key = ensureKey(k);
    delete key.app;
    delete key.config;
    ensureState(key, 0);
    state.selState = 0;
  });
}

// ------------------------------------------------------------------ Actions tab

function actionsTab() {
  const k = state.selected;
  if (k === STATUS_KEY) {
    return [
      h("span", { class: "label ev-title" }, "Status key events"),
      ...eventsEditor((create) => {
        const c = state.config;
        if (!c.status_key) { if (!create) return null; c.status_key = {}; }
        return c.status_key;
      }, { allowState: false, where: "status key" }),
      holdThreshold(),
    ];
  }
  if (state.view.remote) {
    const base = state.config.settings.mqtt.base_topic || "d200";
    return [h("p", { class: "note", html: `Presses on the Remote page go to Home Assistant: <code>${base}/remote/${k}/action</code> gets <code>down</code>, <code>up</code>, <code>press</code> and <code>long_press</code>. Build the reaction as an HA automation.` })];
  }
  const key = keyObj();
  const kind = keyKind(key);
  if (kind === "empty") return [h("p", { class: "sub" }, "Make this key a Button or App on the Design tab first.")];
  if (kind === "app") {
    const app = appDef(key.app);
    const help = appHelp(app);
    return [
      help ? h("p", { class: "note" }, help) : null,
      h("span", { class: "label ev-title" }, "Extra actions"),
      ...eventsEditor((create) => (create ? ensureState(ensureKey(k), 0) : keyObj(k)?.states?.[0]), { allowState: false, where: `key ${k}` }),
      holdThreshold(),
    ];
  }
  curState();
  const i = state.selState;
  return [
    stateChips(key),
    h("span", { class: "label ev-title" }, "Button events"),
    ...eventsEditor((create) => (create ? ensureState(ensureKey(k), i) : keyObj(k)?.states?.[i]), { where: `key ${k}` }),
    holdThreshold(),
  ];
}

// ------------------------------------------------------------------ States tab

function statesTab() {
  const key = keyObj();
  if (state.selected === STATUS_KEY || state.view.remote || keyKind(key) !== "button") {
    return [h("p", { class: "note", html: "States are for buttons: each state has its own look and actions, e.g. a toggle that shows <b>On</b> and <b>Off</b>." })];
  }
  const states = key.states?.length ? key.states : [{}];
  const rows = states.map((st, i) => {
    const icon = isSetIcon(st.icon) ? img(st.icon) : h("span", { class: "ithumb" }, st.icon || "");
    return h("div", { class: "state-row" + (i === state.selState ? " on" : ""), onclick: () => { state.selState = i; renderInspector(); } },
      icon,
      h("div", { class: "grow" }, h("b", {}, `State ${i + 1}${i === 0 ? " · initial" : ""}`), h("span", { class: "sub" }, (st.label || "no label").replace(/\\n|\n/g, " "))),
      states.length > 1 ? h("button", { class: "icon-btn", title: "Remove", onclick: (e) => { e.stopPropagation(); removeState(i); } }, "✕") : null);
  });
  return [
    h("p", { class: "note", html: "A key with several states shows the current one. A <b>Change state</b> action switches it; new states get <b>Change state</b> (next) on short press." }),
    ...rows,
    h("div", { class: "row wrap" }, h("button", { class: "btn", onclick: addState }, "+ Add state (copy of the selected one)")),
  ];
}

const isStateNext = (a) => a.action === "core:state";

function addState() {
  const k = state.selected;
  edit(`Add state · key ${k}`, () => {
    const key = ensureKey(k);
    ensureState(key, 0);
    key.states.push(structuredClone(key.states[state.selState] || {}));
    for (const s of key.states) {
      s.on ||= {};
      s.on.press ||= [];
      if (!s.on.press.some(isStateNext)) s.on.press.push({ action: "core:state", params: { to: "next" } });
    }
    state.selState = key.states.length - 1;
  });
}

async function removeState(i) {
  const k = state.selected;
  if (!(await ask(`Remove state ${i + 1}?`, null))) return;
  edit(`Remove state · key ${k}`, () => {
    const key = ensureKey(k);
    key.states.splice(i, 1);
    state.selState = Math.max(0, Math.min(state.selState, key.states.length - 1));
  });
}
