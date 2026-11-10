// D200 Deck UI. The Rust side owns the deck and renders every key, so the
// live preview is exactly what the device shows. This page edits deck.json
// (saved on every change).
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const NUM_KEYS = 14;
const STATUS_KEY = 13;
const DEFAULT_BG = "#0f1216";
const DEFAULT_FG = "#f4f1e8";
const ICON_DIR = "icons/night-signal";
const $ = (id) => document.getElementById(id);
const iconUrl = (n) => `${ICON_DIR}/svg/${n}.svg`;

const EVENTS = [
  ["down", "Press start", () => "On push"],
  ["up", "Release", () => "On lift"],
  ["press", "Short press", (ms) => `Under ${ms} ms`],
  ["long", "Long press", (ms) => `${ms} ms or longer`],
];

// Action menu. Most entries are one action type; volume has friendlier aliases.
const ACTIONS = {
  "volume-down": { name: "Decrease volume", type: "volume", fields: [{ f: "value", kind: "step", sign: "-" }], def: { value: "-5" } },
  "volume-up": { name: "Increase volume", type: "volume", fields: [{ f: "value", kind: "step", sign: "+" }], def: { value: "+5" } },
  volume: { name: "Set volume / mute", type: "volume", fields: [{ f: "value", kind: "select", opts: [["toggle-mute", "Toggle mute"], ["mute", "Mute"], ["unmute", "Unmute"], ...[0, 10, 25, 50, 75, 100].map((v) => [String(v), `Set to ${v}%`])] }], def: { value: "toggle-mute" } },
  command: { name: "Run a command", fields: [{ f: "command", kind: "textarea", ph: "say hello" }], def: { command: "" } },
  open: { name: "Open app, file or URL", fields: [{ f: "target", ph: "Safari, ~/Downloads or https://…" }], def: { target: "" } },
  keys: { name: "Keyboard shortcut", fields: [{ f: "combo", ph: "cmd+shift+4" }], def: { combo: "" } },
  media: { name: "Media (Spotify / Music)", fields: [{ f: "command", kind: "select", opts: [["playpause", "Play / pause"], ["next", "Next track"], ["previous", "Previous track"], ["play", "Play"], ["pause", "Pause"]] }], def: { command: "playpause" } },
  page: { name: "Go to page", fields: [{ f: "page", kind: "page" }], def: { page: "" } },
  back: { name: "Previous page" },
  home: { name: "Home page" },
  profile: { name: "Switch profile", fields: [{ f: "profile", kind: "profile" }], def: { profile: "next" } },
  state: { name: "Change this key's state", fields: [{ f: "to", kind: "state" }], def: { to: "next" } },
  teams: { name: "Teams meeting", fields: [{ f: "command", kind: "select", opts: [["toggle-mute", "Mute / unmute"], ["toggle-video", "Camera on / off"], ["toggle-hand", "Raise / lower hand"], ["toggle-background-blur", "Background blur"], ["toggle-ui-chat", "Show chat"], ["leave-call", "Leave the meeting"]] }], def: { command: "toggle-mute" } },
  presence: { name: "Teams presence", fields: [{ f: "to", kind: "select", opts: [["toggle", "Toggle available / unavailable"], ["auto", "Back to automatic"], ["Available", "Available"], ["Busy", "Busy"], ["DoNotDisturb", "Do not disturb"], ["BeRightBack", "Be right back"], ["Away", "Away"], ["Offline", "Appear offline"]] }], def: { to: "toggle" } },
  mqtt: { name: "MQTT publish", fields: [{ f: "topic", ph: "home/office/light/set" }, { f: "payload", ph: "ON" }, { f: "retain", kind: "bool", label: "retain" }], def: { topic: "", payload: "" } },
  brightness: { name: "Brightness", fields: [{ f: "value", ph: "+25, -25 or 128" }], def: { value: "+25" } },
  notify: { name: "Show a notification", fields: [{ f: "text", ph: "Text" }, { f: "icon", ph: "Icon name, e.g. notification" }, { f: "color", kind: "color" }], def: { text: "Hello", icon: "notification" } },
  remote: { name: "Show Home Assistant page" },
};
const menuId = (a) => (a.type === "volume" ? (String(a.value).startsWith("-") ? "volume-down" : String(a.value).startsWith("+") ? "volume-up" : "volume") : a.type);

const APPS = {
  sysmon: { name: "System monitor", icon: "system-monitor", help: "CPU, RAM, disk or network up/down with a live graph. <b>Press</b> switches between the ticked views." },
  mute: { name: "Sound", icon: "volume-on", help: "Shows whether the computer is muted. <b>Press</b> mutes or unmutes." },
  weather: { name: "Weather", icon: "weather-partly-cloudy", help: "Current weather for the place set in Settings → Weather. <b>Press</b> refreshes." },
  clock: { name: "Time / date", icon: "calendar", help: "<b>Press</b> switches between the time and the date." },
  pomodoro: { name: "Pomodoro", icon: "pomodoro-focus", help: "<b>Press</b> starts, pauses and resumes. <b>Long press</b> resets. Times in Settings → Pomodoro." },
  media: { name: "Media", icon: "now-playing", help: "Spotify or Apple Music. <b>Press</b> plays / pauses, <b>long press</b> skips." },
  teams_mute: { name: "Teams mic", icon: "microphone-on", help: "Live mic state in Teams meetings. <b>Press</b> mutes / unmutes. Needs Teams → Settings → Privacy → <b>Manage API</b>." },
  teams_camera: { name: "Teams camera", icon: "camera-on", help: "Live camera state in Teams meetings. <b>Press</b> turns it on / off." },
  teams_presence: { name: "Teams presence", icon: "presence-available", help: "Your live Teams status. <b>Press</b> toggles available ↔ unavailable, <b>long press</b> goes back to automatic. Needs a sign-in (Settings → Teams)." },
  debugger: { name: "JetBrains debugger", icon: "debug-active", help: "Lights up while a JetBrains IDE debugs. <b>Press</b>: debug / resume. <b>Long press</b> while debugging: stop." },
};
const SYSMON = [["cpu", "CPU load"], ["ram", "RAM"], ["disk", "Disk usage"], ["net", "Network up/down"]];
const EMOJI = "💻 🧭 📸 🎵 🔊 🔇 🎙 📞 💡 🏠 🔒 🌡 ☕ 🍅 ⏱ 📅 ✉️ 💬 📝 📁 ⚙️ ✅ ❌ ⭐ ❤️ 🔥 🚀 🎮 🎬 📺 🌙 ☀ 🔔 👋".split(" ");

let snap = null;
let config = null;
let view = { profile: 0, page: "", remote: false };
let selected = 0;
let selState = 0;
let tab = "design";
let pane = "editor";
const openEvents = new Set(["down", "press"]);
let saveTimer = null;
let iconSet = null;

// ------------------------------------------------------------------ helpers

function h(tag, attrs = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === "class") el.className = v;
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "html") el.innerHTML = v;
    else if (k in el && k !== "list") el[k] = v;
    else el.setAttribute(k, v);
  }
  for (const kid of kids.flat()) if (kid !== null && kid !== undefined && kid !== false) el.append(kid);
  return el;
}
/** replaceChildren that skips null/false (the native one prints "null"). */
const put = (el, ...kids) => el.replaceChildren(...kids.flat().filter((k) => k !== null && k !== undefined && k !== false));
const img = (name, cls) => h("img", { src: iconUrl(name), alt: "", class: cls });

function toast(msg, isErr = false) {
  const t = $("toast");
  t.textContent = msg;
  t.className = "toast" + (isErr ? " err" : "");
  t.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (t.hidden = true), isErr ? 7000 : 2500);
}

async function call(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    toast(String(e), true);
    throw e;
  }
}

/** Small modal: asks for text, or yes/no when value === null. */
function ask(title, value) {
  return new Promise((resolve) => {
    const d = $("dlg-prompt");
    const input = $("prompt-input");
    $("prompt-title").textContent = title;
    input.hidden = value === null;
    input.value = value || "";
    d.returnValue = "";
    d.onclose = () => resolve(d.returnValue !== "ok" ? null : value === null ? true : input.value.trim());
    input.onkeydown = (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        d.close("ok");
      }
    };
    d.showModal();
    if (value !== null) input.select();
  });
}

async function loadIconSet() {
  if (!iconSet || !iconSet.length) {
    // From Rust first (works under the app's CSP); the file as a fallback.
    iconSet = await invoke("icon_set").catch(() => null);
    if (!Array.isArray(iconSet) || !iconSet.length) {
      iconSet = (await fetch(`${ICON_DIR}/manifest.json`).then((r) => r.json()).catch(() => ({ icons: [] }))).icons;
    }
    if (!iconSet.length) toast("Icon library could not be loaded", true);
  }
  return iconSet;
}
const isSetIcon = (n) => !!iconSet && iconSet.some((i) => i.name === (n || "").trim());

