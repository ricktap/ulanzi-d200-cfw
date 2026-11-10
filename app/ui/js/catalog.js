// What the packages offer (Tauri command `catalog`), plus the icon list with groups.
import { invoke } from "./api.js";
import { state } from "./store.js";
import { toast } from "./dom.js";

const ICON_DIR = "icons/night-signal";
let iconSet = []; // [{name, group, stateful?}] from the Night Signal manifest

export async function loadCatalog() {
  const [cat, icons] = await Promise.all([
    invoke("catalog").catch((e) => {
      toast("Catalog could not be loaded: " + e, true);
      return null;
    }),
    invoke("icon_set").catch(() => null),
  ]);
  if (cat) state.catalog = { ...state.catalog, ...cat };
  iconSet = Array.isArray(icons) ? icons : [];
  if (!iconSet.length) {
    // Fallback outside the app (the CSP blocks this in the app).
    iconSet = (await fetch(`${ICON_DIR}/manifest.json`).then((r) => r.json()).catch(() => ({ icons: [] }))).icons || [];
  }
  if (!iconSet.length) toast("Icon library could not be loaded", true);
}

export const iconList = () => iconSet;

export const appDef = (id) => state.catalog.apps.find((a) => a.id === id);
export const actionDef = (id) => state.catalog.actions.find((a) => a.id === id);
export const iconDef = (id) => state.catalog.icons.find((i) => i.id === withNs(id));
/** Apps a user can place (everything but the plain button). */
export const placeableApps = () => state.catalog.apps.filter((a) => a.id !== "core:button");

/** "volume-on" → "core:volume-on"; ids with a namespace stay. */
export const withNs = (id) => {
  const s = (id || "").trim();
  return s && !s.includes(":") ? `core:${s}` : s;
};

/** URL of an icon's SVG when it is a Night Signal icon (core namespace), else null. */
export function iconUrl(id) {
  const s = (id || "").trim();
  if (!s) return null;
  const name = s.startsWith("core:") ? s.slice(5) : s.includes(":") ? null : s;
  if (!name || !iconSet.some((i) => i.name === name)) return null;
  return `${ICON_DIR}/svg/${name}.svg`;
}
export const isSetIcon = (id) => !!iconUrl(id);

/** Values for a schema with defaults (app config, action params). */
export function schemaDefaults(schema) {
  const out = {};
  for (const f of schema || []) if (f.default !== undefined) out[f.key] = structuredClone(f.default);
  return out;
}

/** Options for a select: fixed `values`, or `{source}` from Rust. → [{label, value}] */
export async function selectOptions(f) {
  if (Array.isArray(f.values)) return f.values;
  if (f.values && f.values.source) {
    const list = await invoke("options", { source: f.values.source, profile: state.view.profile }).catch(() => []);
    return Array.isArray(list) ? list : [];
  }
  return [];
}

/** Params for a new action: defaults, and the first option of every select. */
export async function actionDefaults(def) {
  const params = schemaDefaults(def?.params);
  for (const f of def?.params || []) {
    if (f.type === "select" && !f.multiple && params[f.key] === undefined) {
      const opts = await selectOptions(f);
      if (opts.length) params[f.key] = opts[0].value;
    }
  }
  return params;
}

/** Icons that share a role with the app, richest shared role first (button last). */
export function iconsForApp(app) {
  const roles = app?.roles || ["button"];
  const out = [];
  for (const icon of state.catalog.icons) {
    const i = roles.findIndex((r) => r !== "button" && icon.roles.includes(r));
    const rank = i >= 0 ? i : icon.roles.includes("button") && roles.includes("button") ? roles.length : -1;
    if (rank >= 0) out.push({ icon, rank, role: i >= 0 ? roles[i] : "button" });
  }
  return out.sort((a, b) => a.rank - b.rank || a.icon.id.localeCompare(b.icon.id));
}
