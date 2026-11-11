// Signals view: every signal with its current value; colours get a swatch.
// Asks the engine once a second while the view is open.
import { $, h, put } from "./dom.js";
import { invoke } from "./api.js";
import { state } from "./store.js";

const isColor = (v) => typeof v === "string" && /^(#[0-9a-f]{3,8}|rgba?\(|hsla?\()/i.test(v.trim());
const show = (v) => (v === null || v === undefined ? "—" : typeof v === "object" ? JSON.stringify(v) : String(v));
let timer = null;

async function draw() {
  if (state.pane !== "signals" || document.visibilityState !== "visible") return;
  const list = await invoke("signals").catch(() => []);
  const q = $("sig-filter").value.trim().toLowerCase();
  const rows = list.filter((s) => !q || s.name.toLowerCase().includes(q));
  put($("sig-list"), rows.length
    ? rows.map((s) => h("div", { class: "sig" },
      h("span", { class: "n" }, s.name),
      h("span", { class: "v" }, isColor(s.value) ? h("span", { class: "sw", style: `background:${s.value}` }) : null, show(s.value)),
      h("span", { class: "t" }, s.type || "")))
    : h("div", { class: "sig" }, h("span", { class: "sub" }, q ? "No signal matches." : "No signal has a value yet.")));
}

export function startSignals() {
  stopSignals();
  draw();
  timer = setInterval(draw, 1000);
}
export function stopSignals() {
  clearInterval(timer);
  timer = null;
}
export const wireSignals = () => ($("sig-filter").oninput = draw);