const initials = (name) => {
  const w = (name || "").replace(/[^A-Za-z0-9 ]/g, " ").split(/\s+/).filter(Boolean);
  if (!w.length) return "•";
  if (w.length === 1) {
    const caps = w[0].match(/[A-Z]/g);
    return (caps && caps.length > 1 ? caps.slice(0, 2).join("") : w[0].slice(0, 2)).toUpperCase();
  }
  return (w[0][0] + w[1][0]).toUpperCase();
};

const profile = () => config.profiles[view.profile];
const pageObj = () => profile().pages[view.page] || {};
const keyObj = (k = selected) => pageObj()[String(k)];
function ensureKey(k = selected) {
  const p = profile().pages[view.page];
  return (p[String(k)] ||= {});
}
function isEmptyKey(key) {
  if (!key) return true;
  if (key.app) return false;
  const hasActions = (s) => Object.values(s.actions || {}).some((l) => l && l.length);
  return !(key.states || []).some((s) => hasActions(s) || Object.entries(s).some(([f, v]) => f !== "actions" && v));
}
function tidyKey(k = selected) {
  const p = profile().pages[view.page];
  const key = p && p[String(k)];
  // Empty action lists stay (the editor holds references to them); Rust drops them from deck.json.
  if (key && isEmptyKey(key)) delete p[String(k)];
}
const keyKind = (key) => (!key ? "empty" : key.app ? "app" : "button");

// ------------------------------------------------------------------ saving

function setSaved(busy) {
  const el = $("saved");
  el.classList.toggle("busy", busy);
  el.lastChild.textContent = busy ? " Saving…" : " Changes saved";
  const line = document.querySelector(".saved-line");
  if (line) line.textContent = busy ? "Saving…" : "✓ Saved";
}

function scheduleSave(redraw = false) {
  setSaved(true);
  clearTimeout(saveTimer);
  saveTimer = setTimeout(save, 300);
  if (redraw) renderInspector();
}

async function save() {
  clearTimeout(saveTimer);
  // Keep a key the user is still setting up (an empty Button), drop it elsewhere.
  if (view.profile < config.profiles.length && profile().pages[view.page]) {
    for (const k of Object.keys(pageObj())) if (+k !== selected) tidyKey(+k);
  }
  try {
    snap = await call("save_config", { config });
    view = snap.view;
    setSaved(false);
    renderPreviews();
    renderToolbar();
    renderAutomation();
    renderHeader();
  } catch {
    $("saved").lastChild.textContent = " Not saved";
  }
}

// ------------------------------------------------------------------ status

function showStatus(s) {
  const d = $("st-dev");
  d.className = "pill " + (s.connected ? "ok" : s.device_seen ? "" : "off");
  d.querySelector(".txt").textContent = s.connected ? "D200 Deck · Connected" : s.message || "";
  d.title = (s.message || "") + (s.adb ? `\nadb: ${s.adb}` : "");
  $("btn-firmware").hidden = !(s.device_seen && !s.connected);
  const m = $("st-mqtt");
  const state = s.mqtt_ok === true ? "Connected" : s.mqtt_ok === false ? "Connecting" : "off";
  m.className = "pill plain" + (s.mqtt_ok === true ? "" : " off");
  m.querySelector(".txt").textContent = `Home Assistant · ${state}`;
  m.title = s.mqtt || "";
  if ($("m-status")) $("m-status").textContent = s.mqtt ? "Status: " + s.mqtt : "";
  const a = s.active_app;
  $("app-chip").hidden = !a;
  if (a) {
    $("app-badge").textContent = initials(a.name);
    $("app-name").textContent = a.name;
    $("app-chip").title = a.id;
  }
}

// ------------------------------------------------------------------ toolbar, views

function renderToolbar() {
  $("crumb-name").textContent = profile().name;
  const sp = $("sel-profile");
  sp.replaceChildren(...config.profiles.map((p, i) => h("option", { value: i }, p.name)));
  sp.value = view.profile;
  const pg = $("sel-page");
  pg.replaceChildren(...Object.keys(profile().pages).map((n, i) => h("option", { value: n }, (i === 0 ? "⌂ " : "") + n)));
  pg.value = view.page;
  pg.disabled = view.remote;
  $("follow").checked = config.settings.follow_active_app;
  $("caption").textContent = view.remote ? "Remote page · keys set by Home Assistant" : "Live preview";
  document.querySelectorAll("#rail [data-view]").forEach((b) =>
    b.classList.toggle("on", b.dataset.view === (pane === "editor" && view.remote ? "remote" : pane)));
}

function showPane(name) {
  if (name === "remote") {
    pane = "editor";
    if (!view.remote) goto(view.profile, view.page, true);
  } else {
    pane = name;
    if (name === "editor" && view.remote) goto(view.profile, view.page, false);
  }
  document.querySelectorAll(".main > [data-pane]").forEach((p) => (p.hidden = p.dataset.pane !== pane));
  if (pane === "profiles") renderProfiles();
  if (pane === "apps") renderApps();
  if (pane === "assets") renderAssets();
  renderToolbar();
}

function goto(profileIdx, page, remote) {
  const p = config.profiles[profileIdx];
  const pg = page && p.pages[page] ? page : Object.keys(p.pages)[0];
  if (profileIdx !== view.profile) selState = 0;
  view = { profile: profileIdx, page: pg, remote };
  invoke("set_view", { profile: profileIdx, page: pg, remote });
  renderAll();
}

// ------------------------------------------------------------------ grid

function buildGrid() {
  const grid = $("grid");
  for (let k = 0; k < NUM_KEYS; k++) {
    grid.append(
      h("div", { class: "key" + (k === STATUS_KEY ? " wide" : ""), "data-key": k, onclick: () => select(k) },
        h("img", { alt: "" }), h("span", { class: "num" }, k === STATUS_KEY ? "" : String(k))),
    );
  }
}
const keyEl = (k) => document.querySelector(`.key[data-key="${k}"]`);

async function renderPreview(k) {
  const url = await invoke("preview", { profile: view.profile, page: view.page, remote: view.remote, key: k }).catch(() => null);
  if (url) {
    keyEl(k).querySelector("img").src = url;
    if (k === selected) {
      const face = document.querySelector("#insp-thumb img.face");
      if (face) face.src = url;
    }
  }
}
function renderPreviews() {
  for (let k = 0; k < NUM_KEYS; k++) renderPreview(k);
}

function select(k) {
  if (k !== selected) selState = 0;
  selected = k;
  document.querySelectorAll(".key").forEach((el) => el.classList.toggle("selected", +el.dataset.key === k));
  renderInspector();
  if (pane === "apps") renderApps();
  if (pane === "assets") $("assets-target").textContent = assetsTarget();
}

// ------------------------------------------------------------------ inspector

function curState() {
  const k = keyObj();
  if (!k || !k.states || !k.states.length) return null;
  if (selState >= k.states.length) selState = 0;
  return k.states[selState];
}

function renderHeader() {
  const thumb = $("insp-thumb");
  thumb.replaceChildren();
  let eyebrow, title;
  if (selected === STATUS_KEY) {
    eyebrow = "Status key";
    title = view.remote ? "Remote" : profile().name;
    thumb.append(img("profile-switch"));
  } else if (view.remote) {
    const cur = snap?.remote?.[String(selected)];
    eyebrow = `Remote key ${selected}`;
    title = cur?.look?.label || "Home Assistant";
    thumb.append(cur?.look?.icon && isSetIcon(cur.look.icon) ? img(cur.look.icon) : img("home-assistant"));
  } else {
    const key = keyObj();
    const kind = keyKind(key);
    eyebrow = `Editing key ${selected}`;
    if (kind === "app") {
      const a = APPS[key.app.type];
      title = a?.name || key.app.type;
      thumb.append(img(a?.icon || "grid"));
    } else if (kind === "button") {
      const st = curState() || {};
      title = (st.label || "").replace(/\\n|\n/g, " ").trim() || "Button";
      if (st.icon && isSetIcon(st.icon)) thumb.append(img(st.icon));
      else thumb.append(h("img", { class: "face", alt: "", src: keyEl(selected).querySelector("img").src }));
    } else {
      title = "Empty key";
      thumb.append(img("grid"));
    }
  }
  $("ed-eyebrow").textContent = eyebrow;
  $("ed-title").textContent = title;
}

function renderInspector() {
  document.querySelectorAll("#insp-tabs button").forEach((b) => b.classList.toggle("on", b.dataset.tab === tab));
  renderHeader();
  const parts = tab === "design" ? designTab() : tab === "actions" ? actionsTab() : statesTab();
  $("ed").replaceChildren(...parts.flat().filter(Boolean));
  renderAutomation();
  $("hint").textContent = view.remote
    ? "Remote page · Home Assistant sets these keys over MQTT"
    : "Select a key to edit · Drag artwork onto a key";
}

