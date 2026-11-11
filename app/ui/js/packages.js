// Packages view: every package with its kind, switch, state and settings (built
// from the package's `config` schema), and the Application card.
import { $, h, put, toast, ask } from "./dom.js";
import { invoke, call } from "./api.js";
import { state, edit, hint, saveNow } from "./store.js";
import { nsOf } from "./catalog.js";
import { schemaField } from "./fields.js";
import { checkUpdate } from "./updates.js";
import { openJson } from "./dialogs.js";

const KIND = { core: "Core", plugin: "Plugin", iconset: "Icon set", theme: "Theme", partial: "Theme, partial" };

const conf = (c, id) => {
  c.plugins ||= {};
  c.plugins[id] ||= {};
  c.plugins[id].config ||= {};
  return c.plugins[id].config;
};
const getPath = (o, path) => path.split(".").reduce((x, k) => (x == null ? undefined : x[k]), o);
function setPath(o, path, v) {
  const keys = path.split(".");
  const last = keys.pop();
  const parent = keys.reduce((x, k) => (x[k] && typeof x[k] === "object" ? x[k] : (x[k] = {})), o);
  if (v === undefined) delete parent[last];
  else parent[last] = v;
}

const open = new Set(["core"]); // expanded packages

export function renderPackages() {
  const plugins = state.catalog.plugins || [];
  const warnings = state.catalog.warnings || [];
  put($("pkg-list"),
    h("div", { class: "pkg-tools" },
      h("button", { class: "btn", type: "button", onclick: () => call("open_packages_dir") }, "Open packages folder"),
      h("button", { class: "btn", type: "button", onclick: reloadPackages }, "Reload packages")),
    h("div", { class: "drop-zone", id: "pkg-drop" }, "Drop an icon set or theme folder here to add it. Each package is a folder with a deckling.json; it is copied to the packages folder."),
    warnings.length ? h("details", { class: "note warn" }, h("summary", {}, `${warnings.length} package warning${warnings.length > 1 ? "s" : ""}`), h("ul", {}, warnings.map((w) => h("li", { class: "sub" }, w)))) : null,
    plugins.map(pkgCard));
  renderAppCard();
}

/** Read the packages folder again, then the catalog. */
export async function reloadPackages() {
  const warnings = await call("reload_packages");
  hint(warnings.length ? `Packages reloaded, ${warnings.length} warnings.` : "Packages reloaded.");
}

/** Folders dropped on the Packages view become packages. */
export async function installPaths(paths) {
  for (const path of paths) {
    try {
      const id = await invoke("install_package", { path });
      toast(`${id} added`);
    } catch (e) {
      toast(String(e), true);
    }
  }
}

/** Is the package switched on? Plugins and icon sets: plugins.<id>.enabled; themes: active or an overlay. */
function packageOn(p) {
  if (p.id === "core") return true;
  if (p.type === "theme") {
    const th = (state.catalog.themes || []).find((t) => t.id === p.id);
    return th?.kind === "full" ? state.config.theme === p.id : (state.config.overlays || []).includes(p.id);
  }
  return state.config.plugins?.[p.id]?.enabled !== false;
}

function setPackageOn(p, on) {
  const th = (state.catalog.themes || []).find((t) => t.id === p.id);
  edit(`${p.name} ${on ? "on" : "off"}`, (c) => {
    if (p.type === "theme" && th?.kind === "full") {
      if (on) c.theme = p.id;
      else delete c.theme;
    } else if (p.type === "theme") {
      const list = (c.overlays || []).filter((x) => x !== p.id);
      if (on) list.push(p.id);
      if (list.length) c.overlays = list;
      else delete c.overlays;
    } else {
      c.plugins ||= {};
      c.plugins[p.id] ||= {};
      c.plugins[p.id].enabled = on;
    }
  });
  hint(`${p.name} is ${on ? "on" : "off"}.`);
}

