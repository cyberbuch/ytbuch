// Display helpers for chronobaga policy: storage is ISO/financial;
// invoices and VAT registers print Bulgarian dd.mm.yyyy.

export function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

/** Normalize stored ISO/financial dates to `YYYY-MM-DD` for `<input type="date">`. */
export function toDateInput(raw: string | undefined | null): string {
  if (!raw) return "";
  const s = String(raw).trim();
  if (!s) return "";
  const iso = s.match(/^(\d{4})-(\d{2})-(\d{2})/);
  if (iso) return `${iso[1]}-${iso[2]}-${iso[3]}`;
  const compact = s.match(/^(\d{4})(\d{2})(\d{2})$/);
  if (compact) return `${compact[1]}-${compact[2]}-${compact[3]}`;
  const bg = s.match(/^(\d{1,2})\.(\d{1,2})\.(\d{4})$/);
  if (bg) {
    return `${bg[3]}-${bg[2].padStart(2, "0")}-${bg[1].padStart(2, "0")}`;
  }
  return s.slice(0, 10);
}

export function formatBgDate(raw: string | undefined | null): string {
  if (!raw) return "";
  const s = String(raw).trim();
  if (!s) return "";
  const iso = s.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (iso) return `${iso[3]}.${iso[2]}.${iso[1]}`;
  const fin = s.match(/^(\d{4})(\d{2})(\d{2})$/);
  if (fin) return `${fin[3]}.${fin[2]}.${fin[1]}`;
  const bg = s.match(/^(\d{1,2})\.(\d{1,2})\.(\d{4})$/);
  if (bg) {
    const d = bg[1].padStart(2, "0");
    const m = bg[2].padStart(2, "0");
    return `${d}.${m}.${bg[3]}`;
  }
  return s;
}

export function formatBgPeriod(raw: string | undefined | null): string {
  if (!raw) return "";
  const s = String(raw).trim();
  const iso = s.match(/^(\d{4})-(\d{2})$/);
  if (iso) return `${iso[2]}.${iso[1]}`;
  const fin = s.match(/^(\d{4})(\d{2})$/);
  if (fin) return `${fin[2]}.${fin[1]}`;
  return s;
}