// -- Design tab

function colorField(label, obj, field, fallback) {
  const input = h("input", { type: "color", value: obj[field] || fallback });
  const box = h("span", { class: "colorbox" + (obj[field] ? "" : " unset") }, input,
    h("button", { class: "link", type: "button", onclick: () => { delete obj[field]; input.value = fallback; box.classList.add("unset"); scheduleSave(); } }, "default"));
  input.oninput = () => { obj[field] = input.value; box.classList.remove("unset"); scheduleSave(); };
  return h("label", { class: "field" }, h("span", { class: "label" }, label), box);
}

function textField(label, obj, field, opts = {}) {
  const el = opts.area ? h("textarea", { rows: opts.rows || 2, placeholder: opts.ph || "" }) : h("input", { placeholder: opts.ph || "" });
  el.value = obj[field] ?? "";
  el.oninput = () => { obj[field] = el.value; scheduleSave(); renderHeader(); };
  return h("label", { class: "field" }, label ? h("span", { class: "label", html: label }) : null, el);
}

/** Searchable grid of the icon set; `big` shows names (library view). */
function iconPicker(onPick, big = false) {
  const grid = h("div", { class: "igrid" + (big ? " big" : "") });
  const search = h("input", { placeholder: "Search 215 icons…", class: big ? "lib-search" : "" });
  const box = h("div", { class: big ? "" : "ipick" }, search, grid);
  const draw = async () => {
    const q = search.value.trim().toLowerCase();
    const icons = (await loadIconSet()).filter((i) => !q || i.name.includes(q) || i.group.toLowerCase().includes(q));
    const groups = {};
    for (const i of icons) (groups[i.group] ||= []).push(i.name);
    put(grid,
      ...Object.entries(groups).map(([g, names]) => h("div", {},
        h("div", { class: "igroup" }, g),
        h("div", { class: "irow" }, names.map((n) =>
          h("button", { type: "button", title: n, onclick: () => onPick(n) }, img(n), big ? h("small", {}, n.replace(/-/g, " ")) : null))))),
      q || big ? null : h("div", {}, h("div", { class: "igroup" }, "Emoji"),
        h("div", { class: "emojis" }, EMOJI.map((e) => h("button", { type: "button", onclick: () => onPick(e) }, e)))),
    );
    if (!icons.length && q) grid.replaceChildren(h("p", { class: "sub" }, "No icon matches."));
  };
  search.oninput = draw;
  draw();
  return box;
}

function lookEditor(st) {
  const iconInput = h("input", { placeholder: "Icon name, emoji, or image path" });
  iconInput.value = st.icon || "";
  const thumb = h("img", { class: "ithumb", alt: "" });
  const showThumb = () => {
    const known = isSetIcon(st.icon);
    thumb.hidden = !known;
    if (known) thumb.src = iconUrl(st.icon.trim());
  };
  showThumb();
  iconInput.oninput = () => { st.icon = iconInput.value; showThumb(); drawState(); scheduleSave(); renderHeader(); };
  const palette = h("div", { hidden: true });
  const stateBox = h("div", { class: "box", hidden: true });
  const drawState = () => {
    const name = (st.icon || "").trim();
    const info = iconSet && iconSet.find((i) => i.name === name);
    stateBox.hidden = !info?.stateful;
    if (!stateBox.hidden) put(stateBox, h("span", { class: "label" }, "Icon state"), ...stateControls(name, st, () => scheduleSave()));
  };
  drawState();
  const pick = (v) => { iconInput.value = st.icon = v; showThumb(); palette.hidden = true; drawState(); scheduleSave(); renderHeader(); };
  return [
    textField("Label", st, "label", { area: true, ph: "Text on the key (Enter for a second line)" }),
    h("div", { class: "field" },
      h("span", { class: "label" }, "Icon"),
      h("span", { class: "pathbox" }, thumb, iconInput,
        h("button", { class: "btn small", type: "button", onclick: () => {
          if (!palette.firstChild) palette.append(iconPicker(pick));
          palette.hidden = !palette.hidden;
        } }, "Icons…")),
      palette),
    stateBox,
    textField("Image <em>fills the key</em>", st, "image", { ph: "Path to a PNG/JPEG, or drop one on the key" }),
    h("div", { class: "row" }, colorField("Background", st, "bg", DEFAULT_BG), colorField("Text & icon", st, "fg", DEFAULT_FG)),
  ];
}

/** Controls for a stateful icon's state (port of the icon-state.js API). */
function stateControls(name, st, changed) {
  const s = (st.icon_state ||= {});
  const color = (label, key, def, obj = s) => {
    const i = h("input", { type: "color", value: obj[key] || def });
    i.oninput = () => { obj[key] = i.value; changed(); };
    return h("label", { class: "field" }, h("span", { class: "label" }, label), i);
  };
  const range = (label, key, def, obj = s, min = 0, max = 100, step = 1) => {
    const v = h("span", { class: "mono-num" }, String(obj[key] ?? def));
    const i = h("input", { type: "range", min, max, step, value: obj[key] ?? def });
    i.oninput = () => { obj[key] = +i.value; v.textContent = i.value; changed(); };
    return h("label", { class: "field" }, h("span", { class: "label" }, label), h("span", { class: "pathbox" }, i, v));
  };
  switch (name) {
    case "light": {
      const c = h("input", { type: "checkbox", checked: s.on !== false });
      c.onchange = () => { s.on = c.checked; changed(); };
      return [h("label", { class: "toggle" }, c, "On"), color("Light colour", "color", "#69E0C1")];
    }
    case "thermostat": {
      const t = h("input", { type: "number", step: "0.5", value: s.temperature ?? 22 });
      t.oninput = () => { s.temperature = parseFloat(t.value); changed(); };
      return [h("label", { class: "field" }, h("span", { class: "label" }, "Temperature"), t), range("Level %", "progress", 62), color("Colour", "color", "#FFBF69")];
    }
    case "pomodoro":
      return [range("Ring %", "progress", 75), color("Colour", "color", "#FFBF69")];
    case "analog-clock": {
      s.time ||= { hours: 10, minutes: 10, seconds: 30 };
      return [h("p", { class: "sub" }, "The clock widget shows the live time; on a button the hands are static."),
        range("Hours", "hours", 10, s.time, 0, 11), range("Minutes", "minutes", 10, s.time, 0, 59), range("Ring %", "progress", 70), color("Colour", "color", "#69E0C1")];
    }
    case "bambu-ams":
    case "bambu-ams-lite": {
      s.spools ||= [{ color: "#69E0C1", progress: 80 }, { color: "#B79AF7", progress: 65 }, { color: "#FFBF69", progress: 40 }, { color: "#FF8278", progress: 95 }];
      return s.spools.map((sp, i) => h("div", { class: "row" }, color(`Spool ${i + 1}`, "color", "#69E0C1", sp), range("Remaining %", "progress", 50, sp)));
    }
    case "downloads":
      return [color("Arrow colour", "color", "#87909E"), h("p", { class: "sub" }, "Grey idle, mint active, or any colour.")];
    case "volume-muted":
      return [color("Mark colour", "color", "#FF8278")];
    default:
      return [];
  }
}

function stateChips(k) {
  if (!k.states || k.states.length < 2) return null;
  return h("div", { class: "states" },
    h("span", { class: "label", style: "margin-right:4px" }, "State"),
    k.states.map((_, i) => h("button", { type: "button", class: "st" + (i === selState ? " on" : ""), onclick: () => { selState = i; renderInspector(); } }, String(i + 1))));
}

