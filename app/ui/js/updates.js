// Updates: the engine asks GitHub (with an ETag, so unchanged answers cost nothing)
// and fires "update" when a newer release exists. Installing restarts the app.
import { $, toast, ask } from "./dom.js";
import { invoke, listen } from "./api.js";

let pending = null;

function show(u) {
  pending = u;
  const b = $("btn-update");
  b.hidden = !u;
  if (u) {
    $("btn-update-label").textContent = `Update to ${u.version}`;
    b.title = u.notes || `D200 Deck ${u.version} is available`;
  }
}

async function install() {
  if (!pending || !(await ask(`Install D200 Deck ${pending.version} and restart?`, null))) return;
  const b = $("btn-update");
  b.disabled = true;
  $("btn-update-label").textContent = "Updating…";
  try {
    await invoke("install_update");
  } catch (e) {
    b.disabled = false;
    $("btn-update-label").textContent = `Update to ${pending.version}`;
    toast(String(e), true);
  }
}

/** Check now; returns a short line for the Application card. */
export async function checkUpdate() {
  try {
    const u = await invoke("check_update");
    if (u) {
      show(u);
      return `· ${u.version} is available`;
    }
    return "· up to date";
  } catch (e) {
    return "· could not check: " + e;
  }
}

export async function wireUpdates() {
  $("btn-update").onclick = install;
  await listen("update", (e) => show(e.payload));
}
