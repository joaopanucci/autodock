import { en } from "./en.ts";
import { pt } from "./pt.ts";

export type Lang = "pt" | "en";
export type TranslationKey = keyof typeof pt;
export type Params = Record<string, string | number>;
export type Translate = (key: TranslationKey, params?: Params) => string;

export const LANGUAGES: { id: Lang; label: string }[] = [
  { id: "en", label: "English" },
  { id: "pt", label: "Português" },
];

const dictionaries: Record<Lang, Record<TranslationKey, string>> = { pt, en };
export const STORAGE_KEY = "autodock.language";

/** Replaces `{name}` placeholders. Unknown placeholders are left untouched. */
export function translate(lang: Lang, key: TranslationKey, params?: Params) {
  const template = dictionaries[lang][key] ?? dictionaries.en[key] ?? key;
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

/** Picks `<key>.one` for exactly one item and `<key>.other` otherwise. */
export function translatePlural(
  lang: Lang,
  key: string,
  count: number,
  params?: Params,
) {
  const full = `${key}.${count === 1 ? "one" : "other"}` as TranslationKey;
  return translate(lang, full, { count, ...params });
}

/** Saved choice first, then the system language, English as the fallback. */
export function detectLanguage(): Lang {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === "pt" || saved === "en") return saved;
  } catch {
    // Storage can be unavailable (private mode, blocked data): use the system language.
  }
  const system = (typeof navigator !== "undefined" && navigator.language) || "";
  return system.toLowerCase().startsWith("pt") ? "pt" : "en";
}


let currentLang: Lang = "en";
/** Kept in sync by the provider so non-React code (IPC) can read the language. */
export function setCurrentLang(lang: Lang) {
  currentLang = lang;
}
export function getCurrentLang(): Lang {
  return currentLang;
}