function designTab() {
  if (selected === STATUS_KEY) {
    const sel = h("select", {}, [["auto", "Song while music plays, otherwise the clock"], ["profile", "Always the clock"], ["media", "The song whenever one is loaded"]].map(([v, t]) => h("option", { value: v }, t)));
    sel.value = config.status_key.show || "auto";
    sel.onchange = () => { config.status_key.show = sel.value; scheduleSave(); };
    const app = h("select", {}, [["profile", "Profile"], ["media", "Spotify / Music"], ["meeting", "Next meeting"], ["email", "Email"], ["steam", "Steam game"], ["printer", "Printer"]].map(([v, t]) => h("option", { value: v }, t)));
    app.value = config.status_key.app || "profile";
    app.onchange = () => { config.status_key.app = app.value; scheduleSave(true); };
    const appHelp = {
      profile: "The profile (and whether it follows the active app), Home Assistant, and the song or the clock. <b>Press / hold</b>: the actions on the Actions tab.",
      media: "<b>Press</b> plays / pauses, <b>hold</b> opens the player.",
      meeting: "From your Microsoft 365 calendar. <b>Press</b> opens the join link, <b>hold</b> dismisses the reminder.",
      email: "Unread mail from Microsoft 365. <b>Press</b> opens the message, <b>hold</b> dismisses the banner.",
      steam: "The game you are playing. <b>Press</b> opens it in Steam, <b>hold</b> returns to the profile.",
      printer: "From <code>d200/printer</code> on MQTT. Cards appear when a print completes or fails.",
    };
    return [
      h("p", { class: "note", html: "The wide key shows <b>one thing at a time</b>: the content below, or a card (meeting, printer, push, email) that takes over for 8 seconds. While a card is up, press opens it and hold dismisses it." }),
      h("label", { class: "field" }, h("span", { class: "label" }, "Default content"), app),
      h("p", { class: "sub", html: appHelp[config.status_key.app || "profile"] }),
      (config.status_key.app || "profile") === "profile" ? h("label", { class: "field" }, h("span", { class: "label" }, "Right column"), sel) : null,
    ];
  }
  if (view.remote) return remoteInfo();
  const key = keyObj();
  const kind = keyKind(key);
  const seg = h("div", { class: "seg" },
    [["button", "Button"], ["app", "App"], ["empty", "Empty"]].map(([t, label]) =>
      h("button", { type: "button", class: kind === t ? "on" : "", onclick: () => setKind(t) }, label)));
  const parts = [h("div", { class: "field" }, h("span", { class: "label" }, "Type"), seg)];
  if (kind === "app") {
    const k = ensureKey();
    k.states ||= [{}];
    const st = k.states[0];
    const sel = h("select", {}, Object.entries(APPS).map(([t, d]) => h("option", { value: t }, d.name)));
    sel.value = k.app.type;
    sel.onchange = () => { k.app = { type: sel.value }; scheduleSave(true); };
    parts.push(h("label", { class: "field" }, h("span", { class: "label" }, "App"), sel));
    parts.push(h("p", { class: "note", html: APPS[k.app.type]?.help || "" }));
    if (k.app.type === "sysmon") {
      const modes = k.app.modes?.length ? k.app.modes : SYSMON.map((m) => m[0]);
      parts.push(h("div", { class: "checks" }, SYSMON.map(([m, label]) => {
        const c = h("input", { type: "checkbox", checked: modes.includes(m) });
        c.onchange = () => {
          const set = new Set(k.app.modes?.length ? k.app.modes : SYSMON.map((x) => x[0]));
          c.checked ? set.add(m) : set.delete(m);
          const arr = SYSMON.map((x) => x[0]).filter((x) => set.has(x));
          if (!arr.length) { c.checked = true; return; }
          if (arr.length === SYSMON.length) delete k.app.modes; else k.app.modes = arr;
          scheduleSave();
        };
        return h("label", { class: "toggle" }, c, label);
      })));
    }
    if (k.app.type === "clock") {
      const c = h("input", { type: "checkbox", checked: !!k.app.h12 });
      c.onchange = () => { if (c.checked) k.app.h12 = true; else delete k.app.h12; scheduleSave(); };
      parts.push(h("label", { class: "toggle" }, c, "12-hour clock"));
    }
    if (k.app.type === "debugger") {
      const sc = (f, label, def) => {
        const i = h("input", { class: "mono", placeholder: def });
        i.value = k.app[f] || "";
        i.oninput = () => { if (i.value.trim()) k.app[f] = i.value.trim(); else delete k.app[f]; scheduleSave(); };
        return h("label", { class: "field" }, h("span", { class: "label", html: label }), i);
      };
      parts.push(h("div", { class: "box" },
        h("span", { class: "label" }, "Shortcuts (JetBrains macOS keymap)"),
        sc("idle_keys", "Press while idle <em>· Debug</em>", "ctrl+d"),
        sc("active_keys", "Press while debugging <em>· Resume</em>", "cmd+alt+r"),
        sc("stop_keys", "Long press while debugging <em>· Stop</em>", "cmd+f2")));
    }
    parts.push(h("div", { class: "row" }, colorField("Background", st, "bg", DEFAULT_BG)));
    const wid = k.app.type === "clock" ? "clock" : k.app.type === "sysmon" ? (k.app.modes?.length === 1 ? (k.app.modes[0] === "net" ? "network" : k.app.modes[0]) : "cpu") : null;
    if (wid) parts.push(h("p", { class: "sub" }, "Colours, light and layout: ",
      h("a", { onclick: async () => { await openSettings(); document.querySelector('#set-tabs [data-tab="theme"]').click(); $("th-widget").value = wid; $("th-widget").onchange(); } }, "Theme → " + wid)));
  } else if (kind === "button") {
    const k = ensureKey();
    k.states ||= [{}];
    parts.push(stateChips(k));
    parts.push(...lookEditor(curState()));
  } else {
    parts.push(h("p", { class: "sub" }, "This key is blank. Choose Button or App, or pick an app in the Apps view."));
  }
  return parts;
}

function remoteInfo() {
  const base = config.settings.mqtt.base_topic || "d200";
  const cur = snap?.remote?.[String(selected)];
  const example = `action: mqtt.publish
data:
  topic: ${base}/remote/${selected}/set
  retain: true
  payload: >
    {"label": "Kitchen", "icon": "light-on",
     "value": "{{ states('light.kitchen') }}"}`;
  return [
    h("p", { class: "note", html: `Home Assistant sets this key with a retained message on <code>${base}/remote/${selected}/set</code>. Presses come back on <code>${base}/remote/${selected}/action</code> and as device triggers in HA.` }),
    h("div", { class: "field" }, h("span", { class: "label" }, "Current"), h("pre", { class: "code" }, cur ? JSON.stringify(cur, null, 2) : "nothing set yet")),
    h("div", { class: "field" }, h("span", { class: "label" }, "Example"), h("pre", { class: "code" }, example)),
    config.settings.mqtt.enabled ? null : h("p", { class: "note", html: "MQTT is off. Turn it on in <b>Settings → Home Assistant</b>." }),
  ];
}

// -- Actions tab

function actionField(a, fd, nStates) {
  switch (fd.kind) {
    case "textarea": {
      const t = h("textarea", { rows: 2, class: "mono", placeholder: fd.ph || "" });
      t.value = a[fd.f] || "";
      t.oninput = () => { a[fd.f] = t.value; scheduleSave(); };
      return t;
    }
    case "bool": {
      const c = h("input", { type: "checkbox", checked: !!a[fd.f] });
      c.onchange = () => { if (c.checked) a[fd.f] = true; else delete a[fd.f]; scheduleSave(); };
      return h("label", { class: "toggle sub" }, c, fd.label);
    }
    case "color": {
      const c = h("input", { type: "color", value: a[fd.f] || "#69e0c1" });
      c.oninput = () => { a[fd.f] = c.value; scheduleSave(); };
      return h("span", { class: "colorbox" }, c, h("span", { class: "sub" }, "colour"));
    }
    case "step": {
      const s = h("select", {}, [1, 2, 5, 10, 20].map((v) => h("option", { value: v }, `${v}%`)));
      s.value = String(Math.abs(parseInt(a.value, 10)) || 5);
      s.onchange = () => { a.value = `${fd.sign}${s.value}`; scheduleSave(); };
      return s;
    }
    case "select": {
      const s = h("select", {}, fd.opts.map(([v, t]) => h("option", { value: v }, t)));
      if (!a[fd.f]) a[fd.f] = fd.opts[0][0];
      s.value = a[fd.f];
      s.onchange = () => { a[fd.f] = s.value; scheduleSave(); };
      return s;
    }
    case "page":
    case "profile":
    case "state": {
      let opts;
      if (fd.kind === "page") opts = Object.keys(profile().pages).map((n) => [n, n]);
      else if (fd.kind === "profile") opts = [["next", "Next profile"], ["auto", "Back to automatic"], ...config.profiles.map((p) => [p.name, p.name])];
      else opts = [["next", "Next state"], ...Array.from({ length: nStates }, (_, i) => [String(i), `State ${i + 1}`])];
      if (!a[fd.f] && opts.length) a[fd.f] = opts[0][0];
      const s = h("select", {}, opts.map(([v, t]) => h("option", { value: v }, t)));
      s.value = a[fd.f];
      s.onchange = () => { a[fd.f] = s.value; scheduleSave(); };
      return s;
    }
    default: {
      const i = h("input", { placeholder: fd.ph || "", class: fd.f === "combo" || fd.f === "topic" ? "mono" : "" });
      i.value = a[fd.f] ?? "";
      i.oninput = () => { a[fd.f] = i.value; scheduleSave(); };
      return i;
    }
  }
}

