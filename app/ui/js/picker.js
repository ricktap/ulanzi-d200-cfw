// The picker dialog: choose an icon or an action. It opens over the page, so the
// choice is made where it was asked for (a row of the inspector) and nowhere else.
import { $, h, put } from "./dom.js";
import { state, edit, hint, keyKind, pageObj, ensureState } from "./store.js";
import { appDef, byPlugin, iconList, iconsForApp, iconUrl, nsOf, pluginOn, CORE_PLUGIN } from "./catalog.js";
import { iconImg } from "./library.js";

let query = "";

/**
 * Open the picker. `draw(q, choose)` returns the content for search text `q`;
 * `choose(value)` closes with that value. Resolves undefined when closed without a choice.
 */
function open({ title, sub, placeholder, draw }) {
  return new Promise((resolve) => {
    const d = $("dlg-pick");
    const search = $("pick-search");
    let result;
    const choose = (v) => { result = { v }; d.close(); };
    const redraw = () => put($("pick-body"), draw(search.value.trim().toLowerCase(), choose));
    $("pick-title").textContent = title;
    $("pick-sub").textContent = sub || "";
    $("pick-sub").hidden = !sub;
    search.placeholder = placeholder;
    search.value = query;
    search.oninput = () => { query = search.value; redraw(); };
    $("pick-close").onclick = () => d.close();
    d.onclose = () => resolve(result ? result.v : undefined);
    redraw();
    d.showModal();
    search.focus();
    search.select();
  });
}

const nice = (id) => id.split(":").pop().replace(/-/g, " ");

const EMOJI = "💻 🧭 📸 🎵 🔊 🔇 🎙 📞 💡 🏠 🔒 🌡 ☕ 🍅 ⏱ 📅 ✉️ 💬 📝 📁 ⚙️ ✅ ❌ ⭐ ❤️ 🔥 🚀 🎮 🎬 📺 🌙 ☀ 🔔 👋".split(" ");

/**
 * Choose an icon. Resolves an icon id, `null` for the first choice (`reset`: "Default icon",
 * "Same as on" ...), or undefined when closed.
 * opts: {title, sub, app (icons that follow its values come first), current, reset: {label, icon, note},
 *        emoji (buttons take any emoji), k (the key, for live previews), page (whole-page tools)}
 */
export function pickIcon({ title, sub, app = null, current = null, reset = null, emoji = false, k = state.selected, page = false }) {
  const all = iconsForApp(app || { roles: ["button"] });
  const groupOf = new Map(iconList().map((i) => [`${CORE_PLUGIN}:${i.name}`, i.group]));
  const plugins = new Map((state.catalog.plugins || []).map((p) => [p.id, p.name]));
  return open({
    title, sub, placeholder: `Search ${all.length} icons`,
    draw: (q, choose) => {
      const list = all.filter((x) => !q || nice(x.icon.id).includes(q) || x.icon.id.toLowerCase().includes(q));
      const cell = ({ icon }, live) => h("button", {
        type: "button", class: "icon-cell" + (icon.id === current ? " on" : ""), title: icon.id, onclick: () => choose(icon.id),
      }, iconImg(icon.id, k, live), nice(icon.id));
      // icons that change with the app's values first, then every icon by its group
      const fits = app ? list.filter((x) => x.role !== "button") : [];
      const order = [...new Set(iconList().map((i) => i.group))];
      const rank = (x) => { const i = order.indexOf(groupOf.get(x.icon.id)); return i < 0 ? order.length : i; };
      const groups = new Map();
      for (const x of [...list].sort((a, b) => rank(a) - rank(b))) {
        const g = groupOf.get(x.icon.id) || plugins.get(nsOf(x.icon.id)) || nsOf(x.icon.id);
        if (!groups.has(g)) groups.set(g, []);
        groups.get(g).push(x);
      }
      return [
        reset && !q ? h("button", { type: "button", class: "item" + (!current ? " on" : ""), onclick: () => choose(null) },
          reset.icon ? iconImg(reset.icon, k) : h("span", { class: "noicon" }, "–"),
          h("span", { class: "grow" }, h("span", { class: "name" }, reset.label), h("span", { class: "meta" }, reset.note || ""))) : null,
        fits.length ? h("div", { class: "group" },
          h("div", { class: "group-title" }, `Follow ${app.name}: these change with its state or value`),
          h("div", { class: "icon-grid" }, fits.map((x) => cell(x, true)))) : null,
        [...groups].map(([g, icons]) => h("div", { class: "group" },
          h("div", { class: "group-title" }, g),
          h("div", { class: "icon-grid" }, icons.map((x) => cell(x, false))))),
        emoji && !q ? h("div", { class: "group" },
          h("div", { class: "group-title" }, "Emoji"),
          h("div", { class: "emoji-grid" }, EMOJI.map((e) => h("button", { type: "button", class: current === e ? "on" : "", onclick: () => choose(e) }, e)))) : null,
        list.length ? null : h("p", { class: "sub" }, "No icon matches."),
        page && !q ? pageTools(() => $("dlg-pick").close()) : null,
      ];
    },
  });
}

