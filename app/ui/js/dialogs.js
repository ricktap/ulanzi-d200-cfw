// Small dialogs: test notification and import / export.
import { $, toast } from "./dom.js";
import { invoke, call } from "./api.js";
import { state, load } from "./store.js";

export function wireDialogs() {
  const n = () => ({
    title: $("n-title").value || null, text: $("n-text").value, icon: $("n-icon").value || null, color: $("n-color").value,
    style: $("n-style").value, duration: parseFloat($("n-dur").value) || 8, priority: $("n-prio").value,
  });
  const upd = () => {
    $("n-json").textContent = JSON.stringify(n());
    $("n-topic").textContent = (state.config.settings.mqtt.base_topic || "d200") + "/notify";
  };
  for (const id of ["n-title", "n-text", "n-icon", "n-color", "n-style", "n-dur", "n-prio"]) $(id).addEventListener("input", upd);
  $("btn-notify").onclick = () => { upd(); $("dlg-notify").showModal(); };
  $("n-send").onclick = () => call("notify", { notification: n() });

  $("btn-json").onclick = () => {
    $("json-text").value = JSON.stringify(state.config, null, 2);
    $("json-error").textContent = "";
    $("dlg-json").showModal();
  };
  $("json-copy").onclick = () => navigator.clipboard.writeText($("json-text").value).then(() => toast("Copied"));
  $("json-import").onclick = async () => {
    try {
      // Rust parses (and migrates) the JSON; the previous config stays on the undo stack.
      load(await invoke("import_config", { json: $("json-text").value }), "Import");
      $("dlg-json").close();
      toast("Config imported");
    } catch (e) {
      $("json-error").textContent = String(e);
    }
  };
}
