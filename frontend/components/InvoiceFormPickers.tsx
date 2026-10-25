"use client";

import { useState } from "react";
import { api } from "../lib/api";
import { unitLabel, UnitOfMeasure } from "../lib/units";
import {
  InvoiceFormAccount,
  InvoiceFormCounterpart,
  InvoiceFormProduct,
} from "./invoiceFormTypes";
import {
  ProductForm,
  ProductFormValues,
  emptyProductForm,
  productPayload,
} from "./ProductForm";

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
  accounts,
  onProductCreated,
  initialDescription,
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
  accounts: InvoiceFormAccount[];
  onProductCreated: (lineIdx: number, p: InvoiceFormProduct) => void;
  initialDescription?: string;
}) {
  const [addingProduct, setAddingProduct] = useState(false);
  const [initial, setInitial] = useState<ProductFormValues>(emptyProductForm);

  const closeItemModal = () => {
    setItemOpenFor(null);
    setAddingProduct(false);
  };

  const openAddForm = () => {
    setInitial({
      ...emptyProductForm,
      name: itemQuery.trim() || initialDescription?.trim() || "",
    });
    setAddingProduct(true);
  };

  const submitProduct = async (values: ProductFormValues) => {
    if (itemOpenFor === null) return;
    const created = await api.post<InvoiceFormProduct>("/v1/products", productPayload(values));
    setAddingProduct(false);
    onProductCreated(itemOpenFor, created);
  };

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
        <div className="modal-backdrop" onClick={closeItemModal}>
          <div
            className="card modal picker-modal"
            style={addingProduct ? { maxWidth: 900 } : undefined}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">
              {addingProduct ? t("common.create") : t("invoices.pick_item")}
            </h2>
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
                        {p.price} / {unitLabel(p.unit || "C62", units)} · ДДС {p.vat_rate}%
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
                    className="btn"
                    onClick={closeItemModal}
                  >
                    {t("common.cancel")}
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={openAddForm}
                  >
                    + {t("scan.add_product")}
                  </button>
                </div>
              </>
            ) : (
              <ProductForm
                accounts={accounts}
                units={units}
                initial={initial}
                onSubmit={submitProduct}
                onCancel={() => setAddingProduct(false)}
                cancelLabel={t("common.back")}
              />
            )}
          </div>
        </div>
      )}
    </>
  );
}
