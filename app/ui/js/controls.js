// Inspector controls bound to a field of a config object. Every change is a
// store edit. `read()` gives the object for display (may be undefined);
// `holder()` gives it for writing, created when missing (called inside edits).
import { h } from "./dom.js";
import { edit } from "./store.js";

export function textField(label, read, holder, field, opts = {}) {
  const el = opts.area ? h("textarea", { rows: opts.rows || 2, placeholder: opts.ph || "" }) : h("input", { placeholder: opts.ph || "" });
  el.value = read()?.[field] ?? "";
  el.oninput = () => {
    edit(`${opts.name || field} · ${opts.where || ""}`, () => {
      const o = holder();
      if (el.value === "" && opts.dropEmpty) delete o[field];
      else o[field] = el.value;
    }, { redraw: false, coalesce: `text:${opts.where}:${field}` });
    opts.changed?.(el.value);
  };
  return h("label", { class: "field" }, label ? h("span", { class: "label", html: label }) : null, el);
}