/** The four event sections. `actions` is the object holding the lists. */
function eventsEditor(actions, { nStates = 1, allowState = true } = {}) {
  const ms = config.settings.long_press_ms;
  return EVENTS.map(([ev, title, desc]) => {
    const list = (actions[ev] ||= []);
    const sec = h("div", { class: "ev" });
    const head = h("div", { class: "ev-head" });
    const body = h("div", { class: "ev-body" });
    const draw = () => {
      const open = openEvents.has(ev);
      sec.classList.toggle("open", open);
      put(head,
        h("div", { class: "grow" }, h("b", {}, title), h("span", {}, desc(ms))),
        list.length && !open ? h("span", { class: "count" }, `${list.length} action${list.length > 1 ? "s" : ""}`) : null,
        h("span", { class: "chev" }, open ? "⌃" : list.length ? "⌄" : "+"),
      );
      body.hidden = !open;
      put(body,
        ...list.map((a, i) => {
          const id = menuId(a);
          const def = ACTIONS[id] || { name: a.type };
          const typeSel = h("select", {},
            Object.entries(ACTIONS).filter(([t]) => allowState || t !== "state").map(([t, d]) => h("option", { value: t }, d.name)));
          typeSel.value = id;
          typeSel.onchange = () => {
            const d = ACTIONS[typeSel.value];
            list[i] = { type: d.type || typeSel.value, ...structuredClone(d.def || {}) };
            draw();
            scheduleSave();
          };
          return h("div", { class: "act" },
            h("div", { class: "act-main" }, typeSel, (def.fields || []).map((fd) => actionField(a, fd, nStates))),
            h("button", { class: "icon-btn", title: "Remove", type: "button", onclick: () => { list.splice(i, 1); draw(); scheduleSave(); } }, "✕"));
        }),
        h("div", { class: "ev-foot" },
          h("button", { class: "add-link", type: "button", onclick: () => { list.push({ type: "command", command: "" }); draw(); scheduleSave(); } }, "+  Add action"),
          list.length ? h("button", { class: "btn small", type: "button", title: "Run these actions now", onclick: () => testEvent(ev) }, "▶  Test") : null),
      );
    };
    head.onclick = () => { openEvents.has(ev) ? openEvents.delete(ev) : openEvents.add(ev); draw(); };
    sec.append(head, body);
    draw();
    return sec;
  });
}

function holdThreshold() {
  const ms = config.settings.long_press_ms;
  const opts = [250, 300, 400, 500, 600, 800, 1000, 1500];
  if (!opts.includes(ms)) opts.push(ms);
  const sel = h("select", {}, opts.sort((a, b) => a - b).map((v) => h("option", { value: v }, `${v} ms`)));
  sel.value = String(ms);
  sel.onchange = () => { config.settings.long_press_ms = +sel.value; scheduleSave(true); };
  return h("div", { class: "hold" },
    h("div", { class: "row" }, h("b", {}, "Hold threshold"), sel),
    h("p", { class: "sub" }, "Long press suppresses short press. Applies to every key."),
    h("span", { class: "saved-line" }, "✓ Saved"));
}

async function testEvent(ev) {
  await save();
  await call("trigger", { key: selected, event: ev });
  toast("Ran " + ev);
}

function actionsTab() {
  if (selected === STATUS_KEY) {
    return [h("span", { class: "label ev-title" }, "Status key events"), ...eventsEditor(config.status_key.actions, { allowState: false }), holdThreshold()];
  }
  if (view.remote) {
    const base = config.settings.mqtt.base_topic || "d200";
    return [h("p", { class: "note", html: `Presses on the Remote page go to Home Assistant: <code>${base}/remote/${selected}/action</code> gets <code>down</code>, <code>up</code>, <code>press</code> and <code>long_press</code>. Build the reaction as an HA automation.` })];
  }
  const key = keyObj();
  const kind = keyKind(key);
  if (kind === "empty") return [h("p", { class: "sub" }, "Make this key a Button or App on the Design tab first.")];
  if (kind === "app") {
    const k = ensureKey();
    k.states ||= [{}];
    k.states[0].actions ||= {};
    return [
      h("p", { class: "note", html: APPS[k.app.type]?.help || "" }),
      h("span", { class: "label ev-title" }, "Extra actions"),
      ...eventsEditor(k.states[0].actions, { allowState: false }),
      holdThreshold(),
    ];
  }
  const k = ensureKey();
  k.states ||= [{}];
  const st = curState();
  st.actions ||= {};
  return [stateChips(k), h("span", { class: "label ev-title" }, "Button events"), ...eventsEditor(st.actions, { nStates: k.states.length }), holdThreshold()];
}

// -- States tab

function statesTab() {
  const key = keyObj();
  if (selected === STATUS_KEY || view.remote || keyKind(key) !== "button") {
    return [h("p", { class: "note", html: "States are for buttons: each state has its own look and actions, e.g. a toggle that shows <b>On</b> and <b>Off</b>." })];
  }
  const k = ensureKey();
  k.states ||= [{}];
  const rows = k.states.map((st, i) => {
    const icon = st.icon && isSetIcon(st.icon) ? img(st.icon) : h("span", { class: "ithumb" }, st.icon || "");
    return h("div", { class: "state-row" + (i === selState ? " on" : ""), onclick: () => { selState = i; renderInspector(); } },
      icon,
      h("div", { class: "grow" }, h("b", {}, `State ${i + 1}${i === 0 ? " · initial" : ""}`), h("span", { class: "sub" }, (st.label || "no label").replace(/\\n|\n/g, " "))),
      k.states.length > 1 ? h("button", { class: "icon-btn", title: "Remove", onclick: (e) => { e.stopPropagation(); removeState(k, i); } }, "✕") : null);
  });
  return [
    h("p", { class: "note", html: "A key with several states shows the current one. A <b>Change this key's state</b> action switches it; new states get <b>Next state</b> on short press." }),
    ...rows,
    h("div", { class: "row wrap" }, h("button", { class: "btn", onclick: () => addState(k) }, "+ Add state (copy of the selected one)")),
  ];
}

function setKind(t, appType) {
  const p = profile().pages[view.page];
  const old = keyObj();
  if (t === "empty") delete p[String(selected)];
  else if (t === "app") p[String(selected)] = { app: { type: appType || old?.app?.type || "sysmon" }, states: [{}] };
  else p[String(selected)] = { states: [old && !old.app && old.states?.length ? old.states[0] : { label: "" }] };
  selState = 0;
  scheduleSave(true);
}

function addState(k) {
  k.states.push(structuredClone(k.states[selState] || {}));
  for (const s of k.states) {
    s.actions ||= {};
    s.actions.press ||= [];
    if (!s.actions.press.some((a) => a.type === "state")) s.actions.press.push({ type: "state", to: "next" });
  }
  selState = k.states.length - 1;
  scheduleSave(true);
}

async function removeState(k, i) {
  if (!(await ask(`Remove state ${i + 1}?`, null))) return;
  k.states.splice(i, 1);
  selState = Math.max(0, Math.min(selState, k.states.length - 1));
  scheduleSave(true);
}

// -- Profile automation card

function renderAutomation() {
  const p = profile();
  const first = p.apps[0];
  const short = (id) => id.split(".").pop();
  $("automation").replaceChildren(
    h("h3", {}, "Profile automation"),
    h("div", { class: "auto-row" },
      h("span", { class: "badge" }, first ? initials(short(first)) : "—"),
      h("div", { class: "grow" },
        h("b", {}, first ? p.apps.map(short).join(", ") : view.profile === 0 ? "Default profile" : "No app bound"),
        h("span", { class: "sub" }, first
          ? `${p.name} activates automatically`
          : view.profile === 0 ? "Used when no other profile matches" : "Only used when you pick it")),
      h("a", { onclick: () => showPane("profiles") }, "Edit")),
  );
}

// ------------------------------------------------------------------ Profiles view

function renderProfiles() {
  $("profile-cards").replaceChildren(...config.profiles.map((p, i) => {
    const apps = h("div", { class: "chips" }, p.apps.map((a, j) =>
      h("span", { class: "chip", title: a }, h("span", {}, a),
        h("button", { title: "Remove", onclick: (e) => { e.stopPropagation(); p.apps.splice(j, 1); renderProfiles(); scheduleSave(); } }, "×"))));
    const n = Object.keys(p.pages).length;
    return h("div", { class: "card pcard" + (i === view.profile ? " sel" : ""), onclick: () => { goto(i, null, false); renderProfiles(); } },
      h("div", { class: "pcard-head" },
        h("span", { class: "badge" }, initials(p.name)),
        h("h3", {}, p.name),
        i === 0 ? h("span", { class: "tag" }, "default") : null,
        i === view.profile ? h("span", { class: "tag live" }, "on device") : null),
      h("span", { class: "sub" }, `${n} page${n > 1 ? "s" : ""} · ${p.apps.length ? "used for:" : i === 0 ? "used when nothing else matches" : "no apps bound"}`),
      apps,
      h("div", { class: "row wrap" },
        h("button", { class: "btn small", onclick: (e) => { e.stopPropagation(); openAppPicker(i); } }, "+ Add app"),
        h("button", { class: "btn small", onclick: (e) => { e.stopPropagation(); renameProfile(i); } }, "Rename"),
        i > 0 ? h("button", { class: "btn small", onclick: (e) => { e.stopPropagation(); makeDefault(i); } }, "Make default") : null,
        h("button", { class: "btn small danger-ghost", onclick: (e) => { e.stopPropagation(); deleteProfile(i); } }, "Delete")));
  }));
}

