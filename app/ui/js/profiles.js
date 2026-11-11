// The profile editor above the device: name, bound programs, pages, and
// adding or deleting profiles and pages.
import { $, h, put, ask, toast } from "./dom.js";
import { call } from "./api.js";
import { state, edit, hint, saveNow, goto, render, profile, allActions } from "./store.js";

const saveThen = async (fn) => {
  await saveNow().catch(() => {});
  fn?.();
};

let running = null; // [{id, name}] while the editor is open

export function renderProfileEditor() {
  const el = $("pf-editor");
  el.hidden = !state.editProfile;
  if (!state.editProfile) return;
  const i = state.view.profile;
  const p = profile();
  const name = h("input", { value: p.name });
  name.onchange = () => renameProfile(i, name.value.trim());

  const bound = p.apps.map((a, j) => h("span", { class: "chip", title: a }, a,
    h("button", { type: "button", title: `Unbind ${a}`, "aria-label": `Unbind ${a} from this profile`, onclick: () => {
      edit(`Unbind ${a}`, () => p.apps.splice(j, 1));
      hint(`${a} is no longer bound to ${p.name}.`);
    } }, "✕")));
  const manual = h("input", { placeholder: "Program name or bundle id" });
  const addManual = () => { const v = manual.value.trim(); if (v) bind(i, v); };
  manual.onkeydown = (e) => { if (e.key === "Enter") { e.preventDefault(); addManual(); } };
  const candidates = (running || [])
    .filter((a) => !p.apps.includes(a.id) && !p.apps.includes(a.name))
    .map((a) => h("button", { type: "button", class: "chip add", title: a.id, onclick: () => bind(i, a.id) }, "+ " + a.name));
  if (!running) call("running_apps").then((list) => { running = list || []; renderProfileEditor(); }).catch(() => { running = []; });

  const names = Object.keys(p.pages);
  const pages = names.map((n, j) => {
    const input = h("input", { value: n, "aria-label": "Page name" });
    input.onchange = () => renamePage(n, input.value.trim());
    return h("div", { class: "page-row" }, input,
      j === 0 ? h("span", { class: "tag" }, "start page") : h("button", { class: "btn small", type: "button", onclick: () => makeHome(n) }, "Set as start page"),
      names.length > 1 ? h("button", { class: "btn small danger", type: "button", onclick: () => deletePage(n) }, "Delete") : null);
  });

  put(el,
    h("label", { class: "field" }, h("span", { class: "label" }, "Name"), name),
    h("div", { class: "field" },
      h("span", { class: "label" }, i === 0 ? "Default profile: used when no bound program is in front" : "Programs that switch to this profile"),
      h("div", { class: "chips" }, bound.length ? bound : h("span", { class: "sub" }, "No program bound.")),
      candidates.length ? h("div", { class: "chips" }, candidates) : null,
      h("div", { class: "row" }, manual, h("button", { class: "btn", type: "button", onclick: addManual }, "Bind"))),
    h("div", { class: "field" }, h("span", { class: "label" }, "Pages"), pages,
      h("div", { class: "row wrap" }, h("button", { class: "btn", type: "button", onclick: addPage }, "+ Page"))),
    h("div", { class: "row wrap" },
      i > 0 ? h("button", { class: "btn", type: "button", onclick: () => makeDefault(i) }, "Make default profile") : null,
      i > 0 ? h("button", { class: "btn danger", type: "button", onclick: () => deleteProfile(i) }, "Delete profile") : null));
}

/** A program belongs to at most one profile. */
function bind(i, app) {
  const p = state.config.profiles[i];
  edit(`Bind ${app} to ${p.name}`, (c) => {
    for (const q of c.profiles) q.apps = q.apps.filter((a) => a !== app);
    p.apps.push(app);
  });
  hint(`${app} is bound to ${p.name} now.`);
}

export function toggleEditor() {
  state.editProfile = !state.editProfile;
  running = null;
  render("device");
}

