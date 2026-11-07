// Theme preference: pure helpers (no React, no DOM at import time) so the unit
// tests can load this file directly with Node's type stripping.

export type ThemeMode = "system" | "light" | "dark";
export type Theme = "light" | "dark";

export const THEME_KEY = "autodock.theme";
export const THEME_MODES: ThemeMode[] = ["light", "dark", "system"];

export function normalizeMode(value: unknown): ThemeMode {
  return value === "light" || value === "dark" ? value : "system";
}

export function resolveTheme(mode: ThemeMode, systemPrefersDark: boolean): Theme {
  if (mode === "system") return systemPrefersDark ? "dark" : "light";
  return mode;
}

export function loadMode(): ThemeMode {
  try {
    return normalizeMode(window.localStorage.getItem(THEME_KEY));
  } catch {
    return "system";
  }
}

export function saveMode(mode: ThemeMode) {
  try {
    window.localStorage.setItem(THEME_KEY, mode);
  } catch {
    // Storage can be unavailable: the choice then lasts only for this session.
  }
}

const query = () =>
  typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null;

/** Writes the effective theme to the root element so CSS can switch its tokens. */
export function applyMode(mode: ThemeMode) {
  document.documentElement.dataset.theme = resolveTheme(mode, query()?.matches ?? false);
}

/** Applies the saved choice and keeps "system" in sync with the operating system. */
export function initTheme() {
  applyMode(currentMode());
  query()?.addEventListener?.("change", () => applyMode(currentMode()));
}

// ───────── tiny store so every switch on screen shares one choice ─────────
let mode: ThemeMode = "system";
let loaded = false;
const listeners = new Set<() => void>();

export function currentMode(): ThemeMode {
  if (!loaded) {
    mode = loadMode();
    loaded = true;
  }
  return mode;
}

export function setMode(next: ThemeMode) {
  mode = next;
  loaded = true;
  saveMode(next);
  applyMode(next);
  listeners.forEach((listener) => listener());
}

export function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
