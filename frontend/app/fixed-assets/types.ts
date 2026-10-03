export interface Fac {
  id: number;
  name: string;
  cita_category: string;
  min_depreciation_rate: string;
  max_depreciation_rate: string;
  default_method: string;
}

export interface Fa {
  id: number;
  name: string;
  inventory_number: string;
  category_id?: number;
  acquisition_date: string;
  put_into_service_date: string;
  cost: string;
  salvage_value: string;
  useful_life_months: string;
  depreciation_method: string;
  accounting_depreciation_rate: string;
  tax_depreciation_rate: string;
  accumulated_depreciation: string;
  status: string;
  serial_number: string;
  manufacturer: string;
  model: string;
  responsible_person: string;
  is_conserved: number;
  location_id?: number;
}

export interface DepItem {
  id: number;
  name: string;
  method: string;
  monthly_amount: string;
}

export interface DepPreview {
  items: DepItem[];
  total_amount: string;
}
