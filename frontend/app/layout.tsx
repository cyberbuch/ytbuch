// layout.tsx — коренен лейаут. Сървърен компонент; темата се задава от
// инлайн скрипт преди хидратация (без проблясък), провайдерите са клиентски.

import type { Metadata } from "next";
import "./globals.css";
import "./globals-pages.css";
import { Providers } from "../components/Providers";

export const metadata: Metadata = {
  title: "БагаСчетоводство",
  description: "Счетоводна програма",
};

// Задава тема/език в <html> преди Реакт да хидратира.
const themeInitScript = `
(function () {
  try {
    var t = localStorage.getItem("bagabuch.theme");
    if (t !== "dark" && t !== "light") {
      t = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }
    document.documentElement.dataset.theme = t;
    var l = localStorage.getItem("bagabuch.lang");
    if (l === "bg" || l === "en") document.documentElement.lang = l;
    else document.documentElement.lang = "bg";
  } catch (e) {}
})();
`;

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="bg" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeInitScript }} />
      </head>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