function* allActions() {
  for (const p of config.profiles)
    for (const page of Object.values(p.pages))
      for (const key of Object.values(page))
        for (const s of key.states || []) for (const list of Object.values(s.actions || {})) yield* list;
  for (const list of Object.values(config.status_key.actions || {})) yield* list;
}

async function addProfile() {
  const name = await ask("New profile name", "");
  if (!name) return;
  if (config.profiles.some((p) => p.name === name)) return toast("A profile with that name exists", true);
  config.profiles.push({ name, apps: [], pages: { main: {} } });
  await save();
  goto(config.profiles.length - 1, "main", false);
  renderProfiles();
}

async function renameProfile(i) {
  const old = config.profiles[i].name;
  const name = await ask("Rename profile", old);
  if (!name || name === old) return;
  if (config.profiles.some((p) => p.name === name)) return toast("A profile with that name exists", true);
  config.profiles[i].name = name;
  for (const a of allActions()) if (a.type === "profile" && a.profile === old) a.profile = name;
  await save();
  renderProfiles();
}

async function makeDefault(i) {
  const p = config.profiles.splice(i, 1)[0];
  config.profiles.unshift(p);
  view.profile = 0;
  await save();
  goto(0, null, false);
  renderProfiles();
}

async function deleteProfile(i) {
  if (config.profiles.length < 2) return toast("The last profile cannot be deleted", true);
  if (!(await ask(`Delete profile "${config.profiles[i].name}" and all its pages?`, null))) return;
  config.profiles.splice(i, 1);
  view.profile = 0;
  view.page = Object.keys(config.profiles[0].pages)[0];
  await save();
  goto(0, null, false);
  renderProfiles();
}

async function openAppPicker(i) {
  const list = $("apps-list");
  list.replaceChildren(h("li", {}, h("span", { class: "sub" }, "Loading…")));
  $("dlg-apps").showModal();
  const add = (id) => {
    const p = config.profiles[i];
    if (!p.apps.includes(id)) p.apps.push(id);
    renderProfiles();
    scheduleSave();
    toast(`Added ${id}`);
  };
  $("apps-add").onclick = () => {
    const v = $("apps-manual").value.trim();
    if (v) add(v);
    $("apps-manual").value = "";
  };
  const apps = await call("running_apps").catch(() => []);
  list.replaceChildren(...apps.map((a) => h("li", { onclick: () => add(a.id) }, h("span", {}, a.name), h("span", { class: "sub" }, a.id))));
  if (!apps.length) list.replaceChildren(h("li", {}, h("span", { class: "sub" }, "No apps found — type the name below.")));
}

// ------------------------------------------------------------------ Apps & Assets views

const editableKey = () => selected !== STATUS_KEY && !view.remote;

function renderApps() {
  $("apps-target").textContent = editableKey() ? `Selected: key ${selected}` : "Select one of keys 0–12 first";
  $("app-cards").replaceChildren(...Object.entries(APPS).map(([t, a]) =>
    h("div", { class: "card acard" }, img(a.icon),
      h("div", { class: "grow" }, h("h3", {}, a.name), h("p", { class: "sub", html: a.help }),
        h("button", { class: "btn small", disabled: !editableKey(), onclick: () => { setKind("app", t); toast(`${a.name} on key ${selected}`); showPane("editor"); } }, `Use on key ${selected}`)))));
}

function assetsTarget() {
  return editableKey() ? `Click an icon to put it on key ${selected}` : "Select one of keys 0–12 first";
}

function renderAssets() {
  $("assets-target").textContent = assetsTarget();
  const lib = $("assets-lib");
  if (lib.firstChild) return;
  lib.append(iconPicker((n) => {
    if (!editableKey()) return toast("Select one of keys 0–12 first", true);
    const key = keyObj();
    if (key?.app) return toast("App keys draw their own icons; make it a Button first", true);
    const k = ensureKey();
    k.states ||= [{}];
    if (selState >= k.states.length) selState = 0;
    k.states[selState].icon = n;
    scheduleSave(true);
    toast(`Icon "${n}" on key ${selected}`);
  }, true));
}

// ------------------------------------------------------------------ pages

async function addPage() {
  const name = await ask("New page name", "");
  if (!name) return;
  if (profile().pages[name]) return toast("A page with that name exists", true);
  profile().pages[name] = { "0": { states: [{ label: "Back", icon: "arrow-left", actions: { press: [{ type: "back" }] } }] } };
  await save();
  goto(view.profile, name, false);
}

async function renamePage() {
  const old = view.page;
  const name = await ask("Rename page", old);
  if (!name || name === old) return;
  if (profile().pages[name]) return toast("A page with that name exists", true);
  const pages = {};
  for (const [n, p] of Object.entries(profile().pages)) pages[n === old ? name : n] = p;
  profile().pages = pages;
  for (const a of allActions()) if (a.type === "page" && a.page === old) a.page = name;
  view.page = name;
  await save();
  goto(view.profile, name, false);
}

async function makeHome() {
  const pages = { [view.page]: profile().pages[view.page] };
  for (const [n, p] of Object.entries(profile().pages)) if (n !== view.page) pages[n] = p;
  profile().pages = pages;
  await save();
  renderToolbar();
}

async function deletePage() {
  if (Object.keys(profile().pages).length < 2) return toast("A profile needs at least one page", true);
  if (!(await ask(`Delete page "${view.page}"?`, null))) return;
  delete profile().pages[view.page];
  view.page = Object.keys(profile().pages)[0];
  await save();
  goto(view.profile, null, false);
}

// ------------------------------------------------------------------ settings

function mqttHelp(base) {
  const rows = [
    [`${base}/notify`, "text or JSON {title, text, icon, color, style, duration} → animated notification"],
    [`${base}/remote/0…12/set`, "JSON {label, icon, value, bg, fg}, retained → a key on the Remote page"],
    [`${base}/remote/N/action`, "press · long_press · down · up (also HA device triggers)"],
    [`${base}/remote/show`, "anything shows the Remote page, \"off\" leaves it"],
    [`${base}/key`, "every key event, JSON {profile, page, key, event}"],
    [`${base}/profile/set`, "switch profile by name, or \"auto\""],
    [`${base}/brightness/set`, "0–255"],
    [`${base}/availability`, "\"online <computer>\" while a computer has the D200"],
  ];
  return `<p>With discovery on, Home Assistant gets a <b>Ulanzi D200</b> device: button triggers for the Remote page, a <b>notify</b> entity, a <b>profile</b> select, <b>brightness</b>, a "Show remote page" button and which <b>computer</b> has it. Run D200 Deck with the same settings on every computer the KVM switches to.</p>
  <table>${rows.map(([t, d]) => `<tr><td><code>${t}</code></td><td>${d}</td></tr>`).join("")}</table>`;
}

let pendingLocation;
async function openSettings() {
  const s = config.settings;
  $("s-long").value = s.long_press_ms;
  $("s-follow").checked = s.follow_active_app;
  $("s-autostart").checked = await invoke("autostart_get").catch(() => false);
  $("s-path").textContent = snap?.configPath || "";
  pendingLocation = s.weather.location;
  $("w-current").textContent = pendingLocation ? pendingLocation.name : "not set";
  $("w-f").checked = s.weather.fahrenheit;
  $("w-results").replaceChildren();
  const p = s.pomodoro;
  $("p-work").value = p.work; $("p-short").value = p.short_break; $("p-long").value = p.long_break; $("p-rounds").value = p.rounds; $("p-sound").checked = p.sound;
  const m = s.mqtt;
  $("m-on").checked = m.enabled; $("m-host").value = m.host; $("m-port").value = m.port; $("m-user").value = m.username;
  $("m-pass").value = m.password; $("m-base").value = m.base_topic; $("m-tls").checked = m.tls; $("m-disc").checked = m.discovery;
  $("m-help").innerHTML = mqttHelp(m.base_topic || "d200");
  $("m-status").textContent = snap?.status?.mqtt ? "Status: " + snap.status.mqtt : "";
  const t = s.teams;
  $("t-port").value = t.port; $("t-client").value = t.client_id; $("t-tenant").value = t.tenant; $("t-away").value = t.away;
  $("t-code").hidden = true;
  const c = s.cards, ss = s.screensaver;
  $("c-email").checked = c.email; $("c-meet").checked = c.meetings; $("c-steam").checked = c.steam; $("c-print").checked = c.printer; $("c-hide").checked = c.hide_content;
  $("ss-on").checked = ss.enabled; $("ss-idle").value = ss.idle_minutes; $("ss-scene").value = ss.scene; $("ss-fps").value = ss.fps;
  $("ss-motion").checked = ss.motion; $("ss-south").checked = ss.southern;
  renderThemeEditor();
  showIntegrations(snap?.integrations);
  $("s-error").textContent = "";
  $("dlg-settings").showModal();
}

