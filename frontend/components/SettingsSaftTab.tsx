"use client";

import { FormEvent } from "react";
import Link from "next/link";
import { IconButton } from "./IconButton";
import { SettingsField } from "./SettingsField";
import { Form, Location, Owner, ParentCo } from "./settingsTypes";

export function SettingsSaftTab(props: {
  t: (k: string) => string;
  saftForm: Form;
  setS: (k: string, v: string) => void;
  saving: boolean;
  splitting: boolean;
  splitNote: string;
  onSplitAddress: () => void;
  onSaveSaft: (e: FormEvent) => void;
  locations: Location[];
  showLoc: boolean;
  setShowLoc: (v: boolean) => void;
  editLocId: number;
  setEditLocId: (v: number) => void;
  locForm: Form;
  setLocForm: (v: Form) => void;
  setL: (k: string, v: string) => void;
  emptyLoc: Form;
  saveLoc: (e: FormEvent) => void;
  delLoc: (id: number) => void;
  owners: Owner[];
  showOwner: boolean;
  setShowOwner: (v: boolean) => void;
  editOwnerId: number;
  setEditOwnerId: (v: number) => void;
  ownerForm: Form;
  setOwnerForm: (v: Form) => void;
  setO: (k: string, v: string) => void;
  emptyOwner: Form;
  saveOwner: (e: FormEvent) => void;
  delOwner: (id: number) => void;
  parents: ParentCo[];
  showParent: boolean;
  setShowParent: (v: boolean) => void;
  editParentId: number;
  setEditParentId: (v: number) => void;
  parentForm: Form;
  setParentForm: (v: Form) => void;
  setP: (k: string, v: string) => void;
  emptyParent: Form;
  saveParent: (e: FormEvent) => void;
  delParent: (id: number) => void;
}) {
  const {
    t, saftForm, setS, saving, splitting, splitNote, onSplitAddress, onSaveSaft,
    locations, showLoc, setShowLoc, editLocId, setEditLocId, locForm, setLocForm, setL, emptyLoc, saveLoc, delLoc,
    owners, showOwner, setShowOwner, editOwnerId, setEditOwnerId, ownerForm, setOwnerForm, setO, emptyOwner, saveOwner, delOwner,
    parents, showParent, setShowParent, editParentId, setEditParentId, parentForm, setParentForm, setP, emptyParent, saveParent, delParent,
  } = props;
  return (
        <div style={{ maxWidth: 900 }}>
          <form className="card card-pad" onSubmit={onSaveSaft} style={{ marginBottom: 16 }}>
            <h3 className="section-title">{t("settings.section.saft")}</h3>
            <div className="form-actions" style={{ justifyContent: "flex-start", marginBottom: 12 }}>
              <button type="button" className="btn" onClick={onSplitAddress} disabled={splitting || saving}>
                {splitting ? t("settings.split_address.running") : t("settings.split_address")}
              </button>
              {splitNote ? <span className="muted">{splitNote}</span> : null}
            </div>
            <div className="form-grid-3">
              <SettingsField label={t("settings.street")}>
                <input className="input" value={saftForm.street_name ?? ""} onChange={(e) => setS("street_name", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.building")}>
                <input className="input" value={saftForm.building_number ?? ""} onChange={(e) => setS("building_number", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.address_building")}>
                <input className="input" value={saftForm.address_building ?? ""} onChange={(e) => setS("address_building", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.additional_address")}>
                <input className="input" value={saftForm.additional_address_detail ?? ""} onChange={(e) => setS("additional_address_detail", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.address_type")}>
                <select className="select" value={saftForm.address_type ?? ""} onChange={(e) => setS("address_type", e.target.value)}>
                  <option value="">{t("address_type.empty")}</option>
                  <option value="StreetAddress">{t("address_type.street")}</option>
                  <option value="PostalAddress">{t("address_type.postal")}</option>
                  <option value="BillingAddress">{t("address_type.billing")}</option>
                  <option value="ShipToAddress">{t("address_type.shipto")}</option>
                  <option value="ShipFromAddress">{t("address_type.shipfrom")}</option>
                </select>
              </SettingsField>
              <SettingsField label={t("settings.region")}>
                <input className="input" placeholder="BG-22" value={saftForm.region ?? ""} onChange={(e) => setS("region", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.tax_basis")}>
                <select className="select" value={saftForm.tax_accounting_basis ?? "A"} onChange={(e) => setS("tax_accounting_basis", e.target.value)}>
                  <option value="A">{t("settings.tax_basis.A")}</option>
                  <option value="BANK">{t("settings.tax_basis.BANK")}</option>
                  <option value="P">{t("settings.tax_basis.P")}</option>
                </select>
              </SettingsField>
              <SettingsField label={t("companies.inventory_valuation_method")}>
                <select className="select" value={saftForm.inventory_valuation_method ?? "WAC"} onChange={(e) => setS("inventory_valuation_method", e.target.value)}>
                  <option value="WAC">{t("settings.inv.WAC")}</option>
                  <option value="FIFO">{t("settings.inv.FIFO")}</option>
                  <option value="LIFO">{t("settings.inv.LIFO")}</option>
                </select>
              </SettingsField>
              <SettingsField label={t("settings.group")}>
                <select className="select" value={saftForm.is_part_of_group ?? ""} onChange={(e) => setS("is_part_of_group", e.target.value)}>
                  <option value="">{t("settings.group.empty")}</option>
                  <option value="1">{t("settings.group.1")}</option>
                  <option value="2">{t("settings.group.2")}</option>
                  <option value="3">{t("settings.group.3")}</option>
                  <option value="4">{t("settings.group.4")}</option>
                  <option value="5">{t("settings.group.5")}</option>
                </select>
              </SettingsField>
              <SettingsField label={t("settings.tax_entity")}>
                <input className="input" value={saftForm.tax_entity ?? ""} onChange={(e) => setS("tax_entity", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.software_name")}>
                <input className="input" value={saftForm.software_company_name ?? ""} onChange={(e) => setS("software_company_name", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.software_id")}>
                <input className="input" value={saftForm.software_id ?? ""} onChange={(e) => setS("software_id", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.software_version")}>
                <input className="input" value={saftForm.software_version ?? ""} onChange={(e) => setS("software_version", e.target.value)} />
              </SettingsField>
            </div>
            <div className="form-actions">
              <button className="btn btn-primary" type="submit" disabled={saving}>
                {saving ? t("common.loading") : t("settings.save.saft")}
              </button>
            </div>
          </form>

          <div className="card card-pad" style={{ marginBottom: 16 }}>
            <div className="page-head" style={{ marginBottom: 12 }}>
              <h3 className="section-title" style={{ margin: 0 }}>
                {t("settings.locations")}
              </h3>
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => {
                  setEditLocId(0);
                  setLocForm(emptyLoc);
                  setShowLoc(true);
                }}
              >
                {t("settings.add.location")}
              </button>
            </div>
            {showLoc && (
              <form className="inline-form" onSubmit={saveLoc}>
                <h4 className="section-title" style={{ fontSize: 13 }}>
                  {editLocId > 0 ? t("common.edit") : t("common.create")}
                </h4>
                <div className="form-grid-3">
                  <SettingsField label={`${t("companies.name")} *`}>
                    <input className="input" value={locForm.name} onChange={(e) => setL("name", e.target.value)} required />
                  </SettingsField>
                  <SettingsField label={t("settings.location_type")}>
                    <select className="select" value={locForm.location_type} onChange={(e) => setL("location_type", e.target.value)}>
                      <option value="OFFICE">{t("settings.loc.OFFICE")}</option>
                      <option value="STORE">{t("settings.loc.STORE")}</option>
                      <option value="WAREHOUSE">{t("settings.loc.WAREHOUSE")}</option>
                      <option value="BRANCH">{t("settings.loc.BRANCH")}</option>
                      <option value="OTHER">{t("settings.loc.OTHER")}</option>
                    </select>
                  </SettingsField>
                  <SettingsField label={t("settings.is_main")}>
                    <select className="select" value={locForm.is_main} onChange={(e) => setL("is_main", e.target.value)}>
                      <option value="0">{t("common.no")}</option>
                      <option value="1">{t("settings.is_main.yes")}</option>
                    </select>
                  </SettingsField>
                  <SettingsField label={t("settings.street")}>
                    <input className="input" value={locForm.street_name} onChange={(e) => setL("street_name", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.building")}>
                    <input className="input" value={locForm.building_number} onChange={(e) => setL("building_number", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("companies.city")}>
                    <input className="input" value={locForm.city} onChange={(e) => setL("city", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("companies.post_code")}>
                    <input className="input" value={locForm.post_code} onChange={(e) => setL("post_code", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.region")}>
                    <input className="input" placeholder="BG-22" value={locForm.region} onChange={(e) => setL("region", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.country")}>
                    <input className="input" value={locForm.country} onChange={(e) => setL("country", e.target.value)} />
                  </SettingsField>
                </div>
                <div className="form-actions">
                  <button type="button" className="btn" onClick={() => setShowLoc(false)}>
                    {t("common.cancel")}
                  </button>
                  <button className="btn btn-primary" type="submit" disabled={saving}>
                    {t("common.save")}
                  </button>
                </div>
              </form>
            )}
            {locations.length === 0 && <p className="muted">{t("settings.empty.locations")}</p>}
            {locations.map((loc) => (
              <div className="list-item" key={loc.id}>
                <div>
                  <div style={{ fontWeight: 600 }}>
                    {loc.name}
                    {Number(loc.is_main) ? (
                      <span className="badge badge-success" style={{ marginLeft: 8 }}>
                        {t("settings.is_main.yes")}
                      </span>
                    ) : null}
                  </div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {t(`settings.loc.${loc.location_type}`)}
                    {loc.city ? ` — ${loc.city}` : ""}
                    {loc.street_name ? `, ${loc.street_name}` : ""}
                    {loc.building_number ? ` ${loc.building_number}` : ""}
                  </div>
                </div>
                <div className="icon-actions">
                  <IconButton
                    icon="edit"
                    title={t("common.edit")}
                    onClick={() => {
                      setEditLocId(loc.id);
                      setLocForm({
                        name: loc.name,
                        location_type: loc.location_type,
                        street_name: loc.street_name ?? "",
                        building_number: loc.building_number ?? "",
                        city: loc.city ?? "",
                        post_code: loc.post_code ?? "",
                        region: loc.region ?? "",
                        country: loc.country || "BG",
                        is_main: Number(loc.is_main) ? "1" : "0",
                      });
                      setShowLoc(true);
                    }}
                  />
                  <IconButton
                    icon="delete"
                    title={t("common.delete")}
                    danger
                    onClick={() => delLoc(loc.id)}
                  />
                </div>
              </div>
            ))}
          </div>

          <div className="card card-pad" style={{ marginBottom: 16 }}>
            <div className="page-head" style={{ marginBottom: 12 }}>
              <h3 className="section-title" style={{ margin: 0 }}>
                {t("settings.owners")}
              </h3>
              <Link href="/dividends" className="btn btn-sm">{t("settings.to_dividends")}</Link>
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => {
                  setEditOwnerId(0);
                  setOwnerForm(emptyOwner);
                  setShowOwner(true);
                }}
              >
                {t("settings.add.owner")}
              </button>
            </div>
            {showOwner && (
              <form className="inline-form" onSubmit={saveOwner}>
                <h4 className="section-title" style={{ fontSize: 13 }}>
                  {editOwnerId > 0 ? t("common.edit") : t("common.create")}
                </h4>
                <div className="form-grid-3">
                  <SettingsField label={`${t("settings.owner.first_bg")} *`}>
                    <input className="input" value={ownerForm.first_name_bg} onChange={(e) => setO("first_name_bg", e.target.value)} required />
                  </SettingsField>
                  <SettingsField label={`${t("settings.owner.last_bg")} *`}>
                    <input className="input" value={ownerForm.last_name_bg} onChange={(e) => setO("last_name_bg", e.target.value)} required />
                  </SettingsField>
                  <SettingsField label={t("settings.person.egn")}>
                    <input className="input" value={ownerForm.egn} onChange={(e) => setO("egn", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.owner.first_lat")}>
                    <input className="input" value={ownerForm.first_name_latin} onChange={(e) => setO("first_name_latin", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.owner.last_lat")}>
                    <input className="input" value={ownerForm.last_name_latin} onChange={(e) => setO("last_name_latin", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.country")}>
                    <input className="input" value={ownerForm.country} onChange={(e) => setO("country", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.owner.percent")}>
                    <input className="input" type="number" step="0.01" value={ownerForm.ownership_percentage} onChange={(e) => setO("ownership_percentage", e.target.value)} />
                  </SettingsField>
                </div>
                <div className="form-actions">
                  <button type="button" className="btn" onClick={() => setShowOwner(false)}>
                    {t("common.cancel")}
                  </button>
                  <button className="btn btn-primary" type="submit" disabled={saving}>
                    {t("common.save")}
                  </button>
                </div>
              </form>
            )}
            {owners.length === 0 && <p className="muted">{t("settings.empty.owners")}</p>}
            {owners.map((o) => (
              <div className="list-item" key={o.id}>
                <div>
                  <div style={{ fontWeight: 600 }}>
                    {o.first_name_bg} {o.last_name_bg}
                  </div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {o.ownership_percentage}% {t("settings.owner.owned")}
                    {o.egn ? ` | ${t("settings.person.egn")}: ${o.egn}` : ""} | {o.country}
                  </div>
                </div>
                <div className="icon-actions">
                  <IconButton
                    icon="edit"
                    title={t("common.edit")}
                    onClick={() => {
                      setEditOwnerId(o.id);
                      setOwnerForm({
                        first_name_bg: o.first_name_bg,
                        last_name_bg: o.last_name_bg,
                        egn: o.egn ?? "",
                        first_name_latin: o.first_name_latin ?? "",
                        last_name_latin: o.last_name_latin ?? "",
                        country: o.country || "BG",
                        ownership_percentage: o.ownership_percentage ?? "0",
                      });
                      setShowOwner(true);
                    }}
                  />
                  <IconButton
                    icon="delete"
                    title={t("common.delete")}
                    danger
                    onClick={() => delOwner(o.id)}
                  />
                </div>
              </div>
            ))}
          </div>

          <div className="card card-pad">
            <div className="page-head" style={{ marginBottom: 12 }}>
              <h3 className="section-title" style={{ margin: 0 }}>
                {t("settings.parents")}
              </h3>
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => {
                  setEditParentId(0);
                  setParentForm(emptyParent);
                  setShowParent(true);
                }}
              >
                {t("settings.add.parent")}
              </button>
            </div>
            {showParent && (
              <form className="inline-form" onSubmit={saveParent}>
                <h4 className="section-title" style={{ fontSize: 13 }}>
                  {editParentId > 0 ? t("common.edit") : t("common.create")}
                </h4>
                <div className="form-grid">
                  <SettingsField label={`${t("settings.parent.name_bg")} *`}>
                    <input className="input" value={parentForm.name_bg} onChange={(e) => setP("name_bg", e.target.value)} required />
                  </SettingsField>
                  <SettingsField label={t("settings.parent.uic")}>
                    <input className="input" value={parentForm.uic} onChange={(e) => setP("uic", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.parent.name_lat")}>
                    <input className="input" value={parentForm.name_latin} onChange={(e) => setP("name_latin", e.target.value)} />
                  </SettingsField>
                  <SettingsField label={t("settings.country")}>
                    <input className="input" value={parentForm.country} onChange={(e) => setP("country", e.target.value)} />
                  </SettingsField>
                </div>
                <div className="form-actions">
                  <button type="button" className="btn" onClick={() => setShowParent(false)}>
                    {t("common.cancel")}
                  </button>
                  <button className="btn btn-primary" type="submit" disabled={saving}>
                    {t("common.save")}
                  </button>
                </div>
              </form>
            )}
            {parents.length === 0 && <p className="muted">{t("settings.empty.parents")}</p>}
            {parents.map((p) => (
              <div className="list-item" key={p.id}>
                <div>
                  <div style={{ fontWeight: 600 }}>{p.name_bg}</div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {p.uic ? `${t("settings.parent.uic")}: ${p.uic} | ` : ""}
                    {p.country}
                  </div>
                </div>
                <div className="icon-actions">
                  <IconButton
                    icon="edit"
                    title={t("common.edit")}
                    onClick={() => {
                      setEditParentId(p.id);
                      setParentForm({
                        name_bg: p.name_bg,
                        uic: p.uic ?? "",
                        name_latin: p.name_latin ?? "",
                        country: p.country || "BG",
                      });
                      setShowParent(true);
                    }}
                  />
                  <IconButton
                    icon="delete"
                    title={t("common.delete")}
                    danger
                    onClick={() => delParent(p.id)}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
  );
}
