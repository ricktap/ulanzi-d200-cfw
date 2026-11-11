// D200 Deck UI. The Rust side owns the deck and renders every key, so the
// live preview is exactly what the device shows. This page edits deck.json
// (version 2) through the store, which saves every change.
import { $, h, put, toast, initials } from "./dom.js";
import { invoke, listen, call } from "./api.js";
import {
  state, on, edit, undo, canUndo, undoLabel, load, goto, profile, settingsOf } from "./store.js";
import { loadCatalog } from "./catalog.js";
import { buildGrid, renderPreviews, select, keyEl, startLiveFaces, wireDrop } from "./deck.js";
import { renderInspector, renderHeader } from "./inspector.js";
import { renderApps } from "./apps.js";
import { renderAssets } from "./library.js";
import { renderProfiles, renderAutomation, addProfile, addPage, renamePage, makeHome, deletePage } from "./profiles.js";
import { openSettings, wireSettings, showIntegrations, teamsSignedIn } from "./settings.js";
import { wireDialogs } from "./dialogs.js";
import { wireUpdates } from "./updates.js";

// ------------------------------------------------------------------ header and status

function showStatus(s) {
  const d = $("st-dev");
  d.className = "pill " + (s.connected ? "ok" : s.device_seen ? "" : "off");
  d.querySelector(".txt").textContent = s.connected ? "D200 Deck · Connected" : s.message || "";
  d.title = (s.message || "") + (s.adb ? `\nadb: ${s.adb}` : "");
  $("btn-firmware").hidden = !(s.device_seen && !s.connected);
  const m = $("st-mqtt");
  const st = s.mqtt_ok === true ? "Connected" : s.mqtt_ok === false ? "Connecting" : "off";
  m.className = "pill plain" + (s.mqtt_ok === true ? "" : " off");
  m.querySelector(".txt").textContent = `Home Assistant · ${st}`;
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

function renderUndo() {
  const b = $("btn-undo");
  b.disabled = !canUndo();
  b.title = canUndo() ? `Undo: ${undoLabel()} (${navigator.platform.startsWith("Mac") ? "⌘" : "Ctrl+"}Z)` : "Nothing to undo";
}

function setSaved(busy, failed = false) {
  const el = $("saved");
  el.classList.toggle("busy", busy || failed);
  el.lastChild.textContent = failed ? " Not saved" : busy ? " Saving…" : " Changes saved";
  const line = document.querySelector(".saved-line");
  if (line) line.textContent = failed ? "Not saved" : busy ? "Saving…" : "✓ Saved";
}

function renderBrightness() {
  const b = settingsOf(state.config).brightness;
  $("brightness").value = b;
  $("brightness-val").textContent = Math.round((b / 255) * 100) + "%";
}

// ------------------------------------------------------------------ toolbar, views

function renderToolbar() {
  const { config, view } = state;
  $("crumb-name").textContent = profile().name;
  const sp = $("sel-profile");
  put(sp, config.profiles.map((p, i) => h("option", { value: i }, p.name)));
  sp.value = view.profile;
  const pg = $("sel-page");
  put(pg, Object.keys(profile().pages).map((n, i) => h("option", { value: n }, (i === 0 ? "⌂ " : "") + n)));
  pg.value = view.page;
  pg.disabled = view.remote;
  $("follow").checked = settingsOf(config).follow_active_app;
  $("caption").textContent = view.remote ? "Remote page · keys set by Home Assistant" : "Live preview";
  document.querySelectorAll("#rail [data-view]").forEach((b) =>
    b.classList.toggle("on", b.dataset.view === (state.pane === "editor" && view.remote ? "remote" : state.pane)));
}

function renderPane() {
  if (state.pane === "profiles") renderProfiles();
  if (state.pane === "apps") renderApps();
  if (state.pane === "assets") renderAssets();
}

function showPane(name) {
  if (name === "remote") {
    state.pane = "editor";
    if (!state.view.remote) goto(state.view.profile, state.view.page, true);
  } else {
    state.pane = name;
    if (name === "editor" && state.view.remote) goto(state.view.profile, state.view.page, false);
  }
  document.querySelectorAll(".main > [data-pane]").forEach((p) => (p.hidden = p.dataset.pane !== state.pane));
  renderPane();
  renderToolbar();
}

function renderAll() {
  renderToolbar();
  renderPreviews();
  select(state.selected);
  renderBrightness();
  renderUndo();
}

/** Redraw requests from the modules (store.render). */
function renderParts(parts) {
  if (parts.has("all")) return renderAll();
  for (const p of parts) {
    if (p === "inspector") renderInspector();
    else if (p === "header") renderHeader();
    else if (p === "toolbar") renderToolbar();
    else if (p === "previews") renderPreviews();
    else if (p === "pane") renderPane();
    else if (p.startsWith("pane:")) showPane(p.slice(5));
  }
}

// ------------------------------------------------------------------ store events

on("render", renderParts);
on("edit", ({ redraw }) => {
  if (redraw) {
    renderInspector();
    renderToolbar();
    renderPane();
  } else renderHeader();
  renderUndo();
});
on("undo", (entry) => {
  renderAll();
  toast(`Undone: ${entry.label}`);
});
on("load", (snap) => {
  showStatus(snap.status || {});
  if (snap.fontError) toast("Text rendering: " + snap.fontError, true);
  renderAll();
});
on("saving", () => setSaved(true));
on("saved", () => {
  setSaved(false);
  renderPreviews();
  renderToolbar();
  renderAutomation();
  renderHeader();
});
on("save-error", () => setSaved(false, true));

// ------------------------------------------------------------------ start

function wireShell() {
  document.querySelectorAll("#rail [data-view]").forEach((b) => (b.onclick = () => showPane(b.dataset.view)));
  document.querySelector('#rail [data-action="settings"]').onclick = openSettings;
  $("btn-settings").onclick = openSettings;
  $("btn-undo").onclick = undo;
  $("btn-library").onclick = () => showPane("assets");
  $("crumb-profiles").onclick = () => showPane("profiles");
  document.querySelectorAll("#insp-tabs button").forEach((b) => (b.onclick = () => { state.tab = b.dataset.tab; renderInspector(); }));
  $("sel-profile").onchange = (e) => goto(+e.target.value, null, state.view.remote);
  $("sel-page").onchange = (e) => goto(state.view.profile, e.target.value, false);
  $("pg-add").onclick = addPage;
  $("pg-menu").onclick = (e) => { e.stopPropagation(); $("pg-menu-list").hidden = !$("pg-menu-list").hidden; };
  document.addEventListener("click", () => ($("pg-menu-list").hidden = true));
  $("pg-rename").onclick = renamePage;
  $("pg-home").onclick = makeHome;
  $("pg-delete").onclick = deletePage;
  $("pf-add").onclick = addProfile;
  $("follow").onchange = (e) => edit("Follow active app", (c) => { settingsOf(c).follow_active_app = e.target.checked; }, { redraw: false });
  $("zoom").onchange = (e) => document.querySelector(".device").style.setProperty("--zoom", e.target.value);
  $("brightness").oninput = (e) => {
    edit("Brightness", (c) => { settingsOf(c).brightness = +e.target.value; }, { redraw: false, coalesce: "brightness" });
    $("brightness-val").textContent = Math.round((+e.target.value / 255) * 100) + "%";
  };
  $("btn-firmware").onclick = async () => {
    const b = $("btn-firmware");
    b.disabled = true;
    b.textContent = "Starting…";
    try { toast(await call("start_firmware")); } finally { b.disabled = false; b.textContent = "Start firmware"; }
  };
  // Cmd/Ctrl+Z: undo, unless a text field or a dialog has the focus.
  document.addEventListener("keydown", (e) => {
    if (!(e.metaKey || e.ctrlKey) || e.shiftKey || e.altKey || e.key.toLowerCase() !== "z") return;
    const t = e.target;
    if (t && (t.closest?.("input, textarea, select, [contenteditable]") || t.isContentEditable)) return;
    if (document.querySelector("dialog[open]")) return;
    e.preventDefault();
    undo();
  });
}

async function listenAll() {
  await listen("status", (e) => { if (state.snap) state.snap.status = e.payload; showStatus(e.payload); });
  await listen("view", (e) => {
    const v = e.payload;
    const cur = state.view;
    if (v.profile !== cur.profile || v.page !== cur.page || v.remote !== cur.remote) {
      if (v.profile !== cur.profile) state.selState = 0;
      state.view = v;
      renderToolbar();
      renderPreviews();
      renderInspector();
      if (state.pane === "profiles") renderProfiles();
    }
  });
  await listen("devkey", (e) => keyEl(e.payload.key)?.classList.toggle("hw-down", e.payload.down));
  await listen("config", (e) => {
    // Brightness changed on the device or over MQTT: not an edit, no undo step.
    if (e.payload.brightness !== undefined) {
      settingsOf(state.config).brightness = e.payload.brightness;
      renderBrightness();
    }
  });
  await wireUpdates();
  await listen("remote", async () => {
    const snap = await invoke("get_state");
    state.snap = { ...snap, config: state.config };
    if (state.view.remote) { renderPreviews(); renderInspector(); }
  });
  await listen("error", (e) => toast(e.payload.message, true));
  await listen("integrations", (e) => { if (state.snap) state.snap.integrations = e.payload; showIntegrations(e.payload); });
  await listen("teams-signin", (e) => teamsSignedIn(e.payload));
}

async function init() {
  buildGrid();
  wireSettings();
  wireDialogs();
  wireDrop();
  wireShell();
  await loadCatalog();
  load(await invoke("get_state"));
  await listenAll();
  startLiveFaces();
}

init();
