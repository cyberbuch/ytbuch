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
}) {
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
              {t("invoices.number")}
              {direction === "in" ? " *" : ""}
            </label>
            <input
              className="input"
              value={number}
              onChange={(e) => setNumber(e.target.value)}
              required={direction === "in"}
              placeholder={
                direction === "in"
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
        {docTypeRequiresOriginal(documentType) && (
          <div className="field">
            <label className="label">{t("invoices.original_invoice")} *</label>
            <select
              className="select"
              value={originalInvoiceId}
              onChange={(e) => setOriginalInvoiceId(e.target.value)}
              required
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
      </section>

      <section className="card invoice-section">
        <h2 className="invoice-section-title">
          {direction === "in" ? t("invoices.supplier") : t("invoices.recipient")}
        </h2>
        <button
          type="button"
          className={`btn picker-trigger${selectedCp ? "" : " picker-empty"}`}
          onClick={() => setCpOpen(true)}
        >
          {selectedCp
            ? `${selectedCp.eik ? `${selectedCp.eik} — ` : ""}${selectedCp.name}`
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
