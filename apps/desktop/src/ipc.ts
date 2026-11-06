import { invoke } from "@tauri-apps/api/core";
import { getCurrentLang } from "./i18n/core";

/** Calls a backend command, telling the Rust core which language to answer in. */
export function call<T>(command: string, args: Record<string, unknown> = {}) {
  return invoke<T>(command, { ...args, lang: getCurrentLang() });
}
