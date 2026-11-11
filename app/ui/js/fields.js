// Form controls built from a package schema field (action params, app config):
// {key, label, type: text|toggle|number|select, placeholder?, default?, values?, multiple?}
import { h } from "./dom.js";
import { selectOptions } from "./catalog.js";

const MONO = /command|combo|keys|topic|payload/;

/**
 * One control for schema field `f`.
 * read(): the current value (undefined when unset).
 * write(v, coalesce): stores v; undefined removes the value.
 * opts.labelled: wrap in a labelled field (app settings); else inline (action rows).
 */
export function schemaField(f, read, write, { labelled = false } = {}) {
  const label = f.label || f.key;
  const cur = read();
  let ctl;
  switch (f.type) {
    case "toggle": {
      const c = h("input", { type: "checkbox", class: "switch", checked: cur === undefined ? !!f.default : !!cur });
      c.onchange = () => write(c.checked ? true : f.default === true ? false : undefined);
      return h("label", { class: "switch-row" }, c, label);
    }
    case "number": {
      ctl = h("input", { type: "number", placeholder: f.placeholder || (labelled ? "" : label), title: label });
      ctl.value = cur ?? "";
      ctl.oninput = () => write(ctl.value === "" ? undefined : Number(ctl.value), true);
      break;
    }
    case "select": {
      if (f.multiple) return multiSelect(f, cur, write, label);
      ctl = h("select", { title: label });
      const fill = (opts) => {
        const list = [...opts];
        // Keep a value the options do not know (a deleted page, "next", ...).
        if (cur !== undefined && cur !== null && !list.some((o) => String(o.value) === String(cur))) list.unshift({ label: String(cur), value: cur });
        if (cur === undefined || cur === null) list.unshift({ label: "Choose…", value: "" });
        ctl.replaceChildren(...list.map((o) => h("option", { value: String(o.value) }, o.label ?? String(o.value))));
        ctl.value = cur === undefined || cur === null ? "" : String(cur);
        ctl._opts = list;
      };
      fill(Array.isArray(f.values) ? f.values : []);
      if (!Array.isArray(f.values)) selectOptions(f).then(fill);
      ctl.onchange = () => {
        const o = (ctl._opts || []).find((x) => String(x.value) === ctl.value);
        write(o && ctl.value !== "" ? o.value : undefined);
      };
      break;
    }
    default: {
      // text, and any type this UI does not know yet
      ctl = h("input", { type: f.type === "password" ? "password" : "text", autocomplete: "off", placeholder: f.placeholder || (labelled ? "" : label), title: label, class: MONO.test(f.key) ? "mono" : "" });
      ctl.value = cur === undefined || cur === null ? "" : typeof cur === "object" ? JSON.stringify(cur) : String(cur);
      ctl.oninput = () => write(ctl.value === "" ? undefined : ctl.value, true);
    }
  }
  return labelled ? h("label", { class: "field" }, h("span", { class: "label" }, label), ctl) : ctl;
}

/** Checkboxes holding an array. Unset (and no default) means all ticked; at least one stays ticked. */
function multiSelect(f, cur, write, label) {
  const values = (Array.isArray(f.values) ? f.values : []).map((o) => o.value);
  const all = () => (Array.isArray(f.default) ? f.default : values);
  return h("div", { class: "field" },
    h("span", { class: "label" }, label),
    h("div", { class: "checks" }, (Array.isArray(f.values) ? f.values : []).map((o) => {
      const set = () => new Set(Array.isArray(cur) && cur.length ? cur : all());
      const c = h("input", { type: "checkbox", checked: set().has(o.value) });
      c.onchange = () => {
        const s = set();
        c.checked ? s.add(o.value) : s.delete(o.value);
        const arr = values.filter((v) => s.has(v));
        if (!arr.length) { c.checked = true; return; }
        cur = arr;
        write(f.default === undefined && arr.length === values.length ? undefined : arr);
      };
      return h("label", { class: "toggle" }, c, o.label ?? String(o.value));
    })));
}
