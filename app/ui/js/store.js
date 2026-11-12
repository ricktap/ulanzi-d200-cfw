// The one state object of the UI, with undo.
//
// Every change to the config goes through `edit(label, fn)`: it pushes a copy
// of the config onto the undo stack, runs `fn` (which mutates the live config
// in place, so unknown fields survive), tells the renderer and saves after a
// short debounce. `undo()` puts the previous config back and saves it.
import { invoke, call } from "./api.js";

export const NUM_KEYS = 14;
export const STATUS_KEY = 13;
const UNDO_CAP = 100;
const COALESCE_MS = 1500;
const SAVE_DELAY = 300;

export const state = {
  snap: null, // last snapshot from Rust (status, remote, integrations, ...)
  config: null, // deck.json, version 2
  view: { profile: 0, page: "", remote: false },
  catalog: { plugins: [], actions: [], apps: [], icons: [], interfaces: {}, signals: [], warnings: [] },
  selected: 0,
  selState: 0,
  pane: "deck", // deck | packages | signals
  libTab: "apps", // apps | actions | icons
  iconFor: null, // the icons tab picks for this state of the app's widget (app.states[].key), else the key's icon
  slot: "press", // the action section the library adds to: press | long | down | up
  editProfile: false,
};

const undoStack = []; // [{label, config}]
let last = { coalesce: null, at: 0 };
let saveTimer = null;
const listeners = {};

/** Subscribe to "edit", "undo", "load", "saving", "saved", "save-error", "render". */
export function on(type, fn) {
  (listeners[type] ||= []).push(fn);
}
function emit(type, detail) {
  for (const fn of listeners[type] || []) fn(detail);
}

/** Ask the renderer (main.js) to redraw parts: "all", "inspector", "library", "device", "previews", "pane". */
export const render = (...parts) => emit("render", new Set(parts));

/** The line under the deck that says what just happened. */
export const hint = (text) => emit("hint", text);

export const canUndo = () => undoStack.length > 0;
export const undoLabel = () => undoStack.at(-1)?.label || "";

/**
 * Change the config. `fn(config)` mutates it in place.
 * opts.redraw (default true): redraw the inspector; false only refreshes its header
 *   (for inputs that must keep focus).
 * opts.coalesce: a key; edits with the same key in a row share one undo step (typing, dragging).
 * opts.save (default true): schedule the debounced save.
 */
export function edit(label, fn, opts = {}) {
  const now = Date.now();
  const merge = opts.coalesce && last.coalesce === opts.coalesce && now - last.at < COALESCE_MS;
  if (!merge) {
    undoStack.push({ label, config: structuredClone(state.config) });
    if (undoStack.length > UNDO_CAP) undoStack.shift();
  }
  last = { coalesce: opts.coalesce || null, at: now };
  fn(state.config);
  emit("edit", { label, redraw: opts.redraw !== false });
  if (opts.save !== false) scheduleSave();
}

/** Put the previous config back and save it. */
export function undo() {
  const entry = undoStack.pop();
  if (!entry) return;
  last = { coalesce: null, at: 0 };
  state.config = entry.config;
  clampView();
  emit("undo", entry);
  scheduleSave();
}

/** Replace the config with a snapshot from Rust (start, import). `label` makes it undoable. */
export function load(snap, label) {
  if (label && state.config) {
    undoStack.push({ label, config: state.config });
    if (undoStack.length > UNDO_CAP) undoStack.shift();
  }
  last = { coalesce: null, at: 0 };
  state.snap = snap;
  state.config = snap.config;
  state.view = snap.view;
  clampView();
  emit("load", snap);
}

function clampView() {
  const c = state.config;
  const v = state.view;
  if (!c.profiles[v.profile]) v.profile = 0;
  const pages = c.profiles[v.profile].pages;
  if (!pages[v.page]) {
    v.page = Object.keys(pages)[0];
    invoke("set_view", { profile: v.profile, page: v.page, remote: v.remote }).catch(() => {});
  }
}

// ------------------------------------------------------------------ saving

export function scheduleSave() {
  emit("saving");
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => saveNow().catch(() => {}), SAVE_DELAY);
}

