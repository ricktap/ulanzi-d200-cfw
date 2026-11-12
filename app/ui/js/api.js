// Tauri bridge. Every module talks to the Rust side through here.
import { toast } from "./dom.js";

const tauri = window.__TAURI__;

export const invoke = (cmd, args) => tauri.core.invoke(cmd, args);
export const listen = (event, fn) => tauri.event.listen(event, fn);
export const currentWebview = () => tauri.webview?.getCurrentWebview?.() || null;

/** invoke that shows the error as a toast and rethrows. */
export async function call(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    toast(String(e), true);
    throw e;
  }
}
