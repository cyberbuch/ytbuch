"use client";

// ThemeProvider — светла/тъмна тема през data-theme на <html>.
// Стойността се пази в localStorage; четене през юзСинкЕкстърналСтор
// (хидратационно-безопасно, без сетСтейт в ефект).

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useSyncExternalStore,
  ReactNode,
} from "react";
import { subscribeStorage, writeStorage } from "../lib/storage";

export type Theme = "light" | "dark";

const THEME_KEY = "***";

interface ThemeContextValue {
  theme: Theme;
  toggleTheme: () => void;
  setTheme: (t: Theme) => void;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: "light",
  toggleTheme: () => {},
  setTheme: () => {},
});

// Клиентски снапшот: съхранена тема, иначе системната предпочитана.
function clientSnapshot(): Theme {
  const stored = window.localStorage.getItem(THEME_KEY);
  if (stored === "dark" || stored === "light") return stored;
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const theme = useSyncExternalStore<Theme>(
    subscribeStorage,
    clientSnapshot,
    () => "light"
  );

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const setTheme = useCallback((t: Theme) => writeStorage(THEME_KEY, t), []);
  const toggleTheme = useCallback(
    () => writeStorage(THEME_KEY, theme === "dark" ? "light" : "dark"),
    [theme]
  );

  return (
    <ThemeContext.Provider value={{ theme, toggleTheme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  return useContext(ThemeContext);
}
