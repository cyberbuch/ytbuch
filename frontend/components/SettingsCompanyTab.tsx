"use client";

import { FormEvent } from "react";
import { SettingsField } from "./SettingsField";
import { Form } from "./settingsTypes";

export function SettingsCompanyTab({
  t,
  companyForm,
  setC,
  keys,
  setKeys,
  saving,
  onSubmit,
}: {
  t: (k: string) => string;
  companyForm: Form;
  setC: (k: string, v: string) => void;
  keys: { mistral_api_key: string; zhipu_api_key: string };
  setKeys: (fn: (k: { mistral_api_key: string; zhipu_api_key: string }) => { mistral_api_key: string; zhipu_api_key: string }) => void;
  saving: boolean;
  onSubmit: (e: FormEvent) => void;
}) {
  return (
        <form className="card card-pad" onSubmit={onSubmit} style={{ maxWidth: 760 }}>
          <h3 className="section-title">{t("settings.section.basic")}</h3>
          <div className="form-grid">
            <SettingsField label={`${t("companies.name")} *`} span={2}>
              <input className="input" value={companyForm.name ?? ""} onChange={(e) => setC("name", e.target.value)} required />
            </SettingsField>
            <SettingsField label={`${t("companies.eik")} *`}>
              <input className="input" value={companyForm.eik ?? ""} onChange={(e) => setC("eik", e.target.value)} required />
            </SettingsField>
            <SettingsField label={t("companies.vat_number")}>
              <input className="input" value={companyForm.vat_number ?? ""} onChange={(e) => setC("vat_number", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.address")} span={2}>
              <input className="input" value={companyForm.address ?? ""} onChange={(e) => setC("address", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.city")}>
              <input className="input" value={companyForm.city ?? ""} onChange={(e) => setC("city", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.post_code")}>
              <input className="input" value={companyForm.post_code ?? ""} onChange={(e) => setC("post_code", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("settings.country")}>
              <input className="input" value={companyForm.country ?? "BG"} onChange={(e) => setC("country", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.phone")}>
              <input className="input" value={companyForm.phone ?? ""} onChange={(e) => setC("phone", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.email")}>
              <input className="input" type="email" value={companyForm.email ?? ""} onChange={(e) => setC("email", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.website")}>
              <input className="input" value={companyForm.website ?? ""} onChange={(e) => setC("website", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.mol")}>
              <input className="input" value={companyForm.mol ?? ""} onChange={(e) => setC("mol", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.iban")}>
              <input className="input" value={companyForm.iban ?? ""} onChange={(e) => setC("iban", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.bic")}>
              <input className="input" value={companyForm.bic ?? ""} onChange={(e) => setC("bic", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.tax_authority")}>
              <input className="input" value={companyForm.tax_authority ?? ""} onChange={(e) => setC("tax_authority", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.nap_office")}>
              <input className="input" value={companyForm.nap_office ?? ""} onChange={(e) => setC("nap_office", e.target.value)} />
            </SettingsField>
          </div>

          <h3 className="section-title" style={{ marginTop: 22 }}>
            {t("settings.section.people")}
          </h3>
          <div className="inline-form" style={{ marginBottom: 12 }}>
            <h4 className="section-title" style={{ fontSize: 13 }}>
              {t("settings.manager")}
            </h4>
            <div className="form-grid">
              <SettingsField label={t("settings.person.name")}>
                <input className="input" value={companyForm.manager_name ?? ""} onChange={(e) => setC("manager_name", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.person.egn")}>
                <input className="input" value={companyForm.manager_egn ?? ""} onChange={(e) => setC("manager_egn", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("companies.manager_eik")} span={2}>
                <input className="input" value={companyForm.manager_eik ?? ""} onChange={(e) => setC("manager_eik", e.target.value)} />
              </SettingsField>
            </div>
          </div>
          <div className="inline-form" style={{ marginBottom: 12 }}>
            <h4 className="section-title" style={{ fontSize: 13 }}>
              {t("settings.accountant")}
            </h4>
            <div className="form-grid">
              <SettingsField label={t("settings.person.name")}>
                <input className="input" value={companyForm.accountant_name ?? ""} onChange={(e) => setC("accountant_name", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.person.egn")}>
                <input className="input" value={companyForm.accountant_egn ?? ""} onChange={(e) => setC("accountant_egn", e.target.value)} />
              </SettingsField>
            </div>
          </div>
          <div className="inline-form">
            <h4 className="section-title" style={{ fontSize: 13 }}>
              {t("settings.authorized")}
            </h4>
            <div className="form-grid">
              <SettingsField label={t("settings.person.name")}>
                <input className="input" value={companyForm.authorized_person_name ?? ""} onChange={(e) => setC("authorized_person_name", e.target.value)} />
              </SettingsField>
              <SettingsField label={t("settings.person.egn")}>
                <input className="input" value={companyForm.authorized_person_egn ?? ""} onChange={(e) => setC("authorized_person_egn", e.target.value)} />
              </SettingsField>
            </div>
          </div>

          <h3 className="section-title" style={{ marginTop: 22 }}>
            {t("settings.section.vat")}
          </h3>
          <div className="form-grid-3">
            <SettingsField label={t("companies.is_vat_registered")}>
              <select className="select" value={companyForm.is_vat_registered ?? "0"} onChange={(e) => setC("is_vat_registered", e.target.value)}>
                <option value="1">{t("common.yes")}</option>
                <option value="0">{t("common.no")}</option>
              </select>
            </SettingsField>
            <SettingsField label={t("companies.vat_period")}>
              <select className="select" value={companyForm.vat_period ?? "monthly"} onChange={(e) => setC("vat_period", e.target.value)}>
                <option value="monthly">{t("companies.vat_period.monthly")}</option>
                <option value="quarterly">{t("companies.vat_period.quarterly")}</option>
              </select>
            </SettingsField>
            <SettingsField label={t("companies.currency")}>
              <select className="select" value={companyForm.currency ?? "EUR"} onChange={(e) => setC("currency", e.target.value)}>
                <option value="EUR">EUR (€)</option>
                <option value="BGN">BGN</option>
                <option value="USD">USD</option>
              </select>
            </SettingsField>
            <SettingsField label={t("settings.vat_branch")}>
              <input className="input" value={companyForm.vat_branch_number ?? ""} onChange={(e) => setC("vat_branch_number", e.target.value)} />
            </SettingsField>
            <SettingsField label={t("companies.fiscal_year_start_month")}>
              <input className="input" type="number" min={1} max={12} value={companyForm.fiscal_year_start_month ?? "1"} onChange={(e) => setC("fiscal_year_start_month", e.target.value)} />
            </SettingsField>
          </div>

          <h3 className="section-title" style={{ marginTop: 22 }}>
            {t("settings.section.integrations")}
          </h3>
          <div className="form-grid">
            <SettingsField label={t("settings.mistral_key")} span={2}>
              <input className="input" type="password" autoComplete="off" value={keys.mistral_api_key} onChange={(e) => setKeys((k) => ({ ...k, mistral_api_key: e.target.value }))} />
              <p className="muted" style={{ fontSize: 12, margin: "4px 0 0" }}>
                {t("settings.mistral_hint")}
              </p>
            </SettingsField>
            <SettingsField label={t("settings.zhipu_key")} span={2}>
              <input className="input" value={keys.zhipu_api_key} onChange={(e) => setKeys((k) => ({ ...k, zhipu_api_key: e.target.value }))} />
              <p className="muted" style={{ fontSize: 12, margin: "4px 0 0" }}>
                {t("settings.zhipu_hint")}
              </p>
            </SettingsField>
          </div>
          <div className="form-actions">
            <button className="btn btn-primary" type="submit" disabled={saving}>
              {saving ? t("common.loading") : t("settings.save.company")}
            </button>
          </div>
        </form>
  );
}
