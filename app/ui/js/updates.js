// Updates: the engine asks GitHub (with an ETag, so unchanged answers cost nothing)
// and fires "update" when a newer release exists. Installing restarts the app.
import { $, toast, ask } from "./dom.js";
import { invoke, listen } from "./api.js";

let pending = null;

const IDLE = "Check for updates";

/** The button in the top bar: "Check for updates", or "Update to x" once one is known. */
function show(u) {
  pending = u;
  const b = $("btn-update");
  b.disabled = false;
  b.classList.toggle("primary", !!u);
  $("btn-update-label").textContent = u ? `Update to ${u.version}` : IDLE;
  b.title = u ? u.notes || `D200 Deck ${u.version} is available` : "Look for a newer version of D200 Deck";
}

/** No update known: ask now and say what came of it on the button. */
async function check() {
  const b = $("btn-update");
  const label = $("btn-update-label");
  b.disabled = true;
  label.textContent = "Checking…";
  try {
    const u = await invoke("check_update");
    if (u) return show(u);
    const version = await invoke("app_version").catch(() => "");
    label.textContent = "Up to date";
    b.title = version ? `D200 Deck ${version} is the newest version` : "This is the newest version";
    toast(version ? `D200 Deck ${version} is up to date` : "D200 Deck is up to date");
  } catch (e) {
    label.textContent = "Could not check";
    toast("Could not check for updates: " + e, true);
  }
  setTimeout(() => { if (!pending) show(null); }, 4000);
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
  $("btn-update").onclick = () => (pending ? install() : check());
  await listen("update", (e) => show(e.payload));
}
