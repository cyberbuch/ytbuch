"use client";

import { unitLabel, UnitOfMeasure } from "../lib/units";
import { InvoiceFormCounterpart, InvoiceFormProduct } from "./invoiceFormTypes";

export function InvoiceFormPickers({
  t,
  cpOpen,
  setCpOpen,
  cpQuery,
  setCpQuery,
  filteredCp,
  setCounterpartId,
  itemOpenFor,
  setItemOpenFor,
  itemQuery,
  setItemQuery,
  filteredProducts,
  pickProduct,
  units,
}: {
  t: (k: string) => string;
  cpOpen: boolean;
  setCpOpen: (v: boolean) => void;
  cpQuery: string;
  setCpQuery: (v: string) => void;
  filteredCp: InvoiceFormCounterpart[];
  setCounterpartId: (v: string) => void;
  itemOpenFor: number | null;
  setItemOpenFor: (v: number | null) => void;
  itemQuery: string;
  setItemQuery: (v: string) => void;
  filteredProducts: InvoiceFormProduct[];
  pickProduct: (i: number, p: InvoiceFormProduct) => void;
  units: UnitOfMeasure[];
}) {
  return (
    <>
      {cpOpen && (
        <div className="modal-backdrop" onClick={() => setCpOpen(false)}>
          <div className="card modal picker-modal" onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">{t("invoices.pick_counterpart")}</h2>
            <input
              className="input"
              autoFocus
              value={cpQuery}
              onChange={(e) => setCpQuery(e.target.value)}
              placeholder={t("invoices.search")}
            />
            <div className="picker-list">
              {filteredCp.map((c) => (
                <button
                  type="button"
                  key={c.id}
                  className="picker-item"
                  onClick={() => {
                    setCounterpartId(String(c.id));
                    setCpOpen(false);
                    setCpQuery("");
                  }}
                >
                  <b>{c.name}</b>
                  <span className="muted">
                    {c.eik} {c.vat_number}
                  </span>
                </button>
              ))}
              {filteredCp.length === 0 && (
                <div className="muted">{t("common.empty")}</div>
              )}
            </div>
          </div>
        </div>
      )}

      {itemOpenFor !== null && (
        <div className="modal-backdrop" onClick={() => setItemOpenFor(null)}>
          <div className="card modal picker-modal" onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">{t("invoices.pick_item")}</h2>
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
                    {p.price} / {unitLabel(p.unit || "C62", units)} · ДДС {p.vat_rate}%
                  </span>
                </button>
              ))}
              {filteredProducts.length === 0 && (
                <div className="muted">{t("common.empty")}</div>
              )}
            </div>
          </div>
        </div>
      )}

    </>
  );
}