/** Icons for every app key of the page at once: an icon set, or the defaults again. */
function pageTools(done) {
  const sets = [...new Set(state.catalog.icons.map((i) => nsOf(i.id)))].filter(pluginOn);
  const setSel = h("select", {}, sets.map((s) => h("option", { value: s }, s)));
  return h("details", { class: "more" },
    h("summary", {}, "Icons for the whole page"),
    h("div", { class: "row wrap" }, setSel,
      h("button", { class: "btn", type: "button", onclick: () => { applySet(setSel.value); done(); } }, "Apply set"),
      h("button", { class: "btn", type: "button", onclick: () => { restoreDefaults(); done(); } }, "Restore default icons")));
}

/** Every app key of the page takes the best icon of `set` that shares more than `button`. */
function applySet(set) {
  let n = 0;
  edit(`Apply icon set ${set}`, () => {
    for (const key of Object.values(pageObj())) {
      if (keyKind(key) !== "app") continue;
      const app = appDef(key.app);
      if (!app) continue;
      const best = iconsForApp(app).find((x) => x.role !== "button" && nsOf(x.icon.id) === set);
      if (!best) continue;
      ensureState(key, 0).icon = best.icon.id;
      n++;
    }
  });
  hint(n ? `Set ${set} applied to ${n} key${n > 1 ? "s" : ""}.` : `No key on this page has an app that fits an icon of ${set}.`);
}

function restoreDefaults() {
  edit("Restore default icons", () => {
    for (const key of Object.values(pageObj())) {
      if (keyKind(key) !== "app" || !key.states?.[0]) continue;
      delete key.states[0].icon;
      delete key.states[0].icon_off;
    }
  });
  hint("All app keys of this page use the default icons of their apps again.");
}

/** Choose an action. Resolves its id, or undefined when closed. */
export function pickAction({ title }) {
  const groups = byPlugin(state.catalog.actions);
  return open({
    title, placeholder: "Search actions",
    sub: "What the key does. You can add several; they run in order.",
    draw: (q, choose) => {
      const out = groups.map((g) => {
        const items = g.items.filter((a) => !q || a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q) || (a.description || "").toLowerCase().includes(q));
        return items.length ? h("div", { class: "group" },
          h("div", { class: "group-title" }, g.title),
          items.map((a) => h("button", { type: "button", class: "item", title: a.id, onclick: () => choose(a.id) },
            h("span", { class: "grow" }, h("span", { class: "name" }, a.name), a.description ? h("span", { class: "meta" }, a.description) : null)))) : null;
      });
      return out.some(Boolean) ? out : h("p", { class: "sub" }, "No action matches.");
    },
  });
}

export { nice as niceIcon, iconUrl };