function pkgCard(p) {
  const on = packageOn(p);
  const sw = h("input", { type: "checkbox", class: "switch", checked: on, disabled: p.id === "core", "aria-label": `${p.id} ${on ? "off" : "on"}` });
  sw.onchange = () => setPackageOn(p, sw.checked);
  const th = (state.catalog.themes || []).find((t) => t.id === p.id);
  const isOpen = open.has(p.id);
  const icons = state.catalog.icons.filter((i) => nsOf(i.id) === p.id).length;
  const st = on ? status(p.id) : null;
  return h("div", { class: "pkg" },
    h("div", { class: "pkg-head" },
      h("div", { class: "grow" },
        h("div", { class: "kind" }, th?.kind === "partial" ? KIND.partial : KIND[p.type] || p.type, icons ? ` · ${icons} icons` : "", p.path ? " · from the packages folder" : " · built in"),
        h("h3", {}, p.name),
        h("div", { class: "id" }, p.id + (p.version ? ` · ${p.version}` : "")),
        h("p", { class: "sub" }, p.description || ""),
        st ? h("div", { class: "status" }, h("span", { class: "tag " + (st.ok ? "ok" : "warn") }, st.text), st.note ? h("span", { class: "sub" }, st.note) : null) : null),
      h("div", { class: "row" },
        on && (p.config?.length || SETUP[p.id]) ? h("button", { class: "btn small", type: "button", onclick: () => { isOpen ? open.delete(p.id) : open.add(p.id); renderPackages(); } }, isOpen ? "Close" : "Settings") : null,
        p.path ? h("button", { class: "btn small danger", type: "button", onclick: async () => {
          if (!(await ask(`Remove ${p.name} (${p.id}) from the packages folder?`, null))) return;
          await call("remove_package", { id: p.id }).then(() => hint(`${p.name} removed.`)).catch(() => {});
        } }, "Remove") : null,
        h("label", { class: "switch-row", title: p.id === "core" ? "The core is always on" : th?.kind === "full" ? "The active theme" : "" }, sw, on ? "on" : "off"))),
    on && isOpen ? h("div", { class: "pkg-body" }, SETUP[p.id]?.(), settings(p)) : null);
}

/** The settings from the package's `config` schema, grouped by `group`. */
function settings(p) {
  const groups = new Map();
  for (const f of p.config || []) {
    const g = f.group || "";
    if (!groups.has(g)) groups.set(g, []);
    groups.get(g).push(f);
  }
  return [...groups].map(([g, fields]) => [
    g ? h("h4", {}, g) : null,
    h("div", { class: "fields" }, fields.map((f) => field(p, f))),
  ]);
}

function field(p, f) {
  const read = () => getPath(conf(state.config, p.id), f.key);
  if (f.type === "place") return placeField(p, f, read);
  if (f.type === "token") return tokenField(p, f, read);
  const shown = { ...f, type: f.secret ? "password" : f.type };
  return schemaField(shown, read, (v, typing) => {
    if (f.type === "number" && v !== undefined) v = Math.min(f.max ?? Infinity, Math.max(f.min ?? -Infinity, v));
    edit(`${p.name} · ${f.label || f.key}`, (c) => setPath(conf(c, p.id), f.key, v === undefined ? f.default : v),
      { redraw: false, coalesce: typing ? `pkg:${p.id}:${f.key}` : null });
  }, { labelled: true });
}

