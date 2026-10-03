"use client";

import { RequireAuth } from "../../components/RequireAuth";
import { CrudPage, CrudConfig } from "../../components/CrudPage";

const config: CrudConfig = {
  endpoint: "/v1/counterparts",
  titleKey: "counterparts.title",
  vies: {
    endpoint: "/v1/counterparts/vies-lookup",
    vatField: "vat_number",
    eikField: "eik",
    map: {
      name: "name",
      address: "address",
      vies_address: "vies_address",
      country_code: "country",
      street_name: "street_name",
      building_number: "building_number",
      address_building: "address_building",
      additional_address_detail: "additional_address_detail",
      address_type: "address_type",
      city: "city",
      post_code: "post_code",
      region: "region",
    },
    labelKey: "counterparts.vies.fetch",
    loadingKey: "counterparts.vies.fetching",
    invalidKey: "counterparts.vies.invalid",
    filledKey: "counterparts.vies.filled",
  },
  addressSplit: {
    labelKey: "counterparts.split_address",
    loadingKey: "counterparts.split_address.running",
    emptyKey: "counterparts.split_address.empty",
  },
  fields: [
    { name: "name", labelKey: "counterparts.name", type: "text", required: true },
    {
      name: "counterpart_type",
      labelKey: "counterparts.type",
      type: "select",
      default: "both",
      options: [
        { value: "customer", labelKey: "counterparts.type.customer" },
        { value: "supplier", labelKey: "counterparts.type.supplier" },
        { value: "both", labelKey: "counterparts.type.both" },
      ],
    },
    { name: "eik", labelKey: "counterparts.eik", type: "text" },
    { name: "vat_number", labelKey: "counterparts.vat_number", type: "text" },
    { name: "address", labelKey: "counterparts.address", type: "text" },
    { name: "vies_address", labelKey: "counterparts.vies_address", type: "textarea" },
    { name: "street_name", labelKey: "counterparts.street_name", type: "text" },
    { name: "building_number", labelKey: "counterparts.building_number", type: "text" },
    { name: "address_building", labelKey: "counterparts.address_building", type: "text" },
    { name: "additional_address_detail", labelKey: "counterparts.additional_address", type: "text" },
    {
      name: "address_type",
      labelKey: "counterparts.address_type",
      type: "select",
      default: "",
      options: [
        { value: "", labelKey: "address_type.empty" },
        { value: "StreetAddress", labelKey: "address_type.street" },
        { value: "PostalAddress", labelKey: "address_type.postal" },
        { value: "BillingAddress", labelKey: "address_type.billing" },
        { value: "ShipToAddress", labelKey: "address_type.shipto" },
        { value: "ShipFromAddress", labelKey: "address_type.shipfrom" },
      ],
    },
    { name: "city", labelKey: "counterparts.city", type: "text" },
    { name: "country", labelKey: "counterparts.country", type: "text", default: "BG" },
    { name: "post_code", labelKey: "counterparts.post_code", type: "text" },
    { name: "region", labelKey: "counterparts.region", type: "text" },
    { name: "contact_person", labelKey: "counterparts.contact_person", type: "text" },
    { name: "email", labelKey: "counterparts.email", type: "text" },
    { name: "phone", labelKey: "counterparts.phone", type: "text" },
  ],
  columns: ["name", "counterpart_type", "eik", "city"],
};

export default function CounterpartsPage() {
  return (
    <RequireAuth>
      <CrudPage config={config} />
    </RequireAuth>
  );
}
