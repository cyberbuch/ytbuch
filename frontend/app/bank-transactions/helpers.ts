import { Account, Counterpart } from "./types";

export const fmtAmount = (v: string) => {
  const n = Number(v);
  if (Number.isNaN(n)) return v;
  return n.toLocaleString("bg-BG", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
};

export const isPositive = (v: string) => Number(v) >= 0;

export function matchCounterpart(
  name: string,
  list: Counterpart[]
): Counterpart | null {
  const needle = name.trim().toLowerCase();
  if (needle.length < 2) return null;
  const exact = list.find((c) => c.name.toLowerCase() === needle);
  if (exact) return exact;
  return (
    list.find((c) => {
      const n = c.name.toLowerCase();
      if (n.length < 3) return false;
      return n.includes(needle) || needle.includes(n);
    }) ?? null
  );
}

export function filterCounterparts(list: Counterpart[], q: string): Counterpart[] {
  const s = q.trim().toLowerCase();
  const src = !s
    ? list
    : list.filter(
        (c) =>
          c.name.toLowerCase().includes(s) ||
          (c.eik || "").toLowerCase().includes(s) ||
          (c.vat_number || "").toLowerCase().includes(s)
      );
  return src.slice(0, 40);
}

export function filterAccounts(list: Account[], q: string): Account[] {
  const s = q.trim().toLowerCase();
  const src = !s
    ? list
    : list.filter(
        (a) =>
          a.number.toLowerCase().includes(s) || a.name.toLowerCase().includes(s)
      );
  return src.slice(0, 80);
}

export function cpLabel(c: Counterpart): string {
  const eik = (c.eik || "").trim();
  return eik ? `${c.name} (${eik})` : c.name;
}