/**
 * Save now. Resolves with the snapshot; rejects on error. With `quiet`, the
 * caller shows the error itself (no toast).
 */
export async function saveNow({ quiet = false } = {}) {
  clearTimeout(saveTimer);
  // Keep a key the user is still setting up (an empty Button), drop it elsewhere.
  const p = profile();
  if (p && p.pages[state.view.page]) {
    for (const k of Object.keys(pageObj())) if (+k !== state.selected) tidyKey(+k);
  }
  emit("saving");
  try {
    const snap = await (quiet ? invoke : call)("save_config", { config: state.config });
    state.snap = snap;
    state.view = snap.view;
    emit("saved", snap);
    return snap;
  } catch (e) {
    emit("save-error", e);
    throw e;
  }
}

// ------------------------------------------------------------------ view

export function goto(profileIdx, page, remote) {
  const p = state.config.profiles[profileIdx];
  const pg = page && p.pages[page] ? page : Object.keys(p.pages)[0];
  if (profileIdx !== state.view.profile) state.selState = 0;
  state.view = { profile: profileIdx, page: pg, remote };
  invoke("set_view", { profile: profileIdx, page: pg, remote }).catch(() => {});
  render("all");
}

// ------------------------------------------------------------------ config access

export const profile = () => state.config.profiles[state.view.profile];
export const pageObj = () => profile().pages[state.view.page] || {};
export const keyObj = (k = state.selected) => pageObj()[String(k)];

/** The key object, created when missing. Only call inside an edit. */
export function ensureKey(k = state.selected) {
  const p = profile().pages[state.view.page];
  return (p[String(k)] ||= {});
}

/** State `i` of a key, created when missing. Only call inside an edit. */
export function ensureState(key, i = 0) {
  key.states ||= [];
  while (key.states.length <= i) key.states.push({});
  return key.states[i];
}

const hasActions = (s) => Object.values(s.on || {}).some((l) => Array.isArray(l) && l.length);

export function isEmptyKey(key) {
  if (!key) return true;
  if (key.app && key.app !== "core:button") return false;
  // Any other field (config, unknown fields) keeps the key.
  if (Object.keys(key).some((f) => f !== "states" && f !== "app")) return false;
  return !(key.states || []).some((s) => hasActions(s) || Object.entries(s).some(([f, v]) => f !== "on" && v));
}

export function tidyKey(k) {
  const p = profile().pages[state.view.page];
  const key = p && p[String(k)];
  if (key && isEmptyKey(key)) delete p[String(k)];
}

/** "empty", "app" or "button". `core:button` is a button. */
export const keyKind = (key) => (!key ? "empty" : key.app && key.app !== "core:button" ? "app" : "button");

export const editableKey = () => state.selected !== STATUS_KEY && !state.view.remote;

/** Every action in the config (for renames). */
export function* allActions() {
  const c = state.config;
  for (const p of c.profiles)
    for (const page of Object.values(p.pages))
      for (const key of Object.values(page))
        for (const s of key.states || []) for (const list of Object.values(s.on || {})) if (Array.isArray(list)) yield* list;
  for (const list of Object.values(c.status_key?.on || {})) if (Array.isArray(list)) yield* list;
}

/**
 * The deck's settings as one object, stored per plugin in `plugins.<id>.config`:
 * `weather` and `pomodoro` live in com.deckling.core, `teams` is the whole
 * com.deckling.teams config, everything else is in `core`.
 */
const SETTINGS_HOME = { weather: "com.deckling.core", pomodoro: "com.deckling.core" };
export function settingsOf(c) {
  const conf = (id) => {
    c.plugins ||= {};
    c.plugins[id] ||= {};
    c.plugins[id].config ||= {};
    return c.plugins[id].config;
  };
  return new Proxy({}, {
    get: (_, k) => (k === "teams" ? conf("com.deckling.teams") : conf(SETTINGS_HOME[k] || "core")[k]),
    set: (_, k, v) => {
      if (k === "teams") c.plugins["com.deckling.teams"].config = v;
      else conf(SETTINGS_HOME[k] || "core")[k] = v;
      return true;
    },
  });
}
