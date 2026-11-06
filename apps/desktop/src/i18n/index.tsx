import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  STORAGE_KEY,
  detectLanguage,
  setCurrentLang,
  translate,
  translatePlural,
  type Lang,
  type Params,
  type TranslationKey,
} from "./core";

export { LANGUAGES, type Lang, type Params, type Translate, type TranslationKey } from "./core";

type Context = {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: (key: TranslationKey, params?: Params) => string;
  tn: (key: string, count: number, params?: Params) => string;
  /** BCP 47 tag for Intl/Date formatting. */
  locale: string;
};

const I18nContext = createContext<Context | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(detectLanguage);
  const setLang = useCallback((next: Lang) => {
    setLangState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Not persisted; the choice still applies to this session.
    }
  }, []);
  useEffect(() => {
    document.documentElement.lang = lang === "pt" ? "pt-BR" : "en";
    setCurrentLang(lang);
  }, [lang]);
  // Set before children render so their first backend calls already use it.
  setCurrentLang(lang);
  const value = useMemo<Context>(
    () => ({
      lang,
      setLang,
      t: (key, params) => translate(lang, key, params),
      tn: (key, count, params) => translatePlural(lang, key, count, params),
      locale: lang === "pt" ? "pt-BR" : "en-US",
    }),
    [lang, setLang],
  );
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): Context {
  const context = useContext(I18nContext);
  if (!context) throw new Error("useI18n must be used inside <I18nProvider>");
  return context;
}
