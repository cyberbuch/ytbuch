"use client";

import { FormEvent, useState } from "react";
import { api } from "../lib/api";
import { unitLabel, UnitOfMeasure } from "../lib/units";
import { UnitPicker } from "./UnitPicker";
import {
  InvoiceFormAccount,
  InvoiceFormCounterpart,
  InvoiceFormProduct,
} from "./invoiceFormTypes";

const emptyNewProduct = {
  name: "",
  code: "",
  description: "",
  unit: "C62",
  price: "0",
  cost_price: "0",
  vat_rate: "20",
  is_service: false,
  is_inventory: false,
  inventory_account_id: 0,
  revenue_account_id: 0,
  is_active: true,
};

function accLabel(a: InvoiceFormAccount): string {
  return `${a.number} ${a.name}`;
}

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
  const [form, setForm] = useState(emptyNewProduct);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  const closeItemModal = () => {
    setItemOpenFor(null);
    setAddingProduct(false);
    setFormError("");
  };

  const openAddForm = () => {
    setForm({
      ...emptyNewProduct,
      name: itemQuery.trim() || initialDescription?.trim() || "",
    });
    setFormError("");
    setAddingProduct(true);
  };

  const handleCreateProduct = async (e: FormEvent) => {
    e.preventDefault();
    if (itemOpenFor === null) return;
    if (!form.name.trim()) {
      setFormError(t("scan.need_product_name"));
      return;
    }
    setSaving(true);
    setFormError("");
    try {
      const created = await api.post<InvoiceFormProduct>("/v1/products", {
        name: form.name.trim(),
        code: form.code.trim(),
        description: form.description,
        unit: form.unit || "C62",
        price: form.price || "0",
        cost_price: form.cost_price || "0",
        vat_rate: form.vat_rate || "20",
        is_service: form.is_service,
        is_inventory: form.is_service ? false : form.is_inventory,
        inventory_account_id: form.inventory_account_id,
        revenue_account_id: form.revenue_account_id,
        stock_quantity: "0",
        is_active: form.is_active,
      });
      setAddingProduct(false);
      onProductCreated(itemOpenFor, created);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
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
            style={addingProduct ? { maxWidth: 720 } : undefined}
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
              <form onSubmit={handleCreateProduct}>
                <div className="form-grid">
                  <div className="field">
                    <label className="label">{t("products.name")} *</label>
                    <input
                      className="input"
                      required
                      autoFocus
                      value={form.name}
                      onChange={(e) => setForm({ ...form, name: e.target.value })}
                    />
                  </div>
                  <div className="field">
                    <label className="label">{t("products.code")}</label>
                    <input
                      className="input"
                      value={form.code}
                      onChange={(e) => setForm({ ...form, code: e.target.value })}
                      placeholder={t("products.code_hint")}
                    />
                  </div>
                  <div className="field" style={{ gridColumn: "1 / -1" }}>
                    <label className="label">{t("products.description")}</label>
                    <textarea
                      className="input"
                      rows={2}
                      value={form.description}
                      onChange={(e) =>
                        setForm({ ...form, description: e.target.value })
                      }
                    />
                  </div>
                  <div className="field" style={{ gridColumn: "1 / -1" }}>
                    <label className="label">{t("products.unit")}</label>
                    <UnitPicker
                      value={form.unit}
                      onChange={(unit) => setForm({ ...form, unit })}
                      units={units}
                    />
                  </div>
                  <div className="field">
                    <label className="label">{t("products.price")}</label>
                    <input
                      className="input"
                      value={form.price}
                      onChange={(e) => setForm({ ...form, price: e.target.value })}
                    />
                  </div>
                  <div className="field">
                    <label className="label">{t("products.cost_price")}</label>
                    <input
                      className="input"
                      value={form.cost_price}
                      onChange={(e) =>
                        setForm({ ...form, cost_price: e.target.value })
                      }
                    />
                  </div>
                  <div className="field">
                    <label className="label">{t("products.vat_rate")}</label>
                    <input
                      className="input"
                      value={form.vat_rate}
                      onChange={(e) =>
                        setForm({ ...form, vat_rate: e.target.value })
                      }
                    />
                  </div>
                  <div className="field">
                    <label className="label">{t("products.revenue_account")}</label>
                    <select
                      className="select"
                      value={String(form.revenue_account_id)}
                      onChange={(e) =>
                        setForm({
                          ...form,
                          revenue_account_id: Number(e.target.value) || 0,
                        })
                      }
                    >
                      <option value="0">{t("products.account_required")}</option>
                      {accounts.map((a) => (
                        <option key={a.id} value={a.id}>
                          {accLabel(a)}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="field">
                    <label className="label">{t("products.purchase_account")}</label>
                    <select
                      className="select"
                      value={String(form.inventory_account_id)}
                      onChange={(e) =>
                        setForm({
                          ...form,
                          inventory_account_id: Number(e.target.value) || 0,
                        })
                      }
                    >
                      <option value="0">{t("products.account_required")}</option>
                      {accounts.map((a) => (
                        <option key={a.id} value={a.id}>
                          {accLabel(a)}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="field">
                    <label className="check-inline">
                      <input
                        type="checkbox"
                        checked={form.is_service}
                        onChange={(e) =>
                          setForm({
                            ...form,
                            is_service: e.target.checked,
                            is_inventory: e.target.checked
                              ? false
                              : form.is_inventory,
                          })
                        }
                      />
                      {t("products.is_service")}
                    </label>
                  </div>
                  <div className="field">
                    <label className="check-inline">
                      <input
                        type="checkbox"
                        checked={form.is_inventory}
                        disabled={form.is_service}
                        onChange={(e) =>
                          setForm({ ...form, is_inventory: e.target.checked })
                        }
                      />
                      {t("products.is_inventory")}
                    </label>
                  </div>
                  <div className="field">
                    <label className="check-inline">
                      <input
                        type="checkbox"
                        checked={form.is_active}
                        onChange={(e) =>
                          setForm({ ...form, is_active: e.target.checked })
                        }
                      />
                      {t("products.is_active")}
                    </label>
                  </div>
                </div>
                {formError && <div className="error-text">{formError}</div>}
                <div className="form-actions" style={{ marginTop: 12 }}>
                  <button
                    type="button"
                    className="btn"
                    disabled={saving}
                    onClick={() => setAddingProduct(false)}
                  >
                    {t("common.back")}
                  </button>
                  <button className="btn btn-primary" disabled={saving}>
                    {t("common.save")}
                  </button>
                </div>
              </form>
            )}
          </div>
        </div>
      )}
    </>
  );
}
