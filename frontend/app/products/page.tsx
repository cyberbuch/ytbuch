"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { UnitPicker } from "../../components/UnitPicker";
import { useI18n } from "../../components/I18nProvider";
import { api, ListResponse } from "../../lib/api";
import { UnitOfMeasure } from "../../lib/units";
import { IconButton } from "../../components/IconButton";

interface Account {
  id: number;
  number: string;
  name: string;
}

interface Product {
  id: number;
  name: string;
  code: string;
  description: string;
  unit: string;
  price: string;
  cost_price: string;
  vat_rate: string;
  is_service: number;
  is_inventory: number;
  inventory_account_id: number;
  revenue_account_id: number;
  stock_quantity: string;
  stock_value: string;
  is_active: number;
}

const emptyForm = {
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
  stock_quantity: "0",
  is_active: true,
};

function accLabel(a: Account): string {
  return `${a.number} ${a.name}`;
}

function ProductsInner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<Product[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [units, setUnits] = useState<UnitOfMeasure[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Product | null>(null);
  const [form, setForm] = useState(emptyForm);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");
  const [q, setQ] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [p, a, u] = await Promise.all([
        api.get<ListResponse<Product>>("/v1/products"),
        api.get<ListResponse<Account>>("/v1/accounts"),
        api.get<ListResponse<UnitOfMeasure>>("/v1/units"),
      ]);
      setRows(p.items ?? []);
      setAccounts(a.items ?? []);
      setUnits(u.items ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((p) =>
      `${p.name} ${p.code} ${p.unit} ${p.description}`.toLowerCase().includes(s)
    );
  }, [rows, q]);

  const openCreate = () => {
    setForm(emptyForm);
    setEditing(null);
    setFormError("");
    setModalOpen(true);
  };

  const openEdit = (p: Product) => {
    setForm({
      name: p.name ?? "",
      code: p.code ?? "",
      description: p.description ?? "",
      unit: p.unit || "C62",
      price: p.price || "0",
      cost_price: p.cost_price || "0",
      vat_rate: p.vat_rate || "20",
      is_service: Number(p.is_service) === 1,
      is_inventory: Number(p.is_inventory) === 1,
      inventory_account_id: Number(p.inventory_account_id) || 0,
      revenue_account_id: Number(p.revenue_account_id) || 0,
      stock_quantity: p.stock_quantity || "0",
      is_active: p.is_active === undefined ? true : Number(p.is_active) === 1,
    });
    setEditing(p);
    setFormError("");
    setModalOpen(true);
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError("");
    try {
      const payload = {
        name: form.name,
        code: form.code,
        description: form.description,
        unit: form.unit,
        price: form.price,
        cost_price: form.cost_price,
        vat_rate: form.vat_rate,
        is_service: form.is_service,
        is_inventory: form.is_service ? false : form.is_inventory,
        inventory_account_id: form.inventory_account_id,
        revenue_account_id: form.revenue_account_id,
        stock_quantity: form.stock_quantity,
        is_active: form.is_active,
      };
      if (editing) {
        await api.patch(`/v1/products/${editing.id}`, payload);
      } else {
        await api.post("/v1/products", payload);
      }
      setModalOpen(false);
      await load();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (p: Product) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/products/${p.id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const accountName = (id: number) => {
    const a = accounts.find((x) => x.id === id);
    return a ? accLabel(a) : "";
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("products.title")}</h1>
        <button className="btn btn-primary" onClick={openCreate}>
          {t("common.create")}
        </button>
      </div>
      {error && <div className="error-text">{error}</div>}
      <div className="card" style={{ marginBottom: 12 }}>
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t("common.search")}
        />
      </div>
      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : filtered.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("products.name")}</th>
                <th>{t("products.code")}</th>
                <th>{t("products.unit")}</th>
                <th>{t("products.price")}</th>
                <th>{t("products.purchase_account")}</th>
                <th>{t("products.revenue_account")}</th>
                <th>{t("products.stock_quantity")}</th>
                <th>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((p) => (
                <tr key={p.id}>
                  <td>
                    {p.name}
                    {Number(p.is_inventory) === 1 ? (
                      <span className="badge badge-warning" style={{ marginLeft: 6 }}>
                        {t("products.is_inventory")}
                      </span>
                    ) : Number(p.is_service) === 1 ? (
                      <span className="badge badge-muted" style={{ marginLeft: 6 }}>
                        {t("products.is_service")}
                      </span>
                    ) : null}
                  </td>
                  <td>{p.code}</td>
                  <td>{p.unit}</td>
                  <td>{p.price}</td>
                  <td>{accountName(p.inventory_account_id)}</td>
                  <td>{accountName(p.revenue_account_id)}</td>
                  <td>{Number(p.is_inventory) === 1 ? p.stock_quantity : "—"}</td>
                  <td>
                    <div className="icon-actions">
                      <IconButton
                        icon="edit"
                        title={t("common.edit")}
                        onClick={() => openEdit(p)}
                      />
                      <IconButton
                        icon="delete"
                        title={t("common.delete")}
                        danger
                        onClick={() => handleDelete(p)}
                      />
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {modalOpen && (
        <div className="modal-backdrop" onClick={() => setModalOpen(false)}>
          <div
            className="card modal"
            style={{ maxWidth: 720 }}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">
              {editing ? t("common.edit") : t("common.create")}
            </h2>
            <form onSubmit={handleSave}>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("products.name")} *</label>
                  <input
                    className="input"
                    required
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
                    <option value="0">{t("products.account_default")} 702</option>
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
                    <option value="0">
                      {t("products.account_default")}{" "}
                      {form.is_inventory ? "304" : "602"}
                    </option>
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
                {form.is_inventory && (
                  <div className="field">
                    <label className="label">{t("products.stock_quantity")}</label>
                    <input
                      className="input"
                      value={form.stock_quantity}
                      disabled={Boolean(editing)}
                      onChange={(e) =>
                        setForm({ ...form, stock_quantity: e.target.value })
                      }
                    />
                    {editing && (
                      <div className="muted" style={{ fontSize: 12 }}>
                        {t("products.stock_hint")} {editing.stock_value}
                      </div>
                    )}
                  </div>
                )}
              </div>
              {formError && <div className="error-text">{formError}</div>}
              <div className="form-actions">
                <button
                  type="button"
                  className="btn"
                  onClick={() => setModalOpen(false)}
                >
                  {t("common.cancel")}
                </button>
                <button className="btn btn-primary" disabled={saving}>
                  {t("common.save")}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}

export default function ProductsPage() {
  return (
    <RequireAuth>
      <ProductsInner />
    </RequireAuth>
  );
}
