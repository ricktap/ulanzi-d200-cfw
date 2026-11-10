// The Assets view: the whole icon library; a click puts the icon on the selected key.
import { $, toast } from "./dom.js";
import { state, edit, keyObj, ensureKey, ensureState, keyKind, editableKey } from "./store.js";
import { appDef, iconsForApp } from "./catalog.js";
import { iconPicker } from "./icons.js";

const target = () => (editableKey() ? `Click an icon to put it on key ${state.selected}` : "Select one of keys 0–12 first");

export function renderAssets() {
  $("assets-target").textContent = target();
  const lib = $("assets-lib");
  if (lib.firstChild) return;
  lib.append(iconPicker((id) => {
    if (!editableKey()) return toast("Select one of keys 0–12 first", true);
    const k = state.selected;
    const key = keyObj(k);
    if (keyKind(key) === "app") {
      // An app key takes icons that share a role with the app; it is always state 0.
      const app = appDef(key.app);
      if (!app) return toast("The app of this key is missing", true);
      if (!iconsForApp(app).some((x) => x.icon.id === id)) return toast("This icon does not fit the app", true);
      edit(`Icon · key ${k}`, () => { ensureState(ensureKey(k), 0).icon = id; });
    } else {
      edit(`Icon · key ${k}`, () => { ensureState(ensureKey(k), state.selState).icon = id; });
    }
    toast(`Icon "${id}" on key ${k}`);
  }, true));
}
