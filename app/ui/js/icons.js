// Icon pickers: the whole Night Signal set (buttons, library) and the
// role-filtered picker for app keys.
import { h, put } from "./dom.js";
import { invoke } from "./api.js";
import { state } from "./store.js";
import { iconList, iconUrl, iconsForApp, withNs } from "./catalog.js";

const EMOJI = "💻 🧭 📸 🎵 🔊 🔇 🎙 📞 💡 🏠 🔒 🌡 ☕ 🍅 ⏱ 📅 ✉️ 💬 📝 📁 ⚙️ ✅ ❌ ⭐ ❤️ 🔥 🚀 🎮 🎬 📺 🌙 ☀ 🔔 👋".split(" ");

/** <img> for an icon id; a grid icon when the id is not a set icon. */
export const img = (id, cls) => h("img", { src: iconUrl(id) || iconUrl("grid"), alt: "", class: cls });

const roleName = (r) => {
  const s = r.includes(":") ? r.split(":").pop() : r;
  return s.charAt(0).toUpperCase() + s.slice(1);
};

/** Searchable grid of the icon set; `big` shows names (library view). Picks `com.deckling.core:` ids or emoji. */
export function iconPicker(onPick, big = false) {
  const grid = h("div", { class: "igrid" + (big ? " big" : "") });
  const search = h("input", { placeholder: `Search ${iconList().length} icons…`, class: big ? "lib-search" : "" });
  const box = h("div", { class: big ? "" : "ipick" }, search, grid);
  const draw = () => {
    const q = search.value.trim().toLowerCase();
    const icons = iconList().filter((i) => !q || i.name.includes(q) || (i.group || "").toLowerCase().includes(q));
    const groups = {};
    for (const i of icons) (groups[i.group || "Icons"] ||= []).push(i.name);
    put(grid,
      ...Object.entries(groups).map(([g, names]) => h("div", {},
        h("div", { class: "igroup" }, g),
        h("div", { class: "irow" }, names.map((n) =>
          h("button", { type: "button", title: n, onclick: () => onPick(withNs(n)) }, img(n), big ? h("small", {}, n.replace(/-/g, " ")) : null))))),
      q || big ? null : h("div", {}, h("div", { class: "igroup" }, "Emoji"),
        h("div", { class: "emojis" }, EMOJI.map((e) => h("button", { type: "button", onclick: () => onPick(e) }, e)))),
    );
    if (!icons.length && q) put(grid, h("p", { class: "sub" }, "No icon matches."));
  };
  search.oninput = draw;
  draw();
  return box;
}

/**
 * Icons for an app key: only those sharing a role with the app, richest
 * shared role first. Bound icons are previewed with the key's live values.
 * `current` is the chosen icon id (or empty for the default).
 */
export function appIconPicker(app, keyIndex, current, onPick) {
  const grid = h("div", { class: "igrid" });
  const search = h("input", { placeholder: "Search icons…" });
  const box = h("div", { class: "ipick" }, search, grid);
  const all = iconsForApp(app);
  const cur = withNs(current);
  const previews = new Map(); // bound icon id -> Promise<data URL>
  const thumb = (icon) => {
    const el = h("img", { alt: "", src: iconUrl(icon.id) || iconUrl("grid") });
    if (icon.bound) {
      if (!previews.has(icon.id)) {
        const { profile, page } = state.view;
        previews.set(icon.id, invoke("preview_icon", { icon: icon.id, profile, page, key: keyIndex }).catch(() => null));
      }
      previews.get(icon.id).then((url) => { if (url) el.src = url; });
    }
    return el;
  };
  const draw = () => {
    const q = search.value.trim().toLowerCase();
    const list = all.filter((x) => !q || x.icon.id.toLowerCase().includes(q));
    const groups = new Map();
    for (const x of list) {
      if (!groups.has(x.role)) groups.set(x.role, []);
      groups.get(x.role).push(x.icon);
    }
    put(grid,
      h("div", { class: "row wrap" },
        h("button", { type: "button", class: "btn small" + (!cur ? " on" : ""), disabled: !cur, onclick: () => onPick(null) },
          !cur ? "✓ Default icon" : "Default icon")),
      ...[...groups].map(([role, icons]) => h("div", {},
        h("div", { class: "igroup" }, roleName(role)),
        h("div", { class: "irow" }, icons.map((icon) =>
          h("button", { type: "button", title: icon.id, class: icon.id === cur ? "on" : "", onclick: () => onPick(icon.id) }, thumb(icon)))))),
    );
    if (!list.length && q) grid.append(h("p", { class: "sub" }, "No icon matches."));
  };
  search.oninput = draw;
  draw();
  return box;
}
