// The action list editor: four event sections (`on.down/up/press/long`) of
// `{action, params}` entries, with fields from the catalog's parameter schema.
import { h, put, toast } from "./dom.js";
import { call } from "./api.js";
import { state, edit, saveNow } from "./store.js";
import { actionDef, actionDefaults } from "./catalog.js";
import { schemaField } from "./fields.js";

const EVENTS = [
  ["down", "Press start", () => "On push"],
  ["up", "Release", () => "On lift"],
  ["press", "Short press", (ms) => `Under ${ms} ms`],
  ["long", "Long press", (ms) => `${ms} ms or longer`],
];
const FIRST_ACTION = "core:command";

/**
 * The four event sections.
 * holder(create): the object that carries `on` (a state, or status_key);
 *   with create = true it is created when missing (only called inside edits).
 * where: a short text for undo labels ("key 3").
 */
export function eventsEditor(holder, { allowState = true, where = "" } = {}) {
  const ms = state.config.settings.long_press_ms;
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
  const actions = state.catalog.actions.filter((a) => allowState || a.id !== "core:state");

  return EVENTS.map(([ev, title, desc]) => {
    const sec = h("div", { class: "ev" });
    const head = h("div", { class: "ev-head" });
    const body = h("div", { class: "ev-body" });
    const draw = () => {
      const list = listOf(ev, false);
      const open = state.openEvents.has(ev);
      sec.classList.toggle("open", open);
      put(head,
        h("div", { class: "grow" }, h("b", {}, title), h("span", {}, desc(ms))),
        list.length && !open ? h("span", { class: "count" }, `${list.length} action${list.length > 1 ? "s" : ""}`) : null,
        h("span", { class: "chev" }, open ? "⌃" : list.length ? "⌄" : "+"),
      );
      body.hidden = !open;
      put(body,
        ...list.map((a, i) => actionRow(a, i, list, actions, draw, `${title} · ${where}`)),
        h("div", { class: "ev-foot" },
          h("button", { class: "add-link", type: "button", onclick: async () => {
            const params = await actionDefaults(actionDef(FIRST_ACTION));
            edit(`Add action · ${where}`, () => listOf(ev, true).push(newAction(FIRST_ACTION, params)), { redraw: false });
            draw();
          } }, "+  Add action"),
          list.length ? h("button", { class: "btn small", type: "button", title: "Run these actions now", onclick: () => testEvent(ev) }, "▶  Test") : null),
      );
    };
    head.onclick = () => {
      state.openEvents.has(ev) ? state.openEvents.delete(ev) : state.openEvents.add(ev);
      draw();
    };
    sec.append(head, body);
    draw();
    return sec;
  });
}

const newAction = (id, params) => (Object.keys(params).length ? { action: id, params } : { action: id });

function actionRow(a, i, list, actions, redraw, where) {
  const def = actionDef(a.action);
  const typeSel = h("select", {},
    def ? null : h("option", { value: a.action, disabled: true }, `Plugin missing: ${a.action}`),
    actions.map((d) => h("option", { value: d.id }, d.name)));
  typeSel.value = a.action;
  typeSel.onchange = async () => {
    const id = typeSel.value;
    const params = await actionDefaults(actionDef(id));
    edit(`Change action · ${where}`, () => {
      const old = list[i];
      const next = { ...old, action: id, params };
      if (!Object.keys(params).length) delete next.params;
      list[i] = next;
    }, { redraw: false });
    redraw();
  };
  const params = (def?.params || []).map((f) =>
    schemaField(f, () => a.params?.[f.key], (v, typing) => {
      edit(`Edit action · ${where}`, () => {
        if (v === undefined) {
          if (a.params) delete a.params[f.key];
          if (a.params && !Object.keys(a.params).length) delete a.params;
        } else {
          (a.params ||= {})[f.key] = v;
        }
      }, { redraw: false, coalesce: typing ? `param:${where}:${i}:${f.key}` : null });
    }));
  return h("div", { class: "act" },
    h("div", { class: "act-main" }, typeSel, params,
      def ? null : h("span", { class: "sub" }, "Kept as it is.")),
    h("button", { class: "icon-btn", title: "Remove", type: "button", onclick: () => {
      edit(`Remove action · ${where}`, () => list.splice(i, 1), { redraw: false });
      redraw();
    } }, "✕"));
}

export function holdThreshold() {
  const ms = state.config.settings.long_press_ms;
  const opts = [250, 300, 400, 500, 600, 800, 1000, 1500];
  if (!opts.includes(ms)) opts.push(ms);
  const sel = h("select", {}, opts.sort((a, b) => a - b).map((v) => h("option", { value: v }, `${v} ms`)));
  sel.value = String(ms);
  sel.onchange = () => edit("Hold threshold", (c) => { c.settings.long_press_ms = +sel.value; });
  return h("div", { class: "hold" },
    h("div", { class: "row" }, h("b", {}, "Hold threshold"), sel),
    h("p", { class: "sub" }, "Long press suppresses short press. Applies to every key."),
    h("span", { class: "saved-line" }, "✓ Saved"));
}

async function testEvent(ev) {
  await saveNow();
  await call("trigger", { key: state.selected, event: ev });
  toast("Ran " + ev);
}
