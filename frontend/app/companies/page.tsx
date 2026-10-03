"use client";

import { RequireAuth } from "../../components/RequireAuth";
import { CrudPage, CrudConfig } from "../../components/CrudPage";
import { setActiveCompany } from "../../lib/api";

const config: CrudConfig = {
  endpoint: "/v1/companies",
  titleKey: "companies.title",
  fields: [
    { name: "name", labelKey: "companies.name", type: "text", required: true },
    { name: "eik", labelKey: "companies.eik", type: "text", required: true },
    { name: "vat_number", labelKey: "companies.vat_number", type: "text" },
    { name: "is_vat_registered", labelKey: "companies.is_vat_registered", type: "checkbox" },
    {
      name: "vat_period",
      labelKey: "companies.vat_period",
      type: "select",
      default: "monthly",
      options: [
        { value: "monthly", labelKey: "companies.vat_period.monthly" },
        { value: "quarterly", labelKey: "companies.vat_period.quarterly" },
      ],
    },
    {
      name: "currency",
      labelKey: "companies.currency",
      type: "select",
      default: "EUR",
      options: [
        { value: "EUR", labelKey: "companies.currency.EUR" },
        { value: "BGN", labelKey: "companies.currency.BGN" },
        { value: "USD", labelKey: "companies.currency.USD" },
      ],
    },
    { name: "address", labelKey: "companies.address", type: "text" },
    { name: "city", labelKey: "companies.city", type: "text" },
    { name: "post_code", labelKey: "companies.post_code", type: "text" },
    { name: "phone", labelKey: "companies.phone", type: "text" },
    { name: "email", labelKey: "companies.email", type: "text" },
    { name: "website", labelKey: "companies.website", type: "text" },
    { name: "mol", labelKey: "companies.mol", type: "text" },
    { name: "manager_eik", labelKey: "companies.manager_eik", type: "text" },
    { name: "accountant_name", labelKey: "companies.accountant_name", type: "text" },
    { name: "accountant_egn", labelKey: "companies.accountant_egn", type: "text" },
    { name: "tax_authority", labelKey: "companies.tax_authority", type: "text" },
    { name: "nap_office", labelKey: "companies.nap_office", type: "text" },
    { name: "iban", labelKey: "companies.iban", type: "text" },
    { name: "bic", labelKey: "companies.bic", type: "text" },
    {
      name: "inventory_valuation_method",
      labelKey: "companies.inventory_valuation_method",
      type: "select",
      default: "WAC",
      options: [
        { value: "WAC", labelKey: "settings.inv.WAC" },
        { value: "FIFO", labelKey: "settings.inv.FIFO" },
        { value: "LIFO", labelKey: "settings.inv.LIFO" },
      ],
    },
    { name: "fiscal_year_start_month", labelKey: "companies.fiscal_year_start_month", type: "number", default: "1" },
  ],
  columns: ["name", "manager", "eik", "city", "is_vat_registered"],
  columnLabelKey: { manager: "companies.manager" },
  columnText: (column, rec) => {
    if (column !== "manager") return null;
    const manager = String(rec.manager_name ?? "").trim();
    if (manager) return manager;
    return String(rec.mol ?? "").trim();
  },
  rowAction: {
    labelKey: "companies.activate",
    onClick: (rec) => {
      const id = Number(rec.id);
      if (id > 0) void setActiveCompany(id);
    },
  },
};

export default function CompaniesPage() {
  return (
    <RequireAuth>
      <CrudPage config={config} />
    </RequireAuth>
  );
}
