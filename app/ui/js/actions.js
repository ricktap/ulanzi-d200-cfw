// Action sections of the inspector (On press, Long press, Extended) and adding an
// action from the library. A key's actions live in `on.<event>` of its state as
// `{action, params}`; parameter fields come from the catalog's schema.
import { h, toast } from "./dom.js";
import { call } from "./api.js";
import { state, edit, hint, render, saveNow, settingsOf } from "./store.js";
import { actionDef, actionDefaults, pluginOn, nsOf } from "./catalog.js";
import { schemaField } from "./fields.js";

/** [slot, title, foldable] */
export const SLOTS = [
  ["press", "On press", false],
  ["long", "Long press", true],
  ["down", "Extended: key down", true],
  ["up", "Extended: key up", true],
];

const opened = new Set(); // foldable sections the user opened

/**
 * The action sections for one holder of `on` (a key state, or status_key).
 * holder(create): the object; with create = true it is made when missing (inside edits).
 */
export function actionSections(holder, { where, slots = SLOTS } = {}) {
  const listOf = (ev, create) => {
    const o = holder(create);
    if (!o) return [];
    if (!o.on) {
      if (!create) return [];
      o.on = {};
    }
    if (!Array.isArray(o.on[ev])) {
      if (!create) return [];
      o.on[ev] = [];
    }
    return o.on[ev];
  };
  const ms = settingsOf(state.config).long_press_ms;
  if (!slots.some(([s]) => s === state.slot)) state.slot = slots[0][0];

  return slots.map(([ev, title, foldable]) => {
    const list = listOf(ev, false);
    const target = state.slot === ev;
    const open = !foldable || list.length > 0 || opened.has(ev) || target;
    const count = list.length ? `${list.length} action${list.length > 1 ? "s" : ""}` : open ? "" : "add";
    const head = h("button", { type: "button", class: "sec-head", onclick: () => {
      if (state.slot === ev && foldable && !list.length) opened.has(ev) ? opened.delete(ev) : opened.add(ev);
      else opened.add(ev);
      state.slot = ev;
      render("inspector");
    } },
      h("b", {}, title),
      ev === "long" ? h("span", { class: "count" }, `${ms} ms`) : null,
      target ? h("span", { class: "tgt" }, "target") : null,
      h("span", { class: "count" }, count));
    const body = open
      ? h("div", { class: "sec-body" },
        list.map((a, i) => actionRow(a, i, list, `${title} · ${where}`)),
        list.length ? null : h("p", { class: "sub" }, target
          ? h("span", {}, "Pick an action under ", h("a", { onclick: () => { state.libTab = "actions"; render("library"); } }, "Actions"), " in the library.")
          : "No actions."))
      : null;
    return h("div", { class: "sec" + (target ? " target" : "") }, head, body);
  });
}

function actionRow(a, i, list, where) {
  const def = actionDef(a.action);
  const off = def && !pluginOn(nsOf(a.action));
  const params = def && !off
    ? (def.params || []).map((f) =>
      schemaField(f, () => a.params?.[f.key], (v, typing) => {
        edit(`Edit action · ${where}`, () => {
          if (v === undefined) {
            if (a.params) delete a.params[f.key];
            if (a.params && !Object.keys(a.params).length) delete a.params;
          } else (a.params ||= {})[f.key] = v;
        }, { redraw: false, coalesce: typing ? `param:${where}:${i}:${f.key}` : null });
      }, { labelled: true }))
    : [];
  return h("div", { class: "act" },
    h("div", { class: "act-main" },
      h("span", { class: "name" }, def ? def.name : a.action),
      h("span", { class: "sub mono" }, a.action),
      !def ? h("span", { class: "sub" }, "Plugin missing. The action is kept as it is.") : null,
      off ? h("span", { class: "sub" }, "Its plugin is switched off under Packages.") : null,
      params),
    h("button", { class: "icon-btn", title: "Remove", type: "button", onclick: () => {
      edit(`Remove action · ${where}`, () => list.splice(i, 1));
      hint("Action removed.");
    } }, "✕"));
}

/**
 * Append action `id` (with its default parameters) to the target section of the
 * selected key. `holder(create)` as above; `where` names the key for undo.
 */
export async function addAction(id, holder, where) {
  const def = actionDef(id);
  const params = await actionDefaults(def);
  const ev = state.slot;
  edit(`Add ${def?.name || id} · ${where}`, () => {
    const o = holder(true);
    o.on ||= {};
    (o.on[ev] ||= []).push(Object.keys(params).length ? { action: id, params } : { action: id });
  });
  const title = SLOTS.find(([s]) => s === ev)?.[1] || ev;
  hint(`${def?.name || id} added to ${title} of ${where}.`);
}

/** Test press: save, then really run the On press actions of the selected key. */
export async function testPress() {
  await saveNow();
  await call("trigger", { key: state.selected, event: "press" });
  toast(`Key ${state.selected} pressed`);
}
