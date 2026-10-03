export interface InternalDocLine {
  id?: number;
  line_kind: "product" | "fixed_asset";
  product_id?: number;
  asset_id?: number;
  quantity: string;
  unit_cost: string;
  amount: string;
  description: string;
}

export interface InternalDoc {
  id: number;
  number: string;
  doc_date: string;
  from_location_id: number;
  to_location_id: number;
  status: string;
  notes?: string;
  handed_by?: string;
  received_by?: string;
  lines?: InternalDocLine[];
}

export function emptyIdocLine(): InternalDocLine {
  return {
    line_kind: "product",
    quantity: "1",
    unit_cost: "0",
    amount: "0",
    description: "",
  };
}

export function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}
