// The library on the left: Apps, Actions and Icons.
import { $, h, put } from "./dom.js";
import { invoke } from "./api.js";
import { state, edit, hint, STATUS_KEY, keyObj, ensureKey, ensureState, keyKind, pageObj } from "./store.js";
import { appDef, byNamespace, iconsForApp, iconUrl, shortRole, withNs, nsOf, pluginOn } from "./catalog.js";
import { placeApp } from "./apps.js";
import { addAction } from "./actions.js";
import { keyHolder, keyWhere } from "./inspector.js";

const appRoles = (a) => [...new Set([...(a.roles || []).map(shortRole), "button"])].join(", ");

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
  document.querySelectorAll("#lib-tabs button").forEach((b) => b.classList.toggle("on", b.dataset.tab === state.libTab));
  const body = $("lib-body");
  if (state.libTab === "apps") put(body, appsTab());
  else if (state.libTab === "actions") put(body, actionsTab());
  else put(body, iconsTab());
}

function canPlace() {
  if (state.view.remote) return "The remote page is set by Home Assistant.";
  if (state.selected === STATUS_KEY) return "The wide key shows the profile and cards; pick one of keys 0 to 12.";
  return null;
}

// ------------------------------------------------------------------ apps

function appsTab() {
  const groups = byNamespace(state.catalog.apps);
  return [
    h("p", { class: "sub" }, "An app brings its default icon and its actions. Tap one to put it on the selected key, or drag it onto a key."),
    groups.map((g) => h("div", { class: "group" },
      h("div", { class: "group-title" }, g.title),
      g.items.map((a) => h("button", {
        type: "button", class: "item", draggable: true,
        ondragstart: (e) => e.dataTransfer.setData("text/plain", "app:" + a.id),
        onclick: () => {
          const why = canPlace();
          if (why) return hint(why);
          placeApp(a.id);
        },
      }, h("img", { alt: "", src: iconUrl(a.icon) || iconUrl("grid") }),
      h("span", { class: "grow" }, h("span", { class: "name" }, a.name), h("span", { class: "meta" }, appRoles(a))))))),
  ];
}

// ------------------------------------------------------------------ actions

function actionsTab() {
  const groups = byNamespace(state.catalog.actions.filter((a) => a.id !== "core:state"));
  return [
    h("p", { class: "sub" }, "Actions are what a plugin can do. Tapping one adds it to the section marked as target in the inspector."),
    groups.map((g) => h("div", { class: "group" },
      h("div", { class: "group-title" }, g.title),
      g.items.map((a) => h("button", {
        type: "button", class: "item", title: a.description || a.id,
        onclick: () => {
          if (state.view.remote) return hint("The remote page is set by Home Assistant.");
          const k = state.selected;
          addAction(a.id, keyHolder(k), keyWhere(k));
        },
      }, h("span", { class: "grow" }, h("span", { class: "name" }, a.name), h("span", { class: "meta" }, a.id)))))),
  ];
}

// ------------------------------------------------------------------ icons

let query = "";

function iconsTab() {
  const why = canPlace();
  if (why) return h("p", { class: "sub" }, why);
  const k = state.selected;
  const key = keyObj(k);
  const kind = keyKind(key);
  const app = kind === "app" ? appDef(key.app) : null;
  if (kind === "app" && !app) return h("p", { class: "sub" }, "The plugin of this key is missing, so its icons are unknown.");
  const roles = app ? app.roles : ["button"];
  const cur = withNs(kind === "app" ? key.states?.[0]?.icon : key?.states?.[state.selState]?.icon);
  const all = iconsForApp(app || { roles });
  const search = h("input", { placeholder: `Search ${all.length} icons`, value: query });
  const grid = h("div", {});
  const draw = () => {
    const q = search.value.trim().toLowerCase();
    query = search.value;
    const list = all.filter((x) => !q || x.icon.id.toLowerCase().includes(q));
    const groups = new Map();
    for (const x of list) {
      if (!groups.has(x.role)) groups.set(x.role, []);
      groups.get(x.role).push(x.icon);
    }
    put(grid, [...groups].map(([role, icons]) => h("div", { class: "group" },
      h("div", { class: "group-title" }, role === "button" && app ? "button (any icon)" : shortRole(role)),
      h("div", { class: "icon-grid" }, icons.slice(0, q ? 400 : 240).map((icon) =>
        h("button", { type: "button", class: "icon-cell" + (icon.id === cur ? " on" : ""), title: icon.id, onclick: () => pickIcon(k, kind, icon.id) },
          iconImg(icon.id, k, !!app && icon.bound), icon.id.split(":").pop().replace(/-/g, " ")))))));
    if (!list.length) put(grid, h("p", { class: "sub" }, "No icon matches."));
  };
  search.oninput = draw;
  draw();
  const sets = [...new Set(state.catalog.icons.map((i) => nsOf(i.id)))].filter(pluginOn);
  const setSel = h("select", {}, sets.map((s) => h("option", { value: s }, s)));
  return [
    h("p", { class: "sub" }, app
      ? `Icons that fit ${app.name}. Each one runs with this app's values. The richest shared role comes first.`
      : "A button takes any icon."),
    app ? h("button", { type: "button", class: "item" + (!cur ? " on" : ""), onclick: () => pickIcon(k, kind, null) },
      h("img", { alt: "", src: iconUrl(app.icon) || iconUrl("grid") }),
      h("span", { class: "grow" }, h("span", { class: "name" }, "Default icon"), h("span", { class: "meta" }, app.icon || "drawn by the app"))) : null,
    search,
    grid,
    h("div", { class: "box" },
      h("span", { class: "label" }, "Whole page"),
      h("div", { class: "row" }, setSel, h("button", { class: "btn", type: "button", onclick: () => applySet(setSel.value) }, "Apply set")),
      h("button", { class: "btn", type: "button", onclick: restoreDefaults }, "Restore default icons")),
  ];
}

function pickIcon(k, kind, id) {
  edit(`Icon · key ${k}`, () => {
    const key = ensureKey(k);
    const st = ensureState(key, kind === "app" ? 0 : state.selState);
    if (id) st.icon = id;
    else delete st.icon;
    if (id) delete st.image;
  });
  hint(id ? `Key ${k} uses ${id} now.` : `Key ${k} uses the default icon of its app again.`);
}

/** Every app key of the page takes the best icon of `set` that shares more than `button`. */
function applySet(set) {
  let n = 0;
  edit(`Apply icon set ${set}`, () => {
    for (const [k, key] of Object.entries(pageObj())) {
      if (keyKind(key) !== "app") continue;
      const app = appDef(key.app);
      if (!app) continue;
      const best = iconsForApp(app).find((x) => x.role !== "button" && nsOf(x.icon.id) === set);
      if (!best) continue;
      ensureState(key, 0).icon = best.icon.id;
      n++;
    }
  });
  hint(n ? `Set ${set} applied to ${n} key${n > 1 ? "s" : ""} where an icon shares more than button.` : `No key on this page has an app that fits an icon of ${set}.`);
}

function restoreDefaults() {
  edit("Restore default icons", () => {
    for (const key of Object.values(pageObj())) {
      if (keyKind(key) !== "app" || !key.states?.[0]) continue;
      delete key.states[0].icon;
    }
  });
  hint("All app keys of this page use the default icon of their plugin again.");
}

export function wireLibrary() {
  document.querySelectorAll("#lib-tabs button").forEach((b) => (b.onclick = () => { state.libTab = b.dataset.tab; renderLibrary(); }));
}

