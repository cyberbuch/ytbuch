export interface InvoiceFormCounterpart {
  id: number;
  name: string;
  eik: string;
  vat_number: string;
  address: string;
  city: string;
  counterpart_type?: string;
}

export interface InvoiceFormAccount {
  id: number;
  number: string;
  name: string;
}

export interface InvoiceFormProduct {
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