async function saveSettings() {
  const s = config.settings;
  const num = (id, lo, hi) => Math.min(hi, Math.max(lo, parseInt($(id).value, 10) || lo));
  s.long_press_ms = num("s-long", 150, 5000);
  s.follow_active_app = $("s-follow").checked;
  s.weather.location = pendingLocation || null;
  s.weather.fahrenheit = $("w-f").checked;
  Object.assign(s.pomodoro, { work: num("p-work", 1, 180), short_break: num("p-short", 1, 60), long_break: num("p-long", 1, 120), rounds: num("p-rounds", 1, 12), sound: $("p-sound").checked });
  Object.assign(s.cards, { email: $("c-email").checked, meetings: $("c-meet").checked, steam: $("c-steam").checked, printer: $("c-print").checked, hide_content: $("c-hide").checked });
  Object.assign(s.screensaver, { enabled: $("ss-on").checked, idle_minutes: num("ss-idle", 1, 120), scene: $("ss-scene").value, fps: num("ss-fps", 1, 20), motion: $("ss-motion").checked, southern: $("ss-south").checked });
  Object.assign(s.teams, { port: num("t-port", 1, 65535), client_id: $("t-client").value.trim(), tenant: $("t-tenant").value.trim() || "organizations", away: $("t-away").value });
  Object.assign(s.mqtt, {
    enabled: $("m-on").checked, host: $("m-host").value.trim(), port: num("m-port", 1, 65535), username: $("m-user").value,
    password: $("m-pass").value, base_topic: $("m-base").value.trim() || "d200", tls: $("m-tls").checked, discovery: $("m-disc").checked,
  });
  for (const [k, v] of Object.entries(s.theme?.widgets || {})) if (!Object.keys(v).length) delete s.theme.widgets[k];
  try {
    await invoke("autostart_set", { on: $("s-autostart").checked }).catch(() => {});
    snap = await invoke("save_config", { config });
    $("dlg-settings").close();
    renderAll();
    toast("Settings saved");
  } catch (e) {
    $("s-error").textContent = String(e);
  }
}

function wireSettings() {
  document.querySelectorAll("#set-tabs button").forEach((b) => {
    b.onclick = () => {
      document.querySelectorAll("#set-tabs button").forEach((x) => x.classList.toggle("on", x === b));
      document.querySelectorAll("#dlg-settings [data-pane]").forEach((p) => (p.hidden = p.dataset.pane !== b.dataset.tab));
    };
  });
  $("s-save").onclick = saveSettings;
  $("ss-preview").onclick = async () => { await saveSettings(); await call("screensaver", { on: true }); toast("Screensaver on the device — press any key to wake"); };
  $("s-folder").onclick = () => call("open_config_dir");
  const search = async () => {
    const q = $("w-query").value.trim();
    if (!q) return;
    const ul = $("w-results");
    ul.replaceChildren(h("li", {}, h("span", { class: "sub" }, "Searching…")));
    try {
      const res = await invoke("geocode", { query: q });
      ul.replaceChildren(...(res.length ? res.map((r) =>
        h("li", { onclick: () => { pendingLocation = { name: r.name, lat: r.lat, lon: r.lon }; $("w-current").textContent = `${r.name} (${r.detail})`; ul.replaceChildren(); } },
          h("span", {}, r.name), h("span", { class: "sub" }, r.detail))) : [h("li", {}, h("span", { class: "sub" }, "No places found"))]));
    } catch (e) {
      ul.replaceChildren(h("li", {}, h("span", { class: "sub" }, String(e))));
    }
  };
  $("w-search").onclick = search;
  $("w-query").onkeydown = (e) => { if (e.key === "Enter") { e.preventDefault(); search(); } };
  $("m-base").oninput = () => ($("m-help").innerHTML = mqttHelp($("m-base").value.trim() || "d200"));
}

// ------------------------------------------------------------------ theme editor

const WIDGETS = [
  ["cpu", "CPU"], ["ram", "RAM"], ["disk", "Disk"], ["network", "Network"], ["clock", "Clock"],
  ["mail", "Mail card"], ["music", "Music card"], ["printer", "Printer card"], ["meeting", "Meeting card"], ["game", "Game card"], ["push", "Push card"],
];
const WIDE = new Set(["mail", "music", "printer", "meeting", "game", "push"]);
const TH_COLORS = [
  ["accent", "Accent", "#69E0C1"], ["light", "Background light", "#69E0C1"], ["background", "Surface", "#11171D"], ["border", "Border", "#30363F"],
  ["text", "Text", "#F4F1E8"], ["secondary", "Secondary text", "#9BA5B2"], ["icon_color", "Icon strokes", "#F4F1E8"], ["icon_accent", "Icon accent", "#69E0C1"],
  ["chart", "Chart", "#69E0C1"], ["progress", "Progress", "#69E0C1"], ["track", "Track", "#30363F"],
];

function themeColor(obj, key, label, def) {
  const i = h("input", { type: "color", value: obj[key] || def });
  const box = h("span", { class: "colorbox" + (obj[key] ? "" : " unset") }, i,
    h("button", { class: "link", type: "button", onclick: () => { delete obj[key]; i.value = def; box.classList.add("unset"); } }, "default"));
  i.oninput = () => { obj[key] = i.value; box.classList.remove("unset"); };
  return h("label", { class: "field", style: "min-width:128px;flex:0 0 auto" }, h("span", { class: "label" }, label), box);
}

function renderThemeEditor() {
  const t = (config.settings.theme ||= { palette: {}, widgets: {} });
  t.widgets ||= {};
  const pal = (t.palette ||= {});
  put($("th-palette"),
    themeColor(pal, "active", "Active", "#69E0C1"), themeColor(pal, "info", "Info", "#B79AF7"),
    themeColor(pal, "attention", "Attention", "#FFBF69"), themeColor(pal, "error", "Error", "#FF8278"),
    themeColor(pal, "background", "Surface", "#11171D"));
  const sel = $("th-widget");
  const drawWidget = () => {
    const id = sel.value;
    const w = (t.widgets[id] ||= {});
    const num = (key, label, def) => {
      const v = h("span", { class: "mono-num" }, String(w[key] ?? def));
      const i = h("input", { type: "range", min: 0, max: 1, step: 0.05, value: w[key] ?? def });
      i.oninput = () => { w[key] = +i.value; v.textContent = i.value; };
      return h("label", { class: "field" }, h("span", { class: "label" }, label), h("span", { class: "pathbox" }, i, v));
    };
    const layout = h("div", { class: "seg" }, ["compact", "airy"].map((l) =>
      h("button", { type: "button", class: (w.layout || "compact") === l ? "on" : "", onclick: () => { if (l === "compact") delete w.layout; else w.layout = l; drawWidget(); } }, l === "compact" ? "Compact" : "Airy")));
    put($("th-fields"),
      WIDE.has(id) ? h("div", { class: "field" }, h("span", { class: "label" }, "Layout"), layout,
        h("p", { class: "sub" }, "Compact: cover fills the left 42 %. Airy: 24 % square cover, more padding, short metadata.")) : null,
      h("div", { class: "row wrap" }, TH_COLORS.map(([k, n, d]) => themeColor(w, k, n, d))),
      h("div", { class: "row" }, num("light_opacity", "Light intensity", 0.9), num("idle_light_factor", "Idle light factor", 0.28)),
      id === "network" ? h("div", { class: "row wrap" }, themeColor(w, "network_up", "Upload", "#69E0C1"), themeColor(w, "network_down", "Download", "#B79AF7")) : null,
      id === "music" || id === "game" ? h("div", { class: "row wrap" }, themeColor(w, "cover_tint", "Cover tint", "#000000")) : null,
    );
  };
  if (!sel.options.length) {
    sel.replaceChildren(...WIDGETS.map(([v, n]) => h("option", { value: v }, n)));
    $("th-reset").onclick = () => { delete t.widgets[sel.value]; drawWidget(); };
  }
  sel.onchange = drawWidget;
  drawWidget();
}

