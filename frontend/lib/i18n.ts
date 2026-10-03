// lib/i18n.ts — речници за превод. Български е дефолтът.

import { bgA } from "./i18n-bg-a";
import { bgB } from "./i18n-bg-b";
import { enA } from "./i18n-en-a";
import { enB } from "./i18n-en-b";

export type Lang = "bg" | "en";

export const DEFAULT_LANG: Lang = "bg";

type Dict = Record<string, string>;

const bg: Dict = { ...bgA, ...bgB };
const en: Dict = { ...enA, ...enB };

export const dictionaries: Record<Lang, Dict> = { bg, en };

export function translate(lang: Lang, key: string): string {
  const d = dictionaries[lang] ?? bg;
  return d[key] ?? bg[key] ?? key;
}
