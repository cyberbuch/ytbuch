export interface UnitOfMeasure {
  code: string;
  name: string;
  name_en: string;
  symbol: string;
  category: string;
  source: string;
}

export function unitLabel(code: string, units: UnitOfMeasure[]): string {
  if (!code) return "";
  const u = units.find((x) => x.code === code);
  if (!u) return code;
  return u.symbol ? `${u.symbol} (${u.code})` : u.code;
}

export function unitOptionLabel(u: UnitOfMeasure): string {
  const src =
    u.source === "BOTH" ? "UBL+SAF-T" : u.source === "SAFT" ? "SAF-T" : "UBL";
  return `${u.code} — ${u.symbol} — ${u.name} [${src}]`;
}

export function filterUnits(units: UnitOfMeasure[], q: string): UnitOfMeasure[] {
  const s = q.trim().toLowerCase();
  if (!s) return units;
  return units.filter((u) => {
    const hay = `${u.code} ${u.name} ${u.name_en} ${u.symbol} ${u.category} ${u.source}`.toLowerCase();
    return hay.includes(s);
  });
}

const UNIT_SYMBOL_MAP: Record<string, string> = {
  C62: "бр.",
  EA: "бр.",
  PCE: "бр.",
  H87: "бр.",
  E48: "бр.",
  NAR: "бр.",
  SET: "к-т",
  PR: "чифт",
  DZN: "дуз.",
  GRO: "грос",
  MIL: "хил.",
  CEN: "стотици",
  KGM: "кг",
  GRM: "г",
  MGM: "мг",
  TNE: "т",
  DTN: "дт",
  LBR: "lb",
  ONZ: "oz",
  CTM: "ct",
  MTR: "м",
  CMT: "см",
  MMT: "мм",
  KMT: "км",
  DMT: "дм",
  INH: "in",
  FOT: "ft",
  YRD: "yd",
  NMI: "nmi",
  MTK: "м²",
  CMK: "см²",
  MMK: "мм²",
  LTR: "л",
  LT: "л",
  MLT: "мл",
  HLT: "хл",
  HUR: "час",
  DAY: "ден",
  MON: "месец",
  WEE: "седм.",
  PK: "пакет",
};

export function unitSymbol(code: string): string {
  if (!code) return "";
  const c = code.trim();
  if (UNIT_SYMBOL_MAP[c]) return UNIT_SYMBOL_MAP[c];
  return c;
}
