import { Company } from "../lib/api";
import { DocSeries } from "./DocumentSeriesTab";

export interface Location {
  id: number;
  name: string;
  location_type: string;
  street_name: string;
  building_number: string;
  city: string;
  post_code: string;
  region: string;
  country: string;
  is_main: number;
}

export interface Owner {
  id: number;
  first_name_bg: string;
  last_name_bg: string;
  egn: string;
  first_name_latin: string;
  last_name_latin: string;
  country: string;
  ownership_percentage: string;
}

export interface ParentCo {
  id: number;
  name_bg: string;
  uic: string;
  name_latin: string;
  country: string;
}

export interface SettingsPack {
  company: Company;
  locations: Location[];
  beneficial_owners: Owner[];
  ultimate_parents: ParentCo[];
  document_series?: DocSeries[];
}

export type Form = Record<string, string>;

export const EMPTY_LOC: Form = {
  name: "",
  location_type: "OFFICE",
  street_name: "",
  building_number: "",
  city: "",
  post_code: "",
  region: "",
  country: "BG",
  is_main: "0",
};

export const EMPTY_OWNER: Form = {
  first_name_bg: "",
  last_name_bg: "",
  egn: "",
  first_name_latin: "",
  last_name_latin: "",
  country: "BG",
  ownership_percentage: "0",
};

export const EMPTY_PARENT: Form = {
  name_bg: "",
  uic: "",
  name_latin: "",
  country: "BG",
};
