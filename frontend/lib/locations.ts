import { api, getActiveCompany, ListResponse } from "./api";

export interface CompanyLocation {
  id: number;
  name: string;
  location_type: string;
  is_main: number;
  city?: string;
}

export function mainLocationId(locs: CompanyLocation[]): number {
  const main = locs.find((l) => Number(l.is_main) === 1);
  return main?.id ?? locs[0]?.id ?? 0;
}

export async function fetchCompanyLocations(): Promise<CompanyLocation[]> {
  try {
    const co = await getActiveCompany();
    const id = (co as { id?: number }).id;
    if (!id) return [];
    const pack = await api.get<{ locations?: CompanyLocation[] }>(
      `/v1/companies/${id}/settings`
    );
    return pack.locations ?? [];
  } catch {
    return [];
  }
}

export interface StockAtLocation {
  product_id: number;
  name: string;
  code: string;
  unit: string;
  quantity: string;
  unit_cost: string;
}

export async function fetchStockAtLocation(
  locationId: number
): Promise<StockAtLocation[]> {
  if (!locationId) return [];
  const data = await api.get<ListResponse<StockAtLocation>>(
    `/v1/inventory/at-location?location_id=${locationId}`
  );
  return data.items ?? [];
}
