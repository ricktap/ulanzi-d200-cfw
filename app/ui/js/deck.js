// The device in the middle: key previews (drawn by Rust, so they match the
// device), selection, dragging apps and keys, image drops and the page tabs.
import { $, h, put, toast } from "./dom.js";
import { invoke, listen, currentWebview } from "./api.js";
import { state, NUM_KEYS, STATUS_KEY, edit, hint, render, goto, profile, keyObj, ensureKey, ensureState, keyKind } from "./store.js";
import { appAvailable } from "./catalog.js";
import { placeApp, swapKeys } from "./apps.js";
import { installPaths } from "./packages.js";

export const keyEl = (k) => document.querySelector(`.key[data-key="${k}"]`);

export function buildGrid() {
  const grid = $("grid");
  for (let k = 0; k < NUM_KEYS; k++) {
    const el = h("div", {
      class: "key" + (k === STATUS_KEY ? " wide" : ""), "data-key": k, draggable: k !== STATUS_KEY,
      role: "button", tabindex: 0,
      onclick: () => select(k),
      onkeydown: (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); select(k); } },
      ondragstart: (e) => e.dataTransfer.setData("text/plain", "key:" + k),
      ondragover: (e) => { if (k !== STATUS_KEY && !state.view.remote) { e.preventDefault(); el.classList.add("over"); } },
      ondragleave: () => el.classList.remove("over"),
      ondrop: (e) => {
        e.preventDefault();
        el.classList.remove("over");
        const d = e.dataTransfer.getData("text/plain") || "";
        if (d.startsWith("app:")) {
          state.selected = k;
          placeApp(d.slice(4), k);
        } else if (d.startsWith("key:")) {
          swapKeys(+d.slice(4), k);
        }
      },
    }, h("img", { alt: "" }));
    grid.append(el);
  }
}

export async function renderPreview(k) {
  const { profile: p, page, remote } = state.view;
  const url = await invoke("preview", { profile: p, page, remote, key: k }).catch(() => null);
  if (!url) return;
  keyEl(k).querySelector("img").src = url;
  if (k === state.selected) {
    const t = document.querySelector("#insp .insp-thumb img");
    if (t) t.src = url;
  }
}

/** Key frames: empty (dashed), plugin missing (amber, dashed), selected. */
function markKeys() {
  for (let k = 0; k < NUM_KEYS; k++) {
    const el = keyEl(k);
    const key = state.view.remote || k === STATUS_KEY ? null : keyObj(k);
    const kind = keyKind(key);
    const miss = kind === "app" && !appAvailable(key.app);
    el.classList.toggle("empty", !state.view.remote && k !== STATUS_KEY && kind === "empty");
    el.classList.toggle("missing", miss);
    el.classList.toggle("selected", k === state.selected);
    el.title = (k === STATUS_KEY ? "Wide key" : `Key ${k}`) + (miss ? " · plugin missing" : "");
  }
}

export function renderPreviews() {
  markKeys();
  for (let k = 0; k < NUM_KEYS; k++) renderPreview(k);
}

export function select(k) {
  if (k !== state.selected) {
    state.selState = 0;
    state.slot = "press";
  }
  state.selected = k;
  markKeys();
  render("inspector", "library");
}

// ------------------------------------------------------------------ pages and profiles above and below

export function renderPages() {
  const v = state.view;
  const names = Object.keys(profile().pages);
  const tabs = names.map((n, i) => h("button", {
    type: "button", class: !v.remote && v.page === n ? "on" : "", title: i === 0 ? "Start page" : "",
    onclick: () => goto(v.profile, n, false),
  }, (i === 0 ? "⌂ " : "") + n));
  const mqttOn = state.config.plugins?.core?.config?.mqtt?.enabled;
  if (mqttOn || v.remote) {
    tabs.push(h("button", { type: "button", class: v.remote ? "on" : "", title: "Keys set by Home Assistant",
      onclick: () => goto(v.profile, v.page, true) }, "Remote"));
  }
  put($("page-tabs"), tabs);
  const sel = $("sel-profile");
  put(sel, state.config.profiles.map((p, i) => h("option", { value: i }, p.name + (i === 0 ? " (default)" : ""))));
  sel.value = v.profile;
  $("pf-edit").textContent = state.editProfile ? "Done" : "Edit profile";
}

/**
 * Live faces: the engine reports each key whose face changed ("face" event); the grid
 * fetches only those previews, at most once a second (every 3 s while the window is not
 * focused), and nothing while it is hidden or another view is open.
 */
export function startLiveFaces() {
  const dirty = new Set();
  let last = 0;
  listen("face", (e) => dirty.add(e.payload.key));
  setInterval(() => {
    if (!dirty.size || document.visibilityState !== "visible" || state.pane !== "deck") return;
    const now = Date.now();
    if (!document.hasFocus() && now - last < 3000) return;
    last = now;
    for (const k of dirty) if (k === STATUS_KEY || !state.view.remote) renderPreview(k);
    dirty.clear();
  }, 1000);
}

/** Drop an image file on a key: it becomes a button with that image. */
export function wireDrop() {
  const wv = currentWebview();
  if (!wv) return;
  const keyAt = (pos) => {
    if (!pos) return null;
    const r = window.devicePixelRatio || 1;
    const el = document.elementFromPoint(pos.x / r, pos.y / r)?.closest(".key");
    return el ? +el.dataset.key : null;
  };
  wv.onDragDropEvent((ev) => {
    const p = ev.payload;
    document.querySelectorAll(".key.over").forEach((el) => el.classList.remove("over"));
    if (state.pane === "packages") {
      document.getElementById("pkg-drop")?.classList.toggle("over", p.type === "over" || p.type === "enter");
      if (p.type === "drop") installPaths(p.paths || []);
      return;
    }
    if (state.pane !== "deck" || state.view.remote) return;
    if (p.type === "over" || p.type === "enter") {
      const k = keyAt(p.position);
      if (k !== null && k !== STATUS_KEY) keyEl(k).classList.add("over");
    } else if (p.type === "drop") {
      const file = (p.paths || []).find((f) => /\.(png|jpe?g|gif|webp|bmp)$/i.test(f));
      if (!file) return;
      const k = keyAt(p.position) ?? state.selected;
      if (k === STATUS_KEY) return toast("The wide key takes no image", true);
      select(k);
      edit(`Image on key ${k}`, () => {
        const key = ensureKey(k);
        delete key.app;
        delete key.config;
        ensureState(key, state.selState).icon = file;
      });
      hint(`Key ${k} shows the image now.`);
    }
  });
}