export async function addProfile() {
  const c = state.config;
  let n = c.profiles.length + 1;
  while (c.profiles.some((p) => p.name === `Profile ${n}`)) n++;
  const name = `Profile ${n}`;
  edit(`New profile ${name}`, () => c.profiles.push({ name, apps: [], pages: { Main: {} } }), { save: false });
  state.editProfile = true;
  hint(`${name} created. Give it a name and bind it to a program.`);
  await saveThen(() => goto(state.config.profiles.length - 1, "Main", false));
}

function renameProfile(i, name) {
  const old = state.config.profiles[i].name;
  if (!name || name === old) return render("device");
  if (state.config.profiles.some((p) => p.name === name)) {
    toast("A profile with that name exists", true);
    return render("device");
  }
  edit(`Rename profile ${old}`, (c) => {
    c.profiles[i].name = name;
    for (const a of allActions()) if (a.action === "core:profile" && a.params?.profile === old) a.params.profile = name;
  });
}

async function makeDefault(i) {
  edit(`Make ${state.config.profiles[i].name} default`, (c) => {
    const p = c.profiles.splice(i, 1)[0];
    c.profiles.unshift(p);
    state.view.profile = 0;
  }, { save: false });
  await saveThen(() => goto(0, null, false));
}

async function deleteProfile(i) {
  const c = state.config;
  if (i === 0) return;
  if (!(await ask(`Delete profile "${c.profiles[i].name}" and all its pages?`, null))) return;
  const name = c.profiles[i].name;
  edit(`Delete profile ${name}`, (c) => {
    c.profiles.splice(i, 1);
    state.view.profile = 0;
    state.view.page = Object.keys(c.profiles[0].pages)[0];
  }, { save: false });
  hint(`Profile ${name} deleted.`);
  await saveThen(() => goto(0, null, false));
}

// ------------------------------------------------------------------ pages

export async function addPage() {
  const pages = profile().pages;
  let n = Object.keys(pages).length + 1;
  while (pages[`Page ${n}`]) n++;
  const name = `Page ${n}`;
  edit(`New page ${name}`, () => {
    pages[name] = { "0": { states: [{ label: "Back", icon: "com.deckling.core:arrow-left", on: { press: [{ action: "core:back" }] } }] } };
  }, { save: false });
  hint(`${name} created, with a Back key.`);
  await saveThen(() => goto(state.view.profile, name, false));
}

async function renamePage(old, name) {
  if (!name || name === old) return render("device");
  if (profile().pages[name]) {
    toast("A page with that name exists", true);
    return render("device");
  }
  edit(`Rename page ${old}`, () => {
    const pages = {};
    for (const [n, p] of Object.entries(profile().pages)) pages[n === old ? name : n] = p;
    profile().pages = pages;
    for (const a of allActions()) if (a.action === "core:page" && a.params?.page === old) a.params.page = name;
    if (state.view.page === old) state.view.page = name;
  }, { save: false });
  await saveThen(() => goto(state.view.profile, state.view.page, false));
}

async function makeHome(name) {
  edit(`Make ${name} the start page`, () => {
    const pages = { [name]: profile().pages[name] };
    for (const [n, p] of Object.entries(profile().pages)) if (n !== name) pages[n] = p;
    profile().pages = pages;
  }, { save: false });
  hint(`${name} is the start page now.`);
  await saveThen(() => render("device"));
}

async function deletePage(name) {
  if (Object.keys(profile().pages).length < 2) return;
  if (!(await ask(`Delete page "${name}"?`, null))) return;
  edit(`Delete page ${name}`, () => {
    delete profile().pages[name];
    if (state.view.page === name) state.view.page = Object.keys(profile().pages)[0];
  }, { save: false });
  hint(`Page ${name} deleted.`);
  await saveThen(() => goto(state.view.profile, state.view.page, false));
}
