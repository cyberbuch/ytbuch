"use client";

// Providers — композира тема + език + автентикация около приложението.

import { ReactNode } from "react";
import { ThemeProvider } from "./ThemeProvider";
import { I18nProvider } from "./I18nProvider";
import { AuthProvider } from "./AuthProvider";

export function Providers({ children }: { children: ReactNode }) {
  return (
    <ThemeProvider>
      <I18nProvider>
        <AuthProvider>{children}</AuthProvider>
      </I18nProvider>
    </ThemeProvider>
  );
}
