// Споделени типове и изчисления за фактури.

// Видове данъчни документи — кодове по ППДДС. Проформата не е данъчен документ.
export const DOC_TYPES_COMMON = [
  "01",
  "02",
  "03",
  "07",
  "09",
  "11",
  "12",
  "13",
] as const;
export const DOC_TYPES_OUT = [
  ...DOC_TYPES_COMMON,
  "04",
  "81",
  "82",
  "23",
  "29",
  "50",
  "83",
  "84",
  "85",
  "91",
  "93",
  "94",
  "95",
] as const;
export const DOC_TYPES_IN = [
  ...DOC_TYPES_COMMON,
  "05",
  "23",
  "91",
  "92",
  "93",
  "94",
] as const;
export const DOC_TYPE_PROFORMA = "proforma";

export function docTypesFor(direction: string): string[] {
  const base = direction === "in" ? [...DOC_TYPES_IN] : [...DOC_TYPES_OUT];
  return [DOC_TYPE_PROFORMA, ...base];
}

export function docTypeRequiresOriginal(t: string): boolean {
  return t === "02" || t === "03" || t === "12" || t === "13" || t === "23";
}

export function docTypeIsCredit(t: string): boolean {
  return t === "03" || t === "13" || t === "23";
}

export function docTypeIsProforma(t: string): boolean {
  return t === DOC_TYPE_PROFORMA;
}

export const UNITS = ["C62", "H87", "KGM", "LTR", "MTR", "MTK", "MTQ", "HUR", "DAY", "SET", "PK", "E48"];
export const VAT_RATES = ["20", "9", "0"];
export const PAY_METHODS = ["банков превод", "в брой", "карта", "компенсация", "друго"];
export const CURRENCIES = ["EUR", "USD", "GBP", "CHF", "BGN", "RON", "PLN", "TRY", "JPY", "CNY"];

export interface InvoiceLine {
  id?: number;
  product_id?: number;
  code: string;
  description: string;
  quantity: string;
  unit: string;
  unit_price: string;
  vat_rate: string;
  net_amount: string;
  vat_amount: string;
  total_amount: string;
  seller_code?: string;
  keep_amounts?: boolean;
}

export interface InvoiceParty {
  id?: number;
  name?: string;
  eik?: string;
  vat_number?: string;
  address?: string;
  city?: string;
  mol?: string;
  iban?: string;
  bic?: string;
  contact_person?: string;
}

export interface Invoice {
  id: number;
  direction: string;
  document_type: string;
  number: string;
  issue_date: string;
  due_date?: string;
  tax_event_date?: string;
  accounting_month?: string;
  counterpart_id: number;
  counterpart_name?: string;
  currency: string;
  currency_rate?: string;
  net_amount: string;
  vat_amount: string;
  total_amount: string;
  status: string;
  payment_method?: string;
  notes?: string;
  discount_percent?: string;
  discount_amount?: string;
  vat_exemption_reason?: string;
  original_invoice_id?: number;
  journal_entry_id?: number;
  location_id?: number;
  lines?: InvoiceLine[];
  counterpart?: InvoiceParty;
  company?: InvoiceParty;
}

export function emptyLine(): InvoiceLine {
  return {
    code: "",
    description: "",
    quantity: "1",
    unit: "C62",
    unit_price: "0",
    vat_rate: "20",
    net_amount: "0.00",
    vat_amount: "0.00",
    total_amount: "0.00",
  };
}

export { todayIso, formatBgDate, toDateInput } from "./dates";

export function normVatRate(s: string | undefined | null): string {
  const n = num(String(s ?? ""));
  const asStr = String(n);
  if ((VAT_RATES as readonly string[]).includes(asStr)) return asStr;
  return s && String(s).trim() ? String(s) : "20";
}

export function round2(n: number): string {
  return (Math.round(n * 100) / 100).toFixed(2);
}

export function num(s: string): number {
  return parseFloat(String(s).replace(",", ".")) || 0;
}

export function signedMoney(n: number, negative: boolean): string {
  if (!n) return round2(0);
  const abs = Math.abs(n);
  return round2(negative ? -abs : abs);
}

function signedText(amount: string, negative: boolean): string {
  const raw = String(amount ?? "").trim().replace(/\s/g, "").replace(",", ".");
  const abs = raw.startsWith("-") ? raw.slice(1) : raw;
  if (!abs || num(abs) === 0) return round2(0);
  return negative ? `-${abs}` : abs;
}

export function calcLine(
  l: InvoiceLine,
  pricesIncludeVat: boolean,
  credit = false
): InvoiceLine {
  if (l.keep_amounts && l.net_amount && l.vat_amount && l.total_amount && !pricesIncludeVat) {
    return {
      ...l,
      net_amount: signedText(l.net_amount, credit),
      vat_amount: signedText(l.vat_amount, credit),
      total_amount: signedText(l.total_amount, credit),
    };
  }
  const q = num(l.quantity);
  const rate = num(l.vat_rate) / 100;
  let priceNet = num(l.unit_price);
  if (pricesIncludeVat) {
    priceNet = rate > 0 ? priceNet / (1 + rate) : priceNet;
  }
  const net = q * priceNet;
  const vat = net * rate;
  return {
    ...l,
    net_amount: signedMoney(net, credit),
    vat_amount: signedMoney(vat, credit),
    total_amount: signedMoney(net + vat, credit),
  };
}

