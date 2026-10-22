export interface Counterpart {
  id: number;
  name: string;
  eik: string;
  vat_number: string;
  counterpart_type?: string;
}

export interface Account {
  id: number;
  number: string;
  name: string;
}

export interface Product {
  id: number;
  name: string;
  code: string;
  description?: string;
  unit: string;
  price: string;
  cost_price?: string;
  vat_rate: string;
  is_service?: number;
  is_inventory?: number;
  inventory_account_id?: number;
  revenue_account_id?: number;
  is_active?: number;
}

export interface ScanLine {
  description: string;
  seller_code?: string;
  quantity: string;
  unit: string;
  unit_price: string;
  vat_rate: string;
  net_amount: string;
  vat_amount: string;
  total_amount: string;
  product_id?: number;
}

export interface ScanExtract {
  direction: string;
  filename: string;
  multipage?: boolean;
  pages_scanned?: number;
  vat_registered?: boolean;
  invoice_number: string;
  issue_date: string;
  due_date: string;
  document_type: string;
  counterpart_name: string;
  counterpart_eik: string;
  counterpart_vat_number: string;
  currency: string;
  currency_rate?: string;
  payment_method?: string;
  payment_note?: string;
  vat_exemption_reason?: string;
  tax_event_date?: string;
  original_invoice_number?: string;
  original_invoice_id?: number;
  total_net_amount: string;
  total_vat_amount: string;
  total_amount: string;
  lines: ScanLine[];
  counterpart_id?: number;
  counterpart?: Counterpart;
}

export interface ViesLookup {
  valid: boolean | number;
  name: string;
  address: string;
}

export interface QueuedFile {
  id: string;
  file: File;
  multipage?: boolean;
}