/** A secret token: hidden, with Generate (random, 32 hex digits) and Copy. */
function tokenField(p, f, read) {
  const input = h("input", { type: "password", autocomplete: "off", value: read() || "", placeholder: "Generate one" });
  const save = (v, label) => edit(`${p.name} · ${f.label}`, (c) => setPath(conf(c, p.id), f.key, v || undefined), { redraw: false, coalesce: label ? null : `pkg:${p.id}:${f.key}` });
  input.oninput = () => save(input.value.trim());
  const generate = () => {
    const b = new Uint8Array(16);
    crypto.getRandomValues(b);
    input.value = [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
    save(input.value, "generate");
    hint("New token. Scripts using the old one stop working.");
    renderPackages();
  };
  return h("div", { class: "field" },
    h("span", { class: "label" }, f.label),
    input,
    h("div", { class: "row" },
      h("button", { class: "btn small", type: "button", onclick: generate }, "Generate"),
      h("button", { class: "btn small", type: "button", onclick: () => { input.type = input.type === "password" ? "text" : "password"; } }, "Show"),
      h("button", { class: "btn small", type: "button", onclick: () => navigator.clipboard.writeText(input.value).then(() => toast("Token copied")) }, "Copy")));
}

/** A place for the weather: search by name (Open-Meteo), store {name, lat, lon}. */
function placeField(p, f, read) {
  const cur = read();
  const q = h("input", { placeholder: "Search a city" });
  const list = h("ul", { class: "results" });
  const search = async () => {
    const text = q.value.trim();
    if (!text) return;
    put(list, h("li", {}, "Searching…"));
    try {
      const res = await invoke("geocode", { query: text });
      put(list, res.length ? res.map((r) => h("li", { onclick: () => {
        edit(`${p.name} · ${f.label}`, (c) => setPath(conf(c, p.id), f.key, { name: r.name, lat: r.lat, lon: r.lon }));
        hint(`Weather for ${r.name}.`);
        renderPackages();
      } }, h("span", {}, r.name), h("span", { class: "sub" }, r.detail))) : h("li", {}, "No places found."));
    } catch (e) {
      put(list, h("li", {}, String(e)));
    }
  };
  q.onkeydown = (e) => { if (e.key === "Enter") { e.preventDefault(); search(); } };
  return h("div", { class: "field", style: "grid-column: 1 / -1" },
    h("span", { class: "label" }, `${f.label}: ${cur?.name || "not set"}`),
    h("div", { class: "row" }, q, h("button", { class: "btn", type: "button", onclick: search }, "Search")),
    list,
    h("span", { class: "sub" }, "From Open-Meteo, no account. Only the coordinates of the place are sent."));
}

// ------------------------------------------------------------------ state and setup per plugin

function status(id) {
  const s = state.snap?.status || {};
  const i = state.snap?.integrations || {};
  if (id === "core") return s.connected ? { ok: true, text: "D200 connected" } : { ok: false, text: "D200 not connected", note: s.message };
  if (id === "com.deckling.ha") {
    const m = state.config.plugins?.core?.config?.mqtt || {};
    if (!m.enabled) return { ok: false, text: "MQTT off", note: "Switch it on under Deck, MQTT." };
    return s.mqtt_ok ? { ok: true, text: "Connected", note: s.mqtt } : { ok: false, text: "Connecting", note: s.mqtt };
  }
  if (id === "com.deckling.teams") {
    const t = i.teams || {};
    const step = !t.connected ? "Setup needed" : !i.signedIn ? "Meeting controls ready" : "Connected";
    return { ok: !!t.connected, text: step, note: i.signedIn && i.account ? `Signed in as ${i.account}` : "" };
  }
  if (id === "com.deckling.web") {
    const w = i.web || {};
    const c = state.config.plugins?.[id]?.config || {};
    if (w.listening) return { ok: true, text: `Listening on port ${w.listening}`, note: (w.addresses || []).join("  ·  ") };
    if (w.error) return { ok: false, text: "Not running", note: w.error };
    return { ok: false, text: c.serve ? "Starting" : "Off", note: c.serve ? "" : "Switch on Run the web server in its settings." };
  }
  if (id === "com.deckling.jetbrains") return { ok: true, text: i.debugging ? `Debugging in ${i.debugging}` : "Watching", note: "Looks while a JetBrains IDE is in front." };
  if (id === "com.deckling.core") return i.playing ? { ok: true, text: i.playing.playing ? "Playing" : "Paused", note: `${i.playing.title || ""} ${i.playing.artist ? "· " + i.playing.artist : ""}` } : null;
  return null;
}

let deviceCode = "";

/** Guided setup in steps, for plugins that need one. */
const SETUP = {
  core: () => h("div", { class: "row wrap" },
    h("button", { class: "btn small", type: "button", onclick: async () => {
      await saveNow();
      await call("screensaver", { on: true });
      hint("Screensaver on the device. Press any key to wake it.");
    } }, "Preview screensaver"),
    h("button", { class: "btn small", type: "button", onclick: () => $("btn-notify").click() }, "Test notification"),
    h("button", { class: "btn small", type: "button", onclick: () => call("start_firmware").then((m) => toast(m)) }, "Start d200fw on the device")),
  "com.deckling.teams": () => {
    const i = state.snap?.integrations || {};
    const t = i.teams || {};
    const cfg = state.config.plugins?.["com.deckling.teams"]?.config || {};
    const step = (done, n, title, ...body) => h("div", { class: "step" + (done ? " done" : "") }, h("span", { class: "n" }, done ? "✓" : String(n)),
      h("div", { class: "grow" }, h("b", {}, title), ...body));
    const signIn = async () => {
      deviceCode = "Contacting Microsoft…";
      renderPackages();
      try {
        const r = await invoke("teams_sign_in", { clientId: (cfg.client_id || "").trim(), tenant: (cfg.tenant || "").trim() || "organizations" });
        deviceCode = `Enter the code ${r.user_code} at ${r.verification_uri} (opened in your browser). This finishes by itself.`;
        navigator.clipboard?.writeText(r.user_code).catch(() => {});
      } catch (e) {
        deviceCode = String(e);
      }
      renderPackages();
    };
    return h("div", { class: "steps" },
      step(!!t.connected, 1, "Meeting controls",
        h("p", { class: "sub" }, "In Teams: Settings, Privacy, Manage API: switch on the API for third-party apps. The first time you press a Teams key in a meeting, Teams asks to allow D200 Deck."),
        h("p", { class: "sub" }, t.connected ? (t.paired ? "Connected and paired." : "Connected. Teams asks to pair on the first press in a meeting.") : `Not connected${t.error ? ": " + t.error : ""}.`)),
      step(!!i.signedIn, 2, "Presence, mail and calendar (optional)",
        h("p", { class: "sub" }, "Register an app in Microsoft Entra (single tenant), allow public client flows, and add the delegated Microsoft Graph permissions Presence.ReadWrite, Mail.ReadBasic and Calendars.Read. Enter its client ID and tenant below, then sign in."),
        h("div", { class: "row wrap" },
          i.signedIn
            ? h("button", { class: "btn small", type: "button", onclick: () => call("teams_sign_out") }, "Sign out")
            : h("button", { class: "btn small primary", type: "button", disabled: !cfg.client_id, onclick: signIn }, "Sign in")),
        deviceCode && !i.signedIn ? h("p", { class: "sub" }, deviceCode) : null));
  },
  "com.deckling.web": () => {
    const w = state.snap?.integrations?.web || {};
    const c = state.config.plugins?.["com.deckling.web"]?.config || {};
    const url = (w.addresses || [])[1] || (w.addresses || [])[0] || `http://<this computer>:${c.port || 8765}/api`;
    const base = url.replace(/\/api$/, "");
    return h("div", { class: "steps" },
      h("p", { class: "sub" }, "Anyone on the network with the token can drive the deck. Keep the token secret; generate a new one to lock out old scripts. Actions that start programs or type are refused unless you allow them below."),
      h("pre", { class: "code" }, `# a key: put the app "Web key" on a key and pick the id "build"
curl -X PUT -H "Authorization: Bearer $TOKEN" \\
  -d '{"label": "Build", "kind": "sensor", "value": "green", "on": true}' \\
  ${base}/api/keys/build

# a notification on the wide key
curl -X POST -H "Authorization: Bearer $TOKEN" \\
  -d '{"title": "Deploy", "text": "done"}' ${base}/api/notify

# a signal that themes and icons can bind to: com.deckling.web:ci.status
curl -X PUT -H "Authorization: Bearer $TOKEN" -d '"failed"' ${base}/api/signals/ci.status

# press key 3, run an action, read every signal
curl -X POST -H "Authorization: Bearer $TOKEN" ${base}/api/keys/3/press
curl -X POST -H "Authorization: Bearer $TOKEN" -d '{"params": {"page": "main"}}' ${base}/api/actions/core:page
curl -H "Authorization: Bearer $TOKEN" ${base}/api/signals`));
  },
  "com.deckling.ha": () => {
    const base = state.config.plugins?.core?.config?.mqtt?.base_topic || "d200";
    return h("div", { class: "steps" },
      h("p", { class: "sub" }, "The connection to the broker is part of the core (Deck, MQTT). With discovery on, Home Assistant gets a Ulanzi D200 device with button triggers, a notify entity, a profile select and brightness."),
      h("p", { class: "sub" }, "In Home Assistant, add the integration D200 Deck (custom_components/d200_deck in this repository, or through HACS) and pick the entities for the deck. Then put the app Home Assistant on a key and choose the entity's id. A press runs its default action."),
      h("p", { class: "sub" }, `Without the integration, Home Assistant describes a key on ${base}/ha/<id>/set (label, value, on, color, icon); presses come back on ${base}/ha/<id>/action.`));
  },
};

/** Sign-in result (event from Rust). */
export function teamsSignedIn(p) {
  deviceCode = p.ok ? "" : "Sign-in failed: " + p.error;
  if (p.ok) toast("Signed in to Microsoft 365");
  if (state.pane === "packages") renderPackages();
}

// ------------------------------------------------------------------ Application

let autostart = null;
let version = "";
let updateText = "";

async function renderAppCard() {
  if (autostart === null) {
    autostart = await invoke("autostart_get").catch(() => false);
    version = await invoke("app_version").catch(() => "");
  }
  const auto = h("input", { type: "checkbox", class: "switch", checked: autostart });
  auto.onchange = async () => {
    autostart = await invoke("autostart_set", { on: auto.checked }).catch((e) => { toast(String(e), true); return !auto.checked; });
    hint(`D200 Deck ${autostart ? "starts" : "does not start"} at login.`);
  };
  put($("app-card"),
    h("h3", {}, "Application"),
    h("p", { class: "sub" }, "What belongs to no plugin."),
    h("label", { class: "switch-row" }, auto, "Start at login (in the menu bar)"),
    h("div", { class: "row wrap" },
      h("button", { class: "btn", type: "button", onclick: openJson }, "Export or import deck.json"),
      h("button", { class: "btn", type: "button", onclick: () => call("open_config_dir") }, "Open config folder")),
    h("span", { class: "sub mono" }, state.snap?.configPath || ""),
    h("div", { class: "row wrap" },
      h("span", { class: "sub" }, `Version ${version} ${updateText}`),
      h("button", { class: "btn small", type: "button", onclick: async () => {
        updateText = "· checking…";
        renderAppCard();
        updateText = await checkUpdate();
        renderAppCard();
      } }, "Check for updates")));
}

