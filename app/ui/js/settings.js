// The Settings dialog (general, weather, pomodoro, Home Assistant, Teams,
// wide key & screensaver, theme). Saving it is one undo step.
import { $, h, put, toast } from "./dom.js";
import { invoke, call } from "./api.js";
import { state, edit, saveNow, render, settingsOf } from "./store.js";

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
let themeDraft = null; // edited copy of settings.theme; applied on Save

export async function openSettings() {
  const s = settingsOf(state.config);
  const snap = state.snap;
  $("s-long").value = s.long_press_ms;
  $("s-follow").checked = s.follow_active_app;
  $("s-autostart").checked = await invoke("autostart_get").catch(() => false);
  $("s-path").textContent = snap?.configPath || "";
  pendingLocation = s.weather.location;
  $("w-current").textContent = pendingLocation ? pendingLocation.name : "not set";
  $("w-f").checked = s.weather.fahrenheit;
  put($("w-results"));
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
  themeDraft = structuredClone(s.theme || { palette: {}, widgets: {} });
  renderThemeEditor();
  showIntegrations(snap?.integrations);
  $("s-error").textContent = "";
  $("dlg-settings").showModal();
}

/** Open the Theme tab on one widget (from the inspector). */
export function showThemeWidget(wid) {
  document.querySelector('#set-tabs [data-tab="theme"]').click();
  $("th-widget").value = wid;
  $("th-widget").onchange();
}

async function saveSettings() {
  const num = (id, lo, hi) => Math.min(hi, Math.max(lo, parseInt($(id).value, 10) || lo));
  edit("Settings", (c) => {
    const s = settingsOf(c);
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
    if (themeDraft) {
      for (const [k, v] of Object.entries(themeDraft.widgets || {})) if (!Object.keys(v).length) delete themeDraft.widgets[k];
      s.theme = themeDraft;
      themeDraft = structuredClone(themeDraft);
    }
  }, { save: false });
  try {
    await invoke("autostart_set", { on: $("s-autostart").checked }).catch(() => {});
    await saveNow({ quiet: true });
    $("dlg-settings").close();
    render("all");
    toast("Settings saved");
  } catch (e) {
    $("s-error").textContent = String(e);
  }
}

export function wireSettings() {
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
    put(ul, h("li", {}, h("span", { class: "sub" }, "Searching…")));
    try {
      const res = await invoke("geocode", { query: q });
      put(ul, res.length ? res.map((r) =>
        h("li", { onclick: () => { pendingLocation = { name: r.name, lat: r.lat, lon: r.lon }; $("w-current").textContent = `${r.name} (${r.detail})`; put(ul); } },
          h("span", {}, r.name), h("span", { class: "sub" }, r.detail))) : h("li", {}, h("span", { class: "sub" }, "No places found")));
    } catch (e) {
      put(ul, h("li", {}, h("span", { class: "sub" }, String(e))));
    }
  };
  $("w-search").onclick = search;
  $("w-query").onkeydown = (e) => { if (e.key === "Enter") { e.preventDefault(); search(); } };
  $("m-base").oninput = () => ($("m-help").innerHTML = mqttHelp($("m-base").value.trim() || "d200"));
  wireTeams();
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
  const t = themeDraft;
  t.widgets ||= {};
  const pal = (t.palette ||= {});
  put($("th-palette"),
    themeColor(pal, "active", "Active", "#69E0C1"), themeColor(pal, "info", "Info", "#B79AF7"),
    themeColor(pal, "attention", "Attention", "#FFBF69"), themeColor(pal, "error", "Error", "#FF8278"),
    themeColor(pal, "background", "Surface", "#11171D"));
  const sel = $("th-widget");
  const drawWidget = () => {
    const id = sel.value;
    const w = (themeDraft.widgets[id] ||= {});
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
  if (!sel.options.length) sel.replaceChildren(...WIDGETS.map(([v, n]) => h("option", { value: v }, n)));
  $("th-reset").onclick = () => { delete themeDraft.widgets[sel.value]; drawWidget(); };
  sel.onchange = drawWidget;
  drawWidget();
}

// ------------------------------------------------------------------ Teams

export function showIntegrations(i) {
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

/** Sign-in result event. */
export function teamsSignedIn(p) {
  $("t-code").hidden = false;
  $("t-code").textContent = p.ok ? "Signed in." : "Sign-in failed: " + p.error;
  if (p.ok) toast("Signed in to Microsoft 365");
}
