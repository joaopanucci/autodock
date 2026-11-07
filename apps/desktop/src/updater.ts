import { Channel } from "@tauri-apps/api/core";
import { call } from "./ipc";
import type { DownloadEvent, UpdateCheck, UpdatePrefs } from "./types";

/** When the app opens: only asks if the user left the check on and the last one is over a day old. */
export const checkAutomatically = () => call<UpdateCheck>("update_auto_check");

/** The user pressed "Check now". */
export const checkNow = () => call<UpdateCheck>("update_check_now");

export const getPrefs = () => call<UpdatePrefs>("update_get_prefs");

export const setCheckOnStart = (enabled: boolean) =>
  call<UpdatePrefs>("update_set_check_on_start", { enabled });

export const skipVersion = (version: string) =>
  call<UpdatePrefs>("update_skip_version", { version });

export const restart = () => call<void>("update_restart");

/** Whole percent (0-100) of a download, or null while the size is not known. */
export function percentOf(done: number, total: number | null): number | null {
  if (!total || total <= 0) return null;
  return Math.max(0, Math.min(100, Math.floor((done / total) * 100)));
}

/**
 * Downloads, verifies and installs the update found by the last check.
 * `onProgress` gets the bytes downloaded so far and the total when it is known.
 * On Windows the app exits when the installer starts, so this may never resolve.
 */
export async function installUpdate(onProgress: (done: number, total: number | null) => void) {
  let done = 0;
  let total: number | null = null;
  const onEvent = new Channel<DownloadEvent>();
  onEvent.onmessage = (message) => {
    if (message.event === "started") {
      total = message.contentLength;
      onProgress(0, total);
    } else if (message.event === "progress") {
      done += message.chunkLength;
      onProgress(done, total);
    }
  };
  await call<void>("update_install", { onEvent });
}
