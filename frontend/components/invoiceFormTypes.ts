export interface InvoiceFormCounterpart {
  id: number;
  name: string;
  eik: string;
  vat_number: string;
  address: string;
  city: string;
  counterpart_type?: string;
}

export interface InvoiceFormProduct {
  id: number;
  name: string;
  code: string;
  unit: string;
  price: string;
  vat_rate: string;
}
