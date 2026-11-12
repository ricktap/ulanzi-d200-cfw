// Putting apps on keys, clearing and swapping keys.
import { state, edit, hint, profile, ensureKey } from "./store.js";
import { appDef, schemaDefaults } from "./catalog.js";

/** Plain text from the package; never HTML, since packages may come from anyone. */
export const appHelp = (app) => (app ? app.description || "" : "");

/**
 * Put app `id` on key `k`: the app's settings get their defaults, its default icon
 * applies (a chosen icon goes) and its preset actions go onto state 0. Label and
 * actions the user added stay; other key fields stay.
 */
export function placeApp(id, k = state.selected) {
  const app = appDef(id);
  const name = app?.name || id;
  edit(`${name} on key ${k}`, () => {
    const key = ensureKey(k);
    const wasApp = !!key.app;
    const old = wasApp ? appDef(key.app) : null;
    key.app = id;
    const config = schemaDefaults(app?.config);
    if (Object.keys(config).length) key.config = config;
    else delete key.config;
    if (key.states?.length) {
      delete key.states[0].icon;
      delete key.states[0].image;
      if (!wasApp) key.states = [key.states[0]]; // a button's other states mean nothing on an app key
    }
    applyPresets(key, old, app);
    state.selState = 0;
    state.slot = "press";
  });
  hint(`${name} is on key ${k} now, with the default icon of its plugin.`);
}

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/**
 * The app's preset actions (`on`) go onto state 0. Presets of the app it replaces
 * are taken off first; actions the user added stay.
 */
function applyPresets(key, oldApp, newApp) {
  const st = key.states?.[0];
  if (st?.on && oldApp?.on) {
    for (const [ev, list] of Object.entries(oldApp.on)) {
      if (!Array.isArray(st.on[ev])) continue;
      st.on[ev] = st.on[ev].filter((a) => !list.some((p) => same(p, a)));
      if (!st.on[ev].length) delete st.on[ev];
    }
    if (!Object.keys(st.on).length) delete st.on;
  }
  const presets = Object.entries(newApp?.on || {}).filter(([, list]) => Array.isArray(list) && list.length);
  if (!presets.length) return;
  key.states ||= [{}];
  const s0 = key.states[0];
  s0.on ||= {};
  for (const [ev, list] of presets) s0.on[ev] = [...(s0.on[ev] || []), ...structuredClone(list)];
}

export function clearKey(k = state.selected) {
  edit(`Clear key ${k}`, () => {
    delete profile().pages[state.view.page][String(k)];
    state.selState = 0;
  });
  hint(`Key ${k} is empty.`);
}

/** Swap two keys of the current page (drag one onto the other). */
export function swapKeys(a, b) {
  if (a === b) return;
  edit(`Swap keys ${a} and ${b}`, () => {
    const page = profile().pages[state.view.page];
    const ka = page[String(a)];
    const kb = page[String(b)];
    delete page[String(a)];
    delete page[String(b)];
    if (kb) page[String(a)] = kb;
    if (ka) page[String(b)] = ka;
  });
  state.selected = b;
  hint(`Keys ${a} and ${b} swapped.`);
}
