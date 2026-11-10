// The device grid: key previews (drawn by Rust), selection, image drops.
import { $, h, toast } from "./dom.js";
import { invoke, currentWebview } from "./api.js";
import { state, NUM_KEYS, STATUS_KEY, edit, render, keyObj, ensureKey, ensureState } from "./store.js";

export const keyEl = (k) => document.querySelector(`.key[data-key="${k}"]`);

export function buildGrid() {
  const grid = $("grid");
  for (let k = 0; k < NUM_KEYS; k++) {
    grid.append(
      h("div", { class: "key" + (k === STATUS_KEY ? " wide" : ""), "data-key": k, onclick: () => select(k) },
        h("img", { alt: "" }), h("span", { class: "num" }, k === STATUS_KEY ? "" : String(k))),
    );
  }
}

export async function renderPreview(k) {
  const { profile, page, remote } = state.view;
  const url = await invoke("preview", { profile, page, remote, key: k }).catch(() => null);
  if (!url) return;
  keyEl(k).querySelector("img").src = url;
  if (k === state.selected) {
    const face = document.querySelector("#insp-thumb img.face");
    if (face) face.src = url;
  }
}

export function renderPreviews() {
  for (let k = 0; k < NUM_KEYS; k++) renderPreview(k);
}

export function select(k) {
  if (k !== state.selected) state.selState = 0;
  state.selected = k;
  document.querySelectorAll(".key").forEach((el) => el.classList.toggle("selected", +el.dataset.key === k));
  render("inspector", "pane");
}

/** Live faces (clock, system monitor, pomodoro, media, notifications) while visible. */
export function startLiveFaces() {
  setInterval(() => {
    if (document.visibilityState !== "visible" || state.pane !== "editor") return;
    renderPreview(STATUS_KEY);
    if (state.view.remote) return;
    for (let k = 0; k < STATUS_KEY; k++) {
      const app = keyObj(k)?.app;
      if (app && app !== "core:button") renderPreview(k);
    }
  }, 1000);
}

/** Drop an image file on a key: it becomes a button with that image as icon. */
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
    document.querySelectorAll(".key.drop").forEach((el) => el.classList.remove("drop"));
    if (state.view.remote) return;
    if (p.type === "over" || p.type === "enter") {
      const k = keyAt(p.position);
      if (k !== null && k !== STATUS_KEY) keyEl(k).classList.add("drop");
    } else if (p.type === "drop") {
      const k = keyAt(p.position) ?? state.selected;
      if (k === STATUS_KEY) return;
      const file = (p.paths || []).find((f) => /\.(png|jpe?g|gif|webp|bmp)$/i.test(f));
      if (!file) return toast("Drop a PNG, JPEG, GIF or WebP image", true);
      const idx = k === state.selected ? state.selState : 0;
      select(k);
      edit(`Image on key ${k}`, () => {
        const key = ensureKey(k);
        delete key.app;
        delete key.config;
        ensureState(key, idx).icon = file;
      });
    }
  });
}
