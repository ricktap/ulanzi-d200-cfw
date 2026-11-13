// The library on the left: the apps a key can show. Icons and actions are picked in
// a dialog from the inspector (picker.js).
import { $, h, put } from "./dom.js";
import { invoke } from "./api.js";
import { state, hint, STATUS_KEY } from "./store.js";
import { byPlugin, iconUrl } from "./catalog.js";
import { placeApp } from "./apps.js";

/** The first sentence of an app's description. */
const short = (a) => (a.description || "").split(/(?<=\.)\s/)[0];

/** Icon image for a catalog icon: the Night Signal SVG, or (other sets, bound icons) drawn by Rust. */
const previews = new Map();
export function iconImg(id, keyIndex = state.selected, live = false) {
  const url = iconUrl(id);
  const el = h("img", { alt: "", src: url || iconUrl("grid") });
  if (live || !url) {
    const { profile, page } = state.view;
    const ck = `${id}|${profile}|${page}|${keyIndex}`;
    if (!previews.has(ck)) previews.set(ck, invoke("preview_icon", { icon: id, profile, page, key: keyIndex }).catch(() => null));
    previews.get(ck).then((u) => { if (u) el.src = u; });
  }
  return el;
}
/** Live previews go stale when values change; the library asks again when it redraws. */
export const forgetPreviews = () => previews.clear();

export function renderLibrary() {
  const q = ($("lib-search").value || "").trim().toLowerCase();
  const groups = byPlugin(state.catalog.apps)
    .map((g) => ({ ...g, items: g.items.filter((a) => !q || a.name.toLowerCase().includes(q) || (a.description || "").toLowerCase().includes(q)) }))
    .filter((g) => g.items.length);
  put($("lib-body"), groups.length ? groups.map((g) => h("div", { class: "group" },
    h("div", { class: "group-title" }, g.title),
    g.items.map((a) => h("button", {
      type: "button", class: "item", draggable: true, title: a.description || a.name,
      ondragstart: (e) => e.dataTransfer.setData("text/plain", "app:" + a.id),
      onclick: () => {
        const why = canPlace();
        if (why) return hint(why);
        placeApp(a.id);
      },
    }, h("img", { alt: "", src: iconUrl(a.icon) || iconUrl("grid") }),
    h("span", { class: "grow" }, h("span", { class: "name" }, a.name), h("span", { class: "meta" }, short(a))))))) : h("p", { class: "sub" }, "No app matches."));
}

function canPlace() {
  if (state.view.remote) return "The remote page is set by Home Assistant.";
  if (state.selected === STATUS_KEY) return "The wide key shows the profile and cards; pick one of keys 0 to 12.";
  return null;
}

export function wireLibrary() {
  $("lib-search").oninput = renderLibrary;
}
