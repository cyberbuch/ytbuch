export interface VatExemption {
  code: string;
  name: string;
  name_en: string;
  legal_basis: string;
  legal_basis_en: string;
  ubl_category_code: string;
  ubl_exemption_code: string;
  description: string;
}

export function vatexLabel(e: VatExemption, lang: string): string {
  const name = lang === "en" && e.name_en ? e.name_en : e.name;
  const legal = lang === "en" && e.legal_basis_en ? e.legal_basis_en : e.legal_basis;
  return `${e.code} — ${name} (${legal}) [${e.ubl_category_code}]`;
}

export function vatexDisplay(code: string, items: VatExemption[], lang: string): string {
  if (!code) return "";
  const e = items.find((x) => x.code === code);
  if (!e) return code;
  const name = lang === "en" && e.name_en ? e.name_en : e.name;
  const legal = lang === "en" && e.legal_basis_en ? e.legal_basis_en : e.legal_basis;
  return `${name} — ${legal}`;
}

export function filterVatex(items: VatExemption[], q: string): VatExemption[] {
  const s = q.trim().toLowerCase();
  if (!s) return items;
  return items.filter((e) =>
    `${e.code} ${e.name} ${e.name_en} ${e.legal_basis} ${e.legal_basis_en} ${e.ubl_category_code} ${e.ubl_exemption_code} ${e.description}`
      .toLowerCase()
      .includes(s)
  );
}
