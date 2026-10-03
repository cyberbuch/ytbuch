"use client";

// I18nProvider — език на интерфейса. Български е дефолт; пази се в локалСторидж.
// Четене през юзСинкЕкстърналСтор (хидратационно-безопасно).

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useSyncExternalStore,
  ReactNode,
} from "react";
import { DEFAULT_LANG, Lang, translate } from "../lib/i18n";
import { subscribeStorage, writeStorage } from "../lib/storage";

const LANG_KEY = "***";

interface I18nContextValue {
  lang: Lang;
  setLang: (l: Lang) => void;
  t: (key: string) => string;
}

const I18nContext = createContext<I18nContextValue>({
  lang: DEFAULT_LANG,
  setLang: () => {},
  t: (key) => key,
});

function clientSnapshot(): Lang {
  const stored = window.localStorage.getItem(LANG_KEY);
  return stored === "bg" || stored === "en" ? stored : DEFAULT_LANG;
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const lang = useSyncExternalStore(
    subscribeStorage,
    clientSnapshot,
    () => DEFAULT_LANG
  );

  const setLang = useCallback((l: Lang) => writeStorage(LANG_KEY, l), []);

  const value = useMemo<I18nContextValue>(
    () => ({
      lang,
      setLang,
      t: (key: string) => translate(lang, key),
    }),
    [lang, setLang]
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  return useContext(I18nContext);
}
