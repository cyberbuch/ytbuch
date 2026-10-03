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
