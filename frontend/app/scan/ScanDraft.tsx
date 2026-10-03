"use client";

import { CURRENCIES, Invoice, InvoiceLine, PAY_METHODS, VAT_RATES, docTypeRequiresOriginal, docTypesFor, emptyLine } from "../../lib/invoice";
import { IconButton } from "../../components/IconButton";
import { useI18n } from "../../components/I18nProvider";
import { VatExemption, vatexLabel } from "../../lib/vatExemptions";
import { Counterpart, Product, ViesLookup } from "./types";

export function ScanDraft(props: {
  t: (k: string) => string;
  direction: "in" | "out";
  documentType: string;
  setDocumentType: (v: string) => void;
  number: string;
  setNumber: (v: string) => void;
  issueDate: string;
  setIssueDate: (v: string) => void;
  dueDate: string;
  setDueDate: (v: string) => void;
  currency: string;
  setCurrency: (v: string) => void;
  paymentMethod: string;
  setPaymentMethod: (v: string) => void;
  vatExemption: string;
  setVatExemption: (v: string) => void;
  exemptions: VatExemption[];
  showExemption: boolean;
  originalInvoiceId: string;
  setOriginalInvoiceId: (v: string) => void;
  origNumber: string;
  invoiceChoices: Invoice[];
  ocrName: string;
  setOcrName: (v: string) => void;
  ocrEik: string;
  setOcrEik: (v: string) => void;
  ocrVat: string;
  setOcrVat: (v: string) => void;
  counterpartId: string;
  setCounterpartId: (v: string) => void;
  counterparts: Counterpart[];
  found: Counterpart | undefined;
  createCounterpart: () => void;
  vies: ViesLookup | null;
  priced: InvoiceLine[];
  productById: Map<number, Product>;
  setLine: (i: number, patch: Partial<InvoiceLine>) => void;
  openProductPicker: (i: number) => void;
  clearProduct: (i: number) => void;
  setLines: (fn: (prev: InvoiceLine[]) => InvoiceLine[]) => void;
  totals: { total: string };
  saveDraft: () => void;
  itemOpenFor: number | null;
  closeProductPicker: () => void;
  lines: InvoiceLine[];
  addingProduct: boolean;
  itemQuery: string;
  setItemQuery: (v: string) => void;
  filteredProducts: Product[];
  pickProduct: (i: number, p: Product) => void;
  setAddingProduct: (v: boolean) => void;
  setNewName: (v: string) => void;
  setCreateError: (v: string) => void;
  createError: string;
  newName: string;
  newCode: string;
  setNewCode: (v: string) => void;
  createProduct: () => void;
  creating: boolean;
}) {
  const {
    t, direction, documentType, setDocumentType, number, setNumber, issueDate, setIssueDate,
    dueDate, setDueDate, currency, setCurrency, paymentMethod, setPaymentMethod,
    vatExemption, setVatExemption, exemptions, showExemption, originalInvoiceId,
    setOriginalInvoiceId, origNumber, invoiceChoices, ocrName, setOcrName, ocrEik, setOcrEik,
    ocrVat, setOcrVat, counterpartId, setCounterpartId, counterparts, found, createCounterpart,
    vies, priced, productById, setLine, openProductPicker, clearProduct, setLines, totals,
    saveDraft, itemOpenFor, closeProductPicker, lines, addingProduct, itemQuery, setItemQuery,
    filteredProducts, pickProduct, setAddingProduct, setNewName, setCreateError, createError,
    newName, newCode, setNewCode, createProduct, creating,
  } = props;
  const { lang } = useI18n();
  return (
    <>
          <section className="card invoice-section">
            <h2 className="invoice-section-title">{t("scan.header")}</h2>
            <div className="form-grid">
              <div className="field">
                <label className="label">{t("invoices.document_type")}</label>
                <select
                  className="select"
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
              </div>
              <div className="field">
                <label className="label">{t("scan.number")}</label>
                <input
                  className="input"
                  value={number}
                  onChange={(e) => setNumber(e.target.value)}
                />
              </div>
              <div className="field">
                <label className="label">{t("scan.date")}</label>
                <input
                  className="input"
                  type="date"
                  value={issueDate}
                  onChange={(e) => setIssueDate(e.target.value)}
                />
              </div>
              <div className="field">
                <label className="label">{t("scan.due")}</label>
                <input
                  className="input"
                  type="date"
                  value={dueDate}
                  onChange={(e) => setDueDate(e.target.value)}
                />
              </div>
              <div className="field">
                <label className="label">{t("scan.currency")}</label>
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
              <div className="field">
                <label className="label">{t("invoices.payment_method")}</label>
                <select
                  className="select"
                  value={paymentMethod}
                  onChange={(e) => setPaymentMethod(e.target.value)}
                >
                  {PAY_METHODS.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
              {showExemption && (
                <div className="field">
                  <label className="label">{t("invoices.vat_exemption")} *</label>
                  <select
                    className="select"
                    value={vatExemption}
                    onChange={(e) => setVatExemption(e.target.value)}
                  >
                    <option value="">{t("invoices.vat_exemption_pick")}</option>
                    {exemptions.map((e) => (
                      <option key={e.code} value={e.code}>
                        {vatexLabel(e, lang)}
                      </option>
                    ))}
                  </select>
                </div>
              )}
              {docTypeRequiresOriginal(documentType) && (
                <div className="field">
                  <label className="label">{t("invoices.original_invoice")} *</label>
                  <select
                    className="select"
                    value={originalInvoiceId}
                    onChange={(e) => setOriginalInvoiceId(e.target.value)}
                  >
                    <option value="">—</option>
                    {invoiceChoices
                      .filter(
                        (i) =>
                          i.direction === direction &&
                          (i.document_type === "01" ||
                            i.document_type === "11" ||
                            String(i.id) === originalInvoiceId)
                      )
                      .map((i) => (
                        <option key={i.id} value={i.id}>
                          {i.number} · {i.issue_date} · {i.total_amount}
                        </option>
                      ))}
                  </select>
                  {origNumber ? (
                    <div className="muted">
                      {t("scan.orig_ref")}: {origNumber}
                    </div>
                  ) : null}
                </div>
              )}
            </div>
          </section>

          <section className="card invoice-section">
            <h2 className="invoice-section-title">{t("scan.counterpart")}</h2>
            <div className="form-grid">
              <div className="field">
                <label className="label">{t("scan.ocr_name")}</label>
                <input
                  className="input"
                  value={ocrName}
                  onChange={(e) => setOcrName(e.target.value)}
                />
              </div>
              <div className="field">
                <label className="label">{t("scan.eik")}</label>
                <input
                  className="input"
                  value={ocrEik}
                  onChange={(e) => setOcrEik(e.target.value)}
                />
              </div>
              <div className="field">
                <label className="label">{t("scan.vat_no")}</label>
                <input
                  className="input"
                  value={ocrVat}
                  onChange={(e) => setOcrVat(e.target.value)}
                />
              </div>
              <div className="field">
                <label className="label">{t("scan.pick_existing")}</label>
                <select
                  className="select"
                  value={counterpartId}
                  onChange={(e) => setCounterpartId(e.target.value)}
                >
                  <option value="">{t("invoices.pick_counterpart")}</option>
                  {counterparts.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                      {c.eik ? ` (${c.eik})` : ""}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            {found ? (
              <div className="flash-ok">
                {t("scan.found")}: {found.name}
              </div>
            ) : (
              <div className="flash-err" style={{ display: "block" }}>
                <div>{t("scan.not_found")}</div>
                <button className="btn btn-primary btn-sm" onClick={createCounterpart}>
                  {direction === "in" ? t("scan.add_supplier") : t("scan.add_customer")}
                </button>
              </div>
            )}
            {vies && (vies.valid === true || vies.valid === 1) && (
              <div className="flash-ok">
                {t("scan.vies_ok")}
                {vies.name ? ` — ${vies.name}` : ""}
              </div>
            )}
            {vies && vies.valid !== true && vies.valid !== 1 && (
              <div className="flash-err">{t("scan.vies_fail")}</div>
            )}
          </section>

          <section className="card invoice-section">
            <h2 className="invoice-section-title">{t("scan.lines")}</h2>
            <div className="lines-wrap">
              <table className="table lines-table">
                <thead>
                  <tr>
                    <th>{t("scan.scanned_name")}</th>
                    <th>{t("scan.our_product")}</th>
                    <th>{t("scan.qty")}</th>
                    <th>{t("scan.price")}</th>
                    <th>{t("scan.vat_rate")}</th>
                    <th>{t("scan.line_total")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {priced.map((l, i) => {
                    const mapped = l.product_id
                      ? productById.get(l.product_id)
                      : undefined;
                    return (
                      <tr key={i}>
                        <td>
                          <input
                            className="input"
                            value={l.description}
                            onChange={(e) =>
                              setLine(i, { description: e.target.value })
                            }
                          />
                          {l.seller_code ? (
                            <div className="muted">
                              {t("scan.seller_code")}: {l.seller_code}
                            </div>
                          ) : null}
                        </td>
                        <td style={{ minWidth: 220 }}>
                          <button
                            type="button"
                            className={`btn picker-trigger${mapped ? "" : " picker-empty"}`}
                            onClick={() => openProductPicker(i)}
                          >
                            {mapped
                              ? `${mapped.code ? `${mapped.code} — ` : ""}${mapped.name}`
                              : t("scan.pick_product")}
                          </button>
                          {mapped && (
                            <button
                              type="button"
                              className="btn btn-ghost btn-sm"
                              onClick={() => clearProduct(i)}
                            >
                              {t("scan.clear_product")}
                            </button>
                          )}
                        </td>
                        <td>
                          <input
                            className="input"
                            value={l.quantity}
                            onChange={(e) =>
                              setLine(i, { quantity: e.target.value })
                            }
                          />
                        </td>
                        <td>
                          <input
                            className="input"
                            value={l.unit_price}
                            onChange={(e) =>
                              setLine(i, { unit_price: e.target.value })
                            }
                          />
                        </td>
                        <td>
                          <select
                            className="select"
                            value={l.vat_rate}
                            onChange={(e) =>
                              setLine(i, { vat_rate: e.target.value })
                            }
                          >
                            {VAT_RATES.map((r) => (
                              <option key={r} value={r}>
                                {r}%
                              </option>
                            ))}
                          </select>
                        </td>
                        <td className="num">
                          {l.total_amount} {currency}
                        </td>
                        <td>
                          <IconButton
                            icon="delete"
                            title={t("common.delete")}
                            danger
                            onClick={() =>
                              setLines((prev) =>
                                prev.filter((_, idx) => idx !== i)
                              )
                            }
                          />
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <button
              className="btn"
              type="button"
              onClick={() => setLines((prev) => [...prev, emptyLine()])}
            >
              + {t("invoices.add_line")}
            </button>
            <p className="muted" style={{ marginTop: 12 }}>
              {t("invoices.total")}: {totals.total} {currency}
            </p>
          </section>

          <div className="form-actions">
            <button className="btn btn-primary" onClick={saveDraft}>
              {t("scan.save_draft")}
            </button>
          </div>

          {itemOpenFor !== null && (
            <div className="modal-backdrop" onClick={closeProductPicker}>
              <div
                className="card modal picker-modal"
                onClick={(e) => e.stopPropagation()}
              >
                <h2 className="modal-title">{t("scan.our_product")}</h2>
                <p className="muted" style={{ marginTop: -8 }}>
                  {t("scan.pick_hint")}
                  {lines[itemOpenFor]?.description
                    ? ` — ${lines[itemOpenFor].description}`
                    : ""}
                </p>
                {!addingProduct ? (
                  <>
                    <input
                      className="input"
                      autoFocus
                      value={itemQuery}
                      onChange={(e) => setItemQuery(e.target.value)}
                      placeholder={t("invoices.search")}
                    />
                    <div className="picker-list">
                      {filteredProducts.map((p) => (
                        <button
                          type="button"
                          key={p.id}
                          className="picker-item"
                          onClick={() => pickProduct(itemOpenFor, p)}
                        >
                          <b>
                            {p.code ? `${p.code} — ` : ""}
                            {p.name}
                          </b>
                          <span className="muted">
                            {p.price} · ДДС {p.vat_rate}%
                          </span>
                        </button>
                      ))}
                      {filteredProducts.length === 0 && (
                        <div className="muted">{t("common.empty")}</div>
                      )}
                    </div>
                    <div className="form-actions" style={{ marginTop: 12 }}>
                      <button
                        type="button"
                        className="btn btn-primary"
                        onClick={() => {
                          setAddingProduct(true);
                          setNewName(
                            itemQuery.trim() ||
                              lines[itemOpenFor]?.description ||
                              ""
                          );
                          setCreateError("");
                        }}
                      >
                        + {t("scan.add_product")}
                      </button>
                    </div>
                  </>
                ) : (
                  <div>
                    <p className="muted">{t("scan.add_product_hint")}</p>
                    {createError && (
                      <div className="error-text" style={{ marginBottom: 8 }}>
                        {createError}
                      </div>
                    )}
                    <div className="field">
                      <label className="label">{t("products.name")}</label>
                      <input
                        className="input"
                        autoFocus
                        value={newName}
                        onChange={(e) => setNewName(e.target.value)}
                      />
                    </div>
                    <div className="field">
                      <label className="label">{t("products.code")}</label>
                      <input
                        className="input"
                        value={newCode}
                        onChange={(e) => setNewCode(e.target.value)}
                      />
                    </div>
                    <div className="form-actions">
                      <button
                        type="button"
                        className="btn"
                        onClick={() => setAddingProduct(false)}
                        disabled={creating}
                      >
                        {t("common.back")}
                      </button>
                      <button
                        type="button"
                        className="btn btn-primary"
                        onClick={createProduct}
                        disabled={creating}
                      >
                        {creating ? t("scan.creating") : t("common.create")}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </div>
          )}
        </>

  );
}
