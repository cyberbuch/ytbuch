export type Kind =
  | "trial_balance"
  | "general_ledger"
  | "chronological"
  | "counterpart_trial"
  | "counterpart_chrono"
  | "correspondence_ledger"
  | "balance_sheet"
  | "income_statement"
  | "cash_flow"
  | "equity";

export interface Counterpart {
  id: number;
  name: string;
}

export interface TrialRow {
  account_number: string;
  account_name: string;
  opening_debit: string;
  opening_credit: string;
  turnover_debit: string;
  turnover_credit: string;
  closing_debit: string;
  closing_credit: string;
}

export interface TrialTotals {
  opening_debit: string;
  opening_credit: string;
  turnover_debit: string;
  turnover_credit: string;
  closing_debit: string;
  closing_credit: string;
}

export interface FirmHead {
  company_name?: string;
  company_eik?: string;
}

export interface TrialData extends FirmHead {
  kind: string;
  from: string;
  to: string;
  title: string;
  rows: TrialRow[];
  totals: TrialTotals;
}

export interface ChronoRow {
  row_number: number;
  date: string;
  document_number: string;
  description: string;
  counterpart_name: string;
  debit_account: string;
  credit_account: string;
  debit_amount: string;
  credit_amount: string;
}

export interface ChronoData extends FirmHead {
  kind: string;
  from: string;
  to: string;
  title: string;
  rows: ChronoRow[];
  totals: { debit_amount: string; credit_amount: string };
}

export interface LedgerTx {
  date: string;
  document_number: string;
  description: string;
  debit: string;
  credit: string;
  running_debit: string;
  running_credit: string;
}

export interface LedgerAccount {
  account_number: string;
  account_name: string;
  opening_debit: string;
  opening_credit: string;
  turnover_debit: string;
  turnover_credit: string;
  closing_debit: string;
  closing_credit: string;
  transactions: LedgerTx[];
}

export interface LedgerData extends FirmHead {
  kind: string;
  from: string;
  to: string;
  title: string;
  accounts: LedgerAccount[];
}

export interface CorrRow {
  correspondent: string;
  correspondent_name: string;
  debit: string;
  credit: string;
}

export interface CorrAccount {
  account_number: string;
  account_name: string;
  opening_debit: string;
  opening_credit: string;
  turnover_debit: string;
  turnover_credit: string;
  closing_debit: string;
  closing_credit: string;
  rows: CorrRow[];
}

export interface FsRow {
  code: string;
  kind: string;
  name: string;
  current: string;
  prior: string;
}

export interface FsData extends FirmHead {
  kind: string;
  from: string;
  to: string;
  title: string;
  rows: FsRow[];
}

export interface CorrData extends FirmHead {
  kind: string;
  from: string;
  to: string;
  title: string;
  debit_lead: CorrAccount[];
  credit_lead: CorrAccount[];
}

export function isCp(kind: Kind): boolean {
  return kind === "counterpart_trial" || kind === "counterpart_chrono";
}

export function isFs(kind: Kind): boolean {
  return (
    kind === "balance_sheet" ||
    kind === "income_statement" ||
    kind === "cash_flow" ||
    kind === "equity"
  );
}

export function firmLine(d: FirmHead): string {
  const name = (d.company_name || "").trim();
  const eik = (d.company_eik || "").trim();
  if (name && eik) return `${name} — ЕИК ${eik}`;
  if (name) return name;
  if (eik) return `ЕИК ${eik}`;
  return "";
}
