// Profiles view, the profile automation card, page operations and the
// "use this profile for" app picker.
import { $, h, put, ask, toast, initials } from "./dom.js";
import { call } from "./api.js";
import { state, edit, saveNow, goto, render, profile, allActions } from "./store.js";

const saveThen = async (fn) => {
  await saveNow().catch(() => {});
  fn?.();
};

export function renderAutomation() {
  const p = profile();
  const first = p.apps[0];
  const short = (id) => id.split(".").pop();
  put($("automation"),
    h("h3", {}, "Profile automation"),
    h("div", { class: "auto-row" },
      h("span", { class: "badge" }, first ? initials(short(first)) : "—"),
      h("div", { class: "grow" },
        h("b", {}, first ? p.apps.map(short).join(", ") : state.view.profile === 0 ? "Default profile" : "No app bound"),
        h("span", { class: "sub" }, first
          ? `${p.name} activates automatically`
          : state.view.profile === 0 ? "Used when no other profile matches" : "Only used when you pick it")),
      h("a", { onclick: () => render("pane:profiles") }, "Edit")),
  );
}

export function renderProfiles() {
  const c = state.config;
  put($("profile-cards"), c.profiles.map((p, i) => {
    const apps = h("div", { class: "chips" }, p.apps.map((a, j) =>
      h("span", { class: "chip", title: a }, h("span", {}, a),
        h("button", { title: "Remove", onclick: (e) => {
          e.stopPropagation();
          edit(`Remove ${a} from ${p.name}`, () => p.apps.splice(j, 1));
        } }, "×"))));
    const n = Object.keys(p.pages).length;
    return h("div", { class: "card pcard" + (i === state.view.profile ? " sel" : ""), onclick: () => goto(i, null, false) },
      h("div", { class: "pcard-head" },
        h("span", { class: "badge" }, initials(p.name)),
        h("h3", {}, p.name),
        i === 0 ? h("span", { class: "tag" }, "default") : null,
        i === state.view.profile ? h("span", { class: "tag live" }, "on device") : null),
      h("span", { class: "sub" }, `${n} page${n > 1 ? "s" : ""} · ${p.apps.length ? "used for:" : i === 0 ? "used when nothing else matches" : "no apps bound"}`),
      apps,
      h("div", { class: "row wrap" },
        h("button", { class: "btn small", onclick: (e) => { e.stopPropagation(); openAppPicker(i); } }, "+ Add app"),
        h("button", { class: "btn small", onclick: (e) => { e.stopPropagation(); renameProfile(i); } }, "Rename"),
        i > 0 ? h("button", { class: "btn small", onclick: (e) => { e.stopPropagation(); makeDefault(i); } }, "Make default") : null,
        h("button", { class: "btn small danger-ghost", onclick: (e) => { e.stopPropagation(); deleteProfile(i); } }, "Delete")));
  }));
}

export async function addProfile() {
  const name = await ask("New profile name", "");
  if (!name) return;
  const c = state.config;
  if (c.profiles.some((p) => p.name === name)) return toast("A profile with that name exists", true);
  edit(`New profile ${name}`, () => c.profiles.push({ name, apps: [], pages: { main: {} } }), { save: false });
  await saveThen(() => goto(state.config.profiles.length - 1, "main", false));
}

async function renameProfile(i) {
  const old = state.config.profiles[i].name;
  const name = await ask("Rename profile", old);
  if (!name || name === old) return;
  if (state.config.profiles.some((p) => p.name === name)) return toast("A profile with that name exists", true);
  edit(`Rename profile ${old}`, (c) => {
    c.profiles[i].name = name;
    for (const a of allActions()) if (a.action === "core:profile" && a.params?.profile === old) a.params.profile = name;
  }, { save: false });
  await saveThen();
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
  if (c.profiles.length < 2) return toast("The last profile cannot be deleted", true);
  if (!(await ask(`Delete profile "${c.profiles[i].name}" and all its pages?`, null))) return;
  edit(`Delete profile ${c.profiles[i].name}`, (c) => {
    c.profiles.splice(i, 1);
    state.view.profile = 0;
    state.view.page = Object.keys(c.profiles[0].pages)[0];
  }, { save: false });
  await saveThen(() => goto(0, null, false));
}

async function openAppPicker(i) {
  const list = $("apps-list");
  put(list, h("li", {}, h("span", { class: "sub" }, "Loading…")));
  $("dlg-apps").showModal();
  const add = (id) => {
    const p = state.config.profiles[i];
    if (!p.apps.includes(id)) edit(`Bind ${id} to ${p.name}`, () => p.apps.push(id));
    toast(`Added ${id}`);
  };
  $("apps-add").onclick = () => {
    const v = $("apps-manual").value.trim();
    if (v) add(v);
    $("apps-manual").value = "";
  };
  const apps = await call("running_apps").catch(() => []);
  put(list, apps.map((a) => h("li", { onclick: () => add(a.id) }, h("span", {}, a.name), h("span", { class: "sub" }, a.id))));
  if (!apps.length) put(list, h("li", {}, h("span", { class: "sub" }, "No apps found — type the name below.")));
}

// ------------------------------------------------------------------ pages

export async function addPage() {
  const name = await ask("New page name", "");
  if (!name) return;
  if (profile().pages[name]) return toast("A page with that name exists", true);
  edit(`New page ${name}`, () => {
    profile().pages[name] = { "0": { states: [{ label: "Back", icon: "core:arrow-left", on: { press: [{ action: "core:back" }] } }] } };
  }, { save: false });
  await saveThen(() => goto(state.view.profile, name, false));
}

export async function renamePage() {
  const old = state.view.page;
  const name = await ask("Rename page", old);
  if (!name || name === old) return;
  if (profile().pages[name]) return toast("A page with that name exists", true);
  edit(`Rename page ${old}`, () => {
    const pages = {};
    for (const [n, p] of Object.entries(profile().pages)) pages[n === old ? name : n] = p;
    profile().pages = pages;
    for (const a of allActions()) if (a.action === "core:page" && a.params?.page === old) a.params.page = name;
    state.view.page = name;
  }, { save: false });
  await saveThen(() => goto(state.view.profile, name, false));
}

export async function makeHome() {
  edit(`Make ${state.view.page} home`, () => {
    const pages = { [state.view.page]: profile().pages[state.view.page] };
    for (const [n, p] of Object.entries(profile().pages)) if (n !== state.view.page) pages[n] = p;
    profile().pages = pages;
  }, { save: false });
  await saveThen(() => render("toolbar"));
}

export async function deletePage() {
  if (Object.keys(profile().pages).length < 2) return toast("A profile needs at least one page", true);
  if (!(await ask(`Delete page "${state.view.page}"?`, null))) return;
  edit(`Delete page ${state.view.page}`, () => {
    delete profile().pages[state.view.page];
    state.view.page = Object.keys(profile().pages)[0];
  }, { save: false });
  await saveThen(() => goto(state.view.profile, null, false));
}
