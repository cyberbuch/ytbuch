"use client";

import { api } from "../lib/api";
import { CURRENCIES, Invoice, docTypeRequiresOriginal, docTypesFor } from "../lib/invoice";
import { CompanyLocation } from "../lib/locations";
import { InvoiceFormCounterpart } from "./invoiceFormTypes";

export function InvoiceFormHeader({
  t,
  direction,
  setDirection,
  documentType,
  setDocumentType,
  isCredit,
  locations,
  locationId,
  setLocationId,
  number,
  setNumber,
  issueDate,
  setIssueDate,
  taxEventDate,
  setTaxEventDate,
  dueDate,
  setDueDate,
  currency,
  setCurrency,
  currencyRate,
  setCurrencyRate,
  originalInvoiceId,
  setOriginalInvoiceId,
  invoices,
  selectedCp,
  setCpOpen,
  setFormError,
  excludeVat,
  setExcludeVat,
  vatArticle,
  setVatArticle,
  customsStatistical,
  setCustomsStatistical,
  customsDuty,
  setCustomsDuty,
  customsVat,
  setCustomsVat,
}: {
  t: (k: string) => string;
  direction: string;
  setDirection: (v: string) => void;
  documentType: string;
  setDocumentType: (v: string) => void;
  isCredit: boolean;
  locations: CompanyLocation[];
  locationId: string;
  setLocationId: (v: string) => void;
  number: string;
  setNumber: (v: string) => void;
  issueDate: string;
  setIssueDate: (v: string) => void;
  taxEventDate: string;
  setTaxEventDate: (v: string) => void;
  dueDate: string;
  setDueDate: (v: string) => void;
  currency: string;
  setCurrency: (v: string) => void;
  currencyRate: string;
  setCurrencyRate: (v: string) => void;
  originalInvoiceId: string;
  setOriginalInvoiceId: (v: string) => void;
  invoices: Invoice[];
  selectedCp: InvoiceFormCounterpart | undefined;
  setCpOpen: (v: boolean) => void;
  setFormError: (v: string) => void;
  excludeVat: boolean;
  setExcludeVat: (v: boolean) => void;
  vatArticle: string;
  setVatArticle: (v: string) => void;
  customsStatistical: string;
  setCustomsStatistical: (v: string) => void;
  customsDuty: string;
  setCustomsDuty: (v: string) => void;
  customsVat: string;
  setCustomsVat: (v: string) => void;
}) {
  const isCustoms = direction === "in" && documentType === "07";
  const isProtocol = direction === "in" && documentType === "09";
  const canExcludeVat =
    direction === "in" && documentType !== "07" && documentType !== "09";
  return (
    <>
      <section className="card invoice-section">
        <h2 className="invoice-section-title">{t("invoices.header")}</h2>
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("invoices.document_type")}</label>
            <select
              className="select"
              name="document_type"
              value={documentType}
              onChange={(e) => setDocumentType(e.target.value)}
            >
              {docTypesFor(direction).map((dt) => (
                <option key={dt} value={dt}>
                  {dt === "proforma" ? "" : `${dt} — `}
                  {t(`invoices.document_type.${dt}`)}
                </option>
              ))}
            </select>
            {isCredit && (
              <p className="muted" style={{ marginTop: 6 }}>
                {t("invoices.credit_sign_hint")}
              </p>
            )}
          </div>
        </div>
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("invoices.direction")}</label>
            <select
              className="select"
              value={direction}
              onChange={(e) => setDirection(e.target.value)}
            >
              <option value="out">{t("invoices.direction.out")}</option>
              <option value="in">{t("invoices.direction.in")}</option>
            </select>
          </div>
          {locations.length > 0 && (
            <div className="field">
              <label className="label">{t("invoices.location")}</label>
              <select
                className="select"
                value={locationId}
                onChange={(e) => setLocationId(e.target.value)}
              >
                {locationId === "0" && <option value="0">—</option>}
                {locations.map((loc) => (
                  <option key={loc.id} value={loc.id}>
                    {loc.name}
                    {Number(loc.is_main) ? ` (${t("settings.is_main.yes")})` : ""}
                  </option>
                ))}
              </select>
            </div>
          )}
          <div className="field">
            <label className="label">
              {isCustoms ? t("invoices.customs_mrn") : t("invoices.number")}
              {direction === "in" && documentType !== "09" ? " *" : ""}
            </label>
            <input
              className="input"
              value={number}
              onChange={(e) => setNumber(e.target.value)}
              required={direction === "in" && documentType !== "09"}
              placeholder={
                direction === "in" && documentType !== "09"
                  ? t("invoices.number_in_hint")
                  : t("invoices.number_out_hint")
              }
            />
          </div>
          <div className="field">
            <label className="label">{t("invoices.issue_date")} *</label>
            <input
              className="input"
              type="date"
              value={issueDate}
              onChange={(e) => setIssueDate(e.target.value)}
              required
            />
          </div>
          <div className="field">
            <label className="label">{t("invoices.tax_event_date")}</label>
            <input
              className="input"
              type="date"
              value={taxEventDate}
              onChange={(e) => setTaxEventDate(e.target.value)}
            />
          </div>
          <div className="field">
            <label className="label">{t("invoices.due_date")}</label>
            <input
              className="input"
              type="date"
              value={dueDate}
              onChange={(e) => setDueDate(e.target.value)}
            />
          </div>
          <div className="field">
            <label className="label">{t("invoices.currency")}</label>
            <select
              className="select"
              value={currency}
              onChange={(e) => setCurrency(e.target.value)}
            >
              {CURRENCIES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </div>
          {currency !== "EUR" && (
            <div className="field">
              <label className="label">{t("invoices.currency_rate")}</label>
              <div style={{ display: "flex", gap: 6 }}>
                <input
                  className="input"
                  value={currencyRate}
                  onChange={(e) => setCurrencyRate(e.target.value)}
                />
                <button
                  type="button"
                  className="btn btn-ghost"
                  title={t("invoices.currency_rate_fetch_hint")}
                  onClick={async () => {
                    try {
                      const r = await api.get<{ rate: string; date: string }>(
                        `/v1/exchange-rates/rate?currency=${encodeURIComponent(currency)}&date=${encodeURIComponent(issueDate)}`
                      );
                      setCurrencyRate(r.rate);
                    } catch (err) {
                      setFormError(err instanceof Error ? err.message : String(err));
                    }
                  }}
                >
                  {t("invoices.currency_rate_fetch")}
                </button>
              </div>
            </div>
          )}
        </div>
        {(docTypeRequiresOriginal(documentType) || isProtocol || isCustoms) && (
          <div className="field">
            <label className="label">
              {t("invoices.original_invoice")}
              {docTypeRequiresOriginal(documentType) ? " *" : ""}
            </label>
            <select
              className="select"
              value={originalInvoiceId}
              onChange={(e) => setOriginalInvoiceId(e.target.value)}
              required={docTypeRequiresOriginal(documentType)}
            >
              <option value="">—</option>
              {invoices
                .filter((i) => i.document_type === "01" || i.document_type === "11")
                .map((i) => (
                  <option key={i.id} value={i.id}>
                    {i.number} · {i.issue_date} · {i.total_amount}
                  </option>
                ))}
            </select>
          </div>
        )}
        {isProtocol && (
          <div className="field" style={{ marginTop: 12 }}>
            <label className="label">{t("invoices.vat_article")}</label>
            <select
              className="select"
              value={vatArticle || "чл. 117"}
              onChange={(e) => setVatArticle(e.target.value)}
            >
              <option value="чл. 117">чл. 117 от ЗДДС (ВОП / чл. 82, ал. 2)</option>
              <option value="чл. 163а">чл. 163а от ЗДДС (обратно начисляване)</option>
              <option value="чл. 151в">чл. 151в от ЗДДС</option>
            </select>
            <p className="muted" style={{ marginTop: 6 }}>
              {t("invoices.protocol_hint")}
            </p>
          </div>
        )}
        {isCustoms && (
          <div
            style={{
              marginTop: 14,
              padding: 14,
              borderRadius: 8,
              border: "1px solid var(--border)",
              background: "var(--surface-2, rgba(0,0,0,0.03))",
            }}
          >
            <h3 style={{ margin: "0 0 8px", fontSize: 15 }}>
              {t("invoices.customs_modal_title")}
            </h3>
            <p className="muted" style={{ margin: "0 0 12px" }}>
              {t("invoices.customs_hint")}
            </p>
            <div className="form-grid">
              <div className="field">
                <label className="label">{t("invoices.customs_statistical")}</label>
                <input
                  className="input"
                  value={customsStatistical}
                  onChange={(e) => {
                    const st = e.target.value;
                    setCustomsStatistical(st);
                    const s = parseFloat(st.replace(",", ".")) || 0;
                    const d = parseFloat(customsDuty.replace(",", ".")) || 0;
                    setCustomsVat(((s + d) * 0.2).toFixed(2));
                  }}
                  placeholder="0.00"
                />
              </div>
              <div className="field">
                <label className="label">{t("invoices.customs_duty")}</label>
                <input
                  className="input"
                  value={customsDuty}
                  onChange={(e) => {
                    const du = e.target.value;
                    setCustomsDuty(du);
                    const s = parseFloat(customsStatistical.replace(",", ".")) || 0;
                    const d = parseFloat(du.replace(",", ".")) || 0;
                    setCustomsVat(((s + d) * 0.2).toFixed(2));
                  }}
                  placeholder="0.00"
                />
              </div>
              <div className="field">
                <label className="label">{t("invoices.customs_vat")}</label>
                <input
                  className="input"
                  value={customsVat}
                  onChange={(e) => setCustomsVat(e.target.value)}
                  placeholder="0.00"
                />
              </div>
            </div>
          </div>
        )}
        {canExcludeVat && (
          <div style={{ marginTop: 14 }}>
            <label
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 8,
                fontWeight: 500,
                cursor: "pointer",
              }}
            >
              <input
                type="checkbox"
                checked={excludeVat}
                onChange={(e) => setExcludeVat(e.target.checked)}
              />
              {t("invoices.exclude_vat")}
            </label>
            {excludeVat && (
              <div
                style={{
                  marginTop: 10,
                  padding: "10px 14px",
                  borderRadius: 8,
                  border: "1px solid #d97706",
                  background: "rgba(245, 158, 11, 0.12)",
                  color: "var(--fg)",
                  fontSize: 14,
                }}
              >
                ⚠️ {t("invoices.exclude_vat_warning")}
              </div>
            )}
          </div>
        )}
      </section>

      <section className="card invoice-section">
        <h2 className="invoice-section-title">
          {isCustoms
            ? t("invoices.customs_agency")
            : direction === "in"
              ? t("invoices.supplier")
              : t("invoices.recipient")}
        </h2>
        {isCustoms && !selectedCp && (
          <p className="muted" style={{ marginBottom: 8 }}>
            {t("invoices.customs_agency")} — Агенция „Митници“ (сметка 458)
          </p>
        )}
        <button
          type="button"
          className={`btn picker-trigger${selectedCp ? "" : " picker-empty"}`}
          onClick={() => setCpOpen(true)}
        >
          {selectedCp
            ? `${selectedCp.eik ? `${selectedCp.eik} — ` : ""}${selectedCp.name}`
            : isCustoms
              ? t("invoices.customs_agency")
              : t("invoices.pick_counterpart")}
        </button>
        {selectedCp && (
          <div className="party-preview">
            <div>
              {t("counterparts.name")}: {selectedCp.name}
            </div>
            <div>
              {t("counterparts.eik")}: {selectedCp.eik || "—"}
            </div>
            <div>
              {t("counterparts.vat_number")}: {selectedCp.vat_number || "—"}
            </div>
            <div>
              {t("companies.address")}: {selectedCp.address} {selectedCp.city}
            </div>
          </div>
        )}
      </section>


    </>
  );
}
