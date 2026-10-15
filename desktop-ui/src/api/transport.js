// Single entry point for every backend call made by main.js.
//
// Inside the Tauri desktop app this forwards to the Rust process exactly as
// before. In a browser it dispatches to webCommands, which talk to the HTTP
// backend instead. main.js imports invoke/convertFileSrc/dialogs from here,
// so none of its call sites need to know which build they run in.

import { invoke as tauriInvoke, convertFileSrc as tauriConvertFileSrc } from "@tauri-apps/api/core";
import { open as tauriOpen, save as tauriSave } from "@tauri-apps/plugin-dialog";
import { webCommands, DESKTOP_ONLY } from "./web-commands.js";
import { webOpenDialog, webSaveDialog } from "./web-files.js";

/** True inside the Tauri webview, false in an ordinary browser tab. */
export const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export { DESKTOP_ONLY };

/**
 * Same contract as Tauri's invoke: resolves with the command's result, rejects
 * with a plain string. main.js matches on error text in several places, so a
 * web failure must never surface as an Error object where Tauri gives a string.
 */
export async function invoke(cmd, args = {}) {
  if (IS_TAURI) return tauriInvoke(cmd, args);
  const handler = webCommands[cmd];
  if (!handler) throw `"${cmd}" ist in der Web-Version nicht verfügbar.`;
  try {
    return await handler(args);
  } catch (err) {
    throw typeof err === "string" ? err : (err?.message || String(err));
  }
}

export function convertFileSrc(path, protocol) {
  return IS_TAURI ? tauriConvertFileSrc(path, protocol) : path;
}

export function openDialog(options) {
  return IS_TAURI ? tauriOpen(options) : webOpenDialog(options);
}

export function saveDialog(options) {
  return IS_TAURI ? tauriSave(options) : webSaveDialog(options);
}