function showIntegrations(i) {
  if (!i || !$("t-api")) return;
  const t = i.teams || {};
  $("t-api").textContent = !i.teamsOn
    ? "Not used: add a Teams mic/camera key or a Teams action."
    : t.connected
      ? (t.paired ? "Connected and paired." : "Connected. Teams asks to pair the first time you press a key in a meeting.") + (i.meeting?.in_meeting ? " In a meeting." : "")
      : "Not connected: " + (t.error || "waiting for Teams");
  $("t-account").textContent = i.signedIn ? `Signed in${i.account ? " as " + i.account : ""}${i.presence ? " · " + i.presence : ""}${i.presenceError ? " · " + i.presenceError : ""}` : "Not signed in.";
  $("t-signin").hidden = !!i.signedIn;
  $("t-signout").hidden = !i.signedIn;
}

function wireTeams() {
  $("t-signin").onclick = async () => {
    const clientId = $("t-client").value.trim();
    const tenant = $("t-tenant").value.trim() || "organizations";
    $("t-code").hidden = false;
    $("t-code").textContent = "Contacting Microsoft…";
    try {
      const r = await invoke("teams_sign_in", { clientId, tenant });
      $("t-code").innerHTML = `Enter the code <b class="mono">${r.user_code}</b> at <code>${r.verification_uri}</code> (opened in your browser). This finishes by itself.`;
      navigator.clipboard?.writeText(r.user_code).catch(() => {});
    } catch (e) {
      $("t-code").textContent = String(e);
    }
  };
  $("t-signout").onclick = () => call("teams_sign_out");
}

// ------------------------------------------------------------------ dialogs

function wireDialogs() {
  const n = () => ({
    title: $("n-title").value || null, text: $("n-text").value, icon: $("n-icon").value || null, color: $("n-color").value,
    style: $("n-style").value, duration: parseFloat($("n-dur").value) || 8, priority: $("n-prio").value,
  });
  const upd = () => {
    $("n-json").textContent = JSON.stringify(n());
    $("n-topic").textContent = (config.settings.mqtt.base_topic || "d200") + "/notify";
  };
  for (const id of ["n-title", "n-text", "n-icon", "n-color", "n-style", "n-dur", "n-prio"]) $(id).addEventListener("input", upd);
  $("btn-notify").onclick = () => { upd(); $("dlg-notify").showModal(); };
  $("n-send").onclick = () => call("notify", { notification: n() });

  $("btn-json").onclick = () => {
    $("json-text").value = JSON.stringify(config, null, 2);
    $("json-error").textContent = "";
    $("dlg-json").showModal();
  };
  $("json-copy").onclick = () => navigator.clipboard.writeText($("json-text").value).then(() => toast("Copied"));
  $("json-import").onclick = async () => {
    try {
      applySnapshot(await invoke("import_config", { json: $("json-text").value }));
      $("dlg-json").close();
      toast("Config imported");
    } catch (e) {
      $("json-error").textContent = String(e);
    }
  };
}

// ------------------------------------------------------------------ drag & drop images

function wireDrop() {
  const wv = window.__TAURI__.webview?.getCurrentWebview?.();
  if (!wv) return;
  const keyAt = (pos) => {
    if (!pos) return null;
    const r = window.devicePixelRatio || 1;
    const el = document.elementFromPoint(pos.x / r, pos.y / r)?.closest(".key");
    return el ? +el.dataset.key : null;
  };
  wv.onDragDropEvent((ev) => {
    const p = ev.payload;
    document.querySelectorAll(".key.drop").forEach((el) => el.classList.remove("drop"));
    if (view.remote) return;
    if (p.type === "over" || p.type === "enter") {
      const k = keyAt(p.position);
      if (k !== null && k !== STATUS_KEY) keyEl(k).classList.add("drop");
    } else if (p.type === "drop") {
      const k = keyAt(p.position) ?? selected;
      if (k === STATUS_KEY) return;
      const file = (p.paths || []).find((f) => /\.(png|jpe?g|gif|webp|bmp)$/i.test(f));
      if (!file) return toast("Drop a PNG, JPEG, GIF or WebP image", true);
      const key = ensureKey(k);
      if (key.app) delete key.app;
      key.states ||= [{}];
      key.states[k === selected ? selState : 0].icon = file;
      select(k);
      scheduleSave(true);
    }
  });
}

// ------------------------------------------------------------------ start

function renderAll() {
  renderToolbar();
  renderPreviews();
  select(selected);
  $("brightness").value = config.settings.brightness;
  $("brightness-val").textContent = Math.round((config.settings.brightness / 255) * 100) + "%";
  if (pane === "profiles") renderProfiles();
  if (pane === "apps") renderApps();
}

function applySnapshot(s) {
  snap = s;
  config = s.config;
  view = s.view;
  showStatus(s.status);
  if (s.fontError) toast("Text rendering: " + s.fontError, true);
  renderAll();
}

async function init() {
  buildGrid();
  wireSettings();
  wireTeams();
  wireDialogs();
  wireDrop();
  await loadIconSet();

  document.querySelectorAll("#rail [data-view]").forEach((b) => (b.onclick = () => showPane(b.dataset.view)));
  document.querySelector('#rail [data-action="settings"]').onclick = openSettings;
  $("btn-settings").onclick = openSettings;
  $("btn-library").onclick = () => showPane("assets");
  $("crumb-profiles").onclick = () => showPane("profiles");
  document.querySelectorAll("#insp-tabs button").forEach((b) => (b.onclick = () => { tab = b.dataset.tab; renderInspector(); }));
  $("sel-profile").onchange = (e) => goto(+e.target.value, null, view.remote);
  $("sel-page").onchange = (e) => goto(view.profile, e.target.value, false);
  $("pg-add").onclick = addPage;
  $("pg-menu").onclick = (e) => { e.stopPropagation(); $("pg-menu-list").hidden = !$("pg-menu-list").hidden; };
  document.addEventListener("click", () => ($("pg-menu-list").hidden = true));
  $("pg-rename").onclick = renamePage;
  $("pg-home").onclick = makeHome;
  $("pg-delete").onclick = deletePage;
  $("pf-add").onclick = addProfile;
  $("follow").onchange = (e) => { config.settings.follow_active_app = e.target.checked; scheduleSave(); };
  $("zoom").onchange = (e) => document.querySelector(".device").style.setProperty("--zoom", e.target.value);
  $("brightness").oninput = (e) => {
    config.settings.brightness = +e.target.value;
    $("brightness-val").textContent = Math.round((+e.target.value / 255) * 100) + "%";
    scheduleSave();
  };
  $("btn-firmware").onclick = async () => {
    const b = $("btn-firmware");
    b.disabled = true;
    b.textContent = "Starting…";
    try { toast(await call("start_firmware")); } finally { b.disabled = false; b.textContent = "Start firmware"; }
  };

  applySnapshot(await invoke("get_state"));

  await listen("status", (e) => { if (snap) snap.status = e.payload; showStatus(e.payload); });
  await listen("view", (e) => {
    const v = e.payload;
    if (v.profile !== view.profile || v.page !== view.page || v.remote !== view.remote) {
      if (v.profile !== view.profile) selState = 0;
      view = v;
      renderToolbar();
      renderPreviews();
      renderInspector();
      if (pane === "profiles") renderProfiles();
    }
  });
  await listen("devkey", (e) => keyEl(e.payload.key)?.classList.toggle("hw-down", e.payload.down));
  await listen("config", (e) => {
    if (e.payload.brightness !== undefined) {
      config.settings.brightness = e.payload.brightness;
      $("brightness").value = e.payload.brightness;
      $("brightness-val").textContent = Math.round((e.payload.brightness / 255) * 100) + "%";
    }
  });
  await listen("remote", async () => {
    snap = await invoke("get_state");
    if (view.remote) { renderPreviews(); renderInspector(); }
  });
  await listen("error", (e) => toast(e.payload.message, true));
  await listen("integrations", (e) => { if (snap) snap.integrations = e.payload; showIntegrations(e.payload); });
  await listen("teams-signin", (e) => {
    $("t-code").hidden = false;
    $("t-code").textContent = e.payload.ok ? "Signed in." : "Sign-in failed: " + e.payload.error;
    if (e.payload.ok) toast("Signed in to Microsoft 365");
  });

  // Live faces (clock, system monitor, pomodoro, media, notifications) while visible.
  setInterval(() => {
    if (document.visibilityState !== "visible" || pane !== "editor") return;
    renderPreview(STATUS_KEY);
    if (view.remote) return;
    for (let k = 0; k < STATUS_KEY; k++) if (keyObj(k)?.app) renderPreview(k);
  }, 1000);
}

init();
