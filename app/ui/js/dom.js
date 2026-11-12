// DOM helpers shared by every module.

export const $ = (id) => document.getElementById(id);

/** Element builder: h("div", {class, onclick, ...}, ...children). */
export function h(tag, attrs = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === "class") el.className = v;
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "html") el.innerHTML = v;
    else if (k in el && k !== "list") el[k] = v;
    else el.setAttribute(k, v);
  }
  for (const kid of kids.flat(Infinity)) if (kid !== null && kid !== undefined && kid !== false) el.append(kid);
  return el;
}

/** replaceChildren that skips null/false (the native one prints "null"). */
export const put = (el, ...kids) => el.replaceChildren(...kids.flat(Infinity).filter((k) => k !== null && k !== undefined && k !== false));

export function toast(msg, isErr = false) {
  const t = $("toast");
  t.textContent = msg;
  t.className = "toast" + (isErr ? " err" : "");
  t.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (t.hidden = true), isErr ? 7000 : 2500);
}

/** Small modal: asks for text, or yes/no when value === null. */
export function ask(title, value) {
  return new Promise((resolve) => {
    const d = $("dlg-prompt");
    const input = $("prompt-input");
    $("prompt-title").textContent = title;
    input.hidden = value === null;
    input.value = value || "";
    d.returnValue = "";
    d.onclose = () => resolve(d.returnValue !== "ok" ? null : value === null ? true : input.value.trim());
    input.onkeydown = (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        d.close("ok");
      }
    };
    d.showModal();
    if (value !== null) input.select();
  });
}

export const initials = (name) => {
  const w = (name || "").replace(/[^A-Za-z0-9 ]/g, " ").split(/\s+/).filter(Boolean);
  if (!w.length) return "•";
  if (w.length === 1) {
    const caps = w[0].match(/[A-Z]/g);
    return (caps && caps.length > 1 ? caps.slice(0, 2).join("") : w[0].slice(0, 2)).toUpperCase();
  }
  return (w[0][0] + w[1][0]).toUpperCase();
};

/** A labelled field wrapper. */
export const field = (label, ...kids) => h("label", { class: "field" }, label ? h("span", { class: "label", html: label }) : null, ...kids);