export function calcTotals(lines: InvoiceLine[], discountPercent: string) {
  const subtotal = lines.reduce((s, l) => s + num(l.net_amount), 0);
  const discPct = num(discountPercent);
  const discount = (subtotal * discPct) / 100;
  const taxable = subtotal - discount;
  const factor = subtotal === 0 ? 0 : taxable / subtotal;
  const vat = lines.reduce((s, l) => s + num(l.vat_amount) * factor, 0);
  return {
    subtotal: round2(subtotal),
    discount: round2(discount),
    taxable: round2(taxable),
    vat: round2(vat),
    total: round2(taxable + vat),
  };
}

export function applyDiscountToLines(lines: InvoiceLine[], discountPercent: string): InvoiceLine[] {
  const discPct = num(discountPercent);
  if (discPct === 0) return lines;
  const factor = 1 - discPct / 100;
  return lines.map((l) => {
    const net = num(l.net_amount) * factor;
    const vat = num(l.vat_amount) * factor;
    return {
      ...l,
      net_amount: round2(net),
      vat_amount: round2(vat),
      total_amount: round2(net + vat),
    };
  });
}

export function docTypeTitle(t: string): string {
  switch (t) {
    case "02":
      return "ДЕБИТНО ИЗВЕСТИЕ";
    case "03":
      return "КРЕДИТНО ИЗВЕСТИЕ";
    case "04":
    case "05":
      return "РЕГИСТЪР НА СТОКИ ПОД РЕЖИМ СКЛАДИРАНЕ ДО ПОИСКВАНЕ";
    case "07":
      return "МИТНИЧЕСКА ДЕКЛАРАЦИЯ";
    case "09":
      return "ПРОТОКОЛ";
    case "11":
      return "ФАКТУРА - КАСОВА ОТЧЕТНОСТ";
    case "12":
      return "ДЕБИТНО ИЗВЕСТИЕ - КАСОВА ОТЧЕТНОСТ";
    case "13":
      return "КРЕДИТНО ИЗВЕСТИЕ - КАСОВА ОТЧЕТНОСТ";
    case "23":
      return "КРЕДИТНО ИЗВЕСТИЕ ПО ЧЛ. 126Б, АЛ. 1 ОТ ЗДДС";
    case "29":
      return "ПРОТОКОЛ ПО ЧЛ. 126Б, АЛ. 2 И 7 ОТ ЗДДС";
    case "81":
    case "83":
    case "84":
    case "85":
      return "ОТЧЕТ ЗА ИЗВЪРШЕНИТЕ ПРОДАЖБИ";
    case "82":
      return "ОТЧЕТ ЗА ПРОДАЖБИ ПРИ СПЕЦИАЛЕН РЕД НА ОБЛАГАНЕ";
    case "proforma":
      return "ПРОФОРМА ФАКТУРА";
    default:
      return "ФАКТУРА";
  }
}

export function docTypePrefix(t: string): string {
  switch (t) {
    case "02":
    case "12":
      return "ДИ-";
    case "03":
    case "13":
    case "23":
      return "КИ-";
    case "proforma":
      return "ПФ-";
    default:
      return "";
  }
}

export function docTypeFileSlug(t: string): string {
  switch (t) {
    case "02":
      return "debit_note";
    case "03":
      return "credit_note";
    case "04":
      return "call_off_dispatch";
    case "05":
      return "call_off_receipt";
    case "07":
      return "customs_declaration";
    case "09":
      return "protocol";
    case "11":
      return "invoice_cash";
    case "12":
      return "debit_note_cash";
    case "13":
      return "credit_note_cash";
    case "23":
      return "credit_note_126b";
    case "29":
      return "protocol_126b";
    case "50":
      return "fuel_protocol";
    case "81":
      return "sales_report";
    case "82":
      return "sales_report_special";
    case "83":
      return "sales_report_fuel";
    case "84":
      return "sales_report_bread";
    case "85":
      return "sales_report_flour";
    case "91":
      return "vat_protocol";
    case "92":
      return "vat_protocol_credit";
    case "93":
      return "vat_protocol_151v";
    case "94":
      return "vat_protocol_151v_scheme";
    case "95":
      return "food_protocol";
    case "proforma":
      return "proforma";
    default:
      return "invoice";
  }
}

export function invoiceDownloadName(number: string, documentType: string, ext: string): string {
  const n = (number || "document").replace(/["\\/:*?<>|]/g, "_").trim() || "document";
  return `${n}_${docTypeFileSlug(documentType)}.${ext}`;
}
