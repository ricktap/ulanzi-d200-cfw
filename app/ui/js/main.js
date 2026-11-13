// D200 Deck UI. The Rust side owns the deck and draws every key, so the preview
// is exactly what the device shows. This page edits deck.json through the store,
// which saves every change and keeps the undo steps.
import { $, toast } from "./dom.js";
import { invoke, listen, call } from "./api.js";
import { state, on, edit, undo, canUndo, undoLabel, load, goto, settingsOf } from "./store.js";
import { loadCatalog } from "./catalog.js";
import { buildGrid, renderPreviews, renderPages, select, keyEl, startLiveFaces, wireDrop } from "./deck.js";
import { renderInspector } from "./inspector.js";
import { renderLibrary, wireLibrary, forgetPreviews } from "./library.js";
import { renderProfileEditor, toggleEditor, addProfile, addPage } from "./profiles.js";
import { renderTheme } from "./theme.js";
import { renderPackages, teamsSignedIn } from "./packages.js";
import { startSignals, stopSignals, wireSignals } from "./signals.js";
import { wireDialogs } from "./dialogs.js";
import { wireUpdates } from "./updates.js";

// ------------------------------------------------------------------ header

function showStatus(s) {
  const d = $("st-dev");
  d.className = "pill " + (s.connected ? "ok" : s.device_seen ? "" : "off");
  d.querySelector(".txt").textContent = s.connected ? "D200 connected" : s.message || "D200 not connected";
  d.title = (s.message || "") + (s.adb ? `\nadb: ${s.adb}` : "");
  $("btn-firmware").hidden = !(s.device_seen && !s.connected);
  const mqttOn = state.config?.plugins?.core?.config?.mqtt?.enabled;
  const m = $("st-mqtt");
  m.hidden = !mqttOn;
  m.className = "pill " + (s.mqtt_ok ? "ok" : "");
  m.querySelector(".txt").textContent = s.mqtt_ok ? "Home Assistant connected" : "Home Assistant connecting";
  m.title = s.mqtt || "";
  const a = s.active_app;
  $("app-chip").hidden = !a;
  if (a) {
    $("app-name").textContent = `In front: ${a.name}`;
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
  el.textContent = failed ? "Not saved" : busy ? "Saving…" : "Saved";
}

function renderBrightness() {
  const b = settingsOf(state.config).brightness ?? 180;
  $("brightness").value = b;
  $("brightness-val").textContent = Math.round((b / 255) * 100) + "%";
}

// ------------------------------------------------------------------ views

function renderDevice() {
  renderPages();
  renderProfileEditor();
  $("follow").checked = settingsOf(state.config).follow_active_app !== false;
  renderBrightness();
  renderTheme();
}

function showPane(name) {
  state.pane = name;
  document.querySelectorAll("#side [data-view]").forEach((b) => b.classList.toggle("on", b.dataset.view === name));
  document.querySelectorAll(".shell > [data-pane]").forEach((p) => (p.hidden = p.dataset.pane !== name));
  if (name === "signals") startSignals();
  else stopSignals();
  if (name === "packages") renderPackages();
  if (name === "deck" || name === "look") renderAll();
}

function renderAll() {
  forgetPreviews();
  renderLibrary();
  renderDevice();
  renderPreviews();
  select(state.selected);
  renderUndo();
}

/** Redraw requests from the modules (store.render). */
function renderParts(parts) {
  if (parts.has("all")) return renderAll();
  for (const p of parts) {
    if (p === "inspector") renderInspector();
    else if (p === "library") renderLibrary();
    else if (p === "device") renderDevice();
    else if (p === "previews") renderPreviews();
    else if (p === "pane" && state.pane === "packages") renderPackages();
  }
}

// ------------------------------------------------------------------ store events

on("render", renderParts);
on("hint", (text) => ($("hint").textContent = text));
on("edit", ({ redraw }) => {
  if (redraw) {
    renderInspector();
    renderLibrary();
    renderDevice();
    if (state.pane === "packages") renderPackages();
  }
  renderUndo();
});
on("undo", (entry) => {
  renderAll();
  if (state.pane === "packages") renderPackages();
  $("hint").textContent = `Undone: ${entry.label}.`;
});
on("load", (snap) => {
  showStatus(snap.status || {});
  if (snap.fontError) toast("Text rendering: " + snap.fontError, true);
  renderAll();
});
on("saving", () => setSaved(true));
on("saved", () => {
  setSaved(false);
  forgetPreviews();
  renderPreviews();
  renderPages();
});
on("save-error", () => setSaved(false, true));

// ------------------------------------------------------------------ start

function wireShell() {
  document.querySelectorAll("#side [data-view]").forEach((b) => (b.onclick = () => showPane(b.dataset.view)));
  $("btn-undo").onclick = undo;
  $("sel-profile").onchange = (e) => goto(+e.target.value, null, false);
  $("pf-add").onclick = addProfile;
  $("pf-edit").onclick = toggleEditor;
  $("pg-add").onclick = addPage;
  $("follow").onchange = (e) => edit("Follow program", (c) => { settingsOf(c).follow_active_app = e.target.checked; }, { redraw: false });
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
  await listen("status", (e) => {
    if (state.snap) state.snap.status = e.payload;
    showStatus(e.payload);
  });
  await listen("view", (e) => {
    const v = e.payload;
    const cur = state.view;
    if (v.profile !== cur.profile || v.page !== cur.page || v.remote !== cur.remote) {
      if (v.profile !== cur.profile) state.selState = 0;
      state.view = v;
      renderAll();
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
  await listen("integrations", (e) => {
    if (state.snap) state.snap.integrations = e.payload;
    // keep a field the user is typing in
    const typing = document.activeElement?.closest?.("#pkg-list input, #pkg-list select");
    if (state.pane === "packages" && !typing) renderPackages();
  });
  await listen("teams-signin", (e) => teamsSignedIn(e.payload));
  // the engine changed deck.json itself (Home Assistant set up MQTT over the Web API)
  await listen("config-changed", async () => {
    load(await invoke("get_state"));
    if (state.pane === "packages") renderPackages();
    toast("Settings changed by Home Assistant");
  });
  // packages were added, removed or reloaded: new catalog, everything redrawn
  await listen("packages", async () => {
    await loadCatalog();
    renderAll();
    if (state.pane === "packages") renderPackages();
  });
}

async function init() {
  buildGrid();
  wireDialogs();
  wireDrop();
  wireLibrary();
  wireSignals();
  wireShell();
  await loadCatalog();
  load(await invoke("get_state"));
  await listenAll();
  startLiveFaces();
}

init();
