"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { UnitPicker } from "../../components/UnitPicker";
import { Cn8Item, Cn8Picker } from "../../components/Cn8Picker";
import { useI18n } from "../../components/I18nProvider";
import { api, fetchAll, ListResponse } from "../../lib/api";
import { UnitOfMeasure, unitSymbol } from "../../lib/units";
import { IconButton } from "../../components/IconButton";
import { Pagination } from "../../components/Pagination";

const PAGE_SIZE = 50;

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
  product_type: string;
  cn8_code: string;
  cn8_name: string;
  expense_account_id: number;
  auto_write_off: number;
}

interface Nomenclature {
  code: string;
  name: string;
}

interface Cn8Suggestion {
  cn8_code: string;
  cn8_name: string;
  unit: string;
  product_type: string;
  is_service: boolean;
  source: string;
  candidates: Cn8Item[];
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
  product_type: "",
  cn8_code: "",
  cn8_name: "",
  expense_account_id: 0,
  auto_write_off: false,
};

type ProductForm = typeof emptyForm;

function accountId(accounts: Account[], number: string): number {
  return accounts.find((a) => a.number === number)?.id ?? 0;
}

function withProductType(form: ProductForm, type: string, accounts: Account[]): ProductForm {
  const next = { ...form, product_type: type };
  if (type === "10") {
    next.is_service = false;
    next.is_inventory = true;
    if (form.product_type !== "10" || !next.inventory_account_id) {
      next.inventory_account_id = accountId(accounts, "302") || next.inventory_account_id;
    }
    if (form.product_type !== "10" || !next.revenue_account_id) {
      next.revenue_account_id = accountId(accounts, "706") || next.revenue_account_id;
    }
    if (form.product_type !== "10" || !next.expense_account_id) {
      next.expense_account_id = accountId(accounts, "601") || next.expense_account_id;
    }
  } else {
    next.auto_write_off = false;
    next.expense_account_id = 0;
    if (type === "30" && form.product_type === "10") {
      next.inventory_account_id = accountId(accounts, "304") || next.inventory_account_id;
      next.revenue_account_id = accountId(accounts, "702") || next.revenue_account_id;
    }
  }
  return next;
}

function accLabel(a: Account): string {
  return `${a.number} ${a.name}`;
}

function ProductsInner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<Product[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [units, setUnits] = useState<UnitOfMeasure[]>([]);
  const [productTypes, setProductTypes] = useState<Nomenclature[]>([]);
  const [cn8Candidates, setCn8Candidates] = useState<Cn8Item[]>([]);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiNote, setAiNote] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Product | null>(null);
  const [form, setForm] = useState(emptyForm);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [p, a, u, types] = await Promise.all([
        fetchAll<Product>("/v1/products"),
        fetchAll<Account>("/v1/accounts"),
        api.get<ListResponse<UnitOfMeasure>>("/v1/units"),
        api.get<ListResponse<Nomenclature>>("/v1/saft/nomenclatures", { kind: "product_types" }),
      ]);
      setRows(p);
      setAccounts(a);
      setUnits(u.items ?? []);
      setProductTypes(types.items ?? []);
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
      `${p.name} ${p.code} ${p.unit} ${p.description} ${p.cn8_code} ${p.cn8_name}`.toLowerCase().includes(s)
    );
  }, [rows, q]);

  const pageCount = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const page = Math.min(Math.floor(offset / PAGE_SIZE), pageCount - 1);
  const start = page * PAGE_SIZE;
  const paged = filtered.slice(start, start + PAGE_SIZE);

  const openCreate = () => {
    setForm(emptyForm);
    setEditing(null);
    setFormError("");
    setCn8Candidates([]);
    setAiNote("");
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
      product_type: p.product_type ?? "",
      cn8_code: p.cn8_code ?? "",
      cn8_name: p.cn8_name ?? "",
      expense_account_id: Number(p.expense_account_id) || 0,
      auto_write_off: Number(p.auto_write_off) === 1,
    });
    setEditing(p);
    setFormError("");
    setCn8Candidates([]);
    setAiNote("");
    setModalOpen(true);
  };

  const suggestCn8 = async () => {
    setAiBusy(true);
    setFormError("");
    setAiNote("");
    try {
      const result = await api.post<Cn8Suggestion>("/v1/products/cn8-suggest", {
        name: form.name,
        description: form.description,
        unit: form.unit,
        product_type: form.product_type,
        is_service: form.is_service,
      });
      let next: ProductForm = {
        ...form,
        cn8_code: result.cn8_code,
        cn8_name: result.cn8_name,
        unit: result.unit || form.unit,
        is_service: result.is_service,
        is_inventory: result.is_service
          ? false
          : form.is_inventory || Boolean(result.product_type),
      };
      if (result.is_service) {
        next = {
          ...next,
          is_service: true,
          is_inventory: false,
          product_type: "",
          expense_account_id: 0,
          auto_write_off: false,
        };
      } else {
        next = withProductType(next, result.product_type || form.product_type, accounts);
      }
      setForm(next);
      setCn8Candidates(result.candidates ?? []);
      setAiNote(`${result.cn8_code} — ${t(result.source === "ai" ? "products.cn8_ai_source" : "products.cn8_catalog_source")}`);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setAiBusy(false);
    }
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
        product_type: form.product_type,
        cn8_code: form.cn8_code,
        cn8_name: form.cn8_name,
        expense_account_id: form.expense_account_id,
        auto_write_off: form.auto_write_off,
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
  const productTypeName = (code: string) =>
    productTypes.find((x) => x.code === code)?.name ?? code;

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
                <th>{t("products.cn8_code")}</th>
                <th>{t("products.price")}</th>
                <th>{t("products.purchase_account")}</th>
                <th>{t("products.expense_account")}</th>
                <th>{t("products.revenue_account")}</th>
                <th>{t("products.stock_quantity")}</th>
                <th>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {paged.map((p) => (
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
                    {p.product_type && (
                      <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>
                        {productTypeName(p.product_type)}
                        {Number(p.auto_write_off) === 1 ? ` · ${t("products.auto_write_off")}` : ""}
                      </div>
                    )}
                  </td>
                  <td>{p.code}</td>
                  <td>{unitSymbol(p.unit)}</td>
                  <td title={p.cn8_name}>{p.cn8_code || "—"}</td>
                  <td>{p.price}</td>
                  <td>{accountName(p.inventory_account_id)}</td>
                  <td>{accountName(p.expense_account_id)}</td>
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
        <Pagination
          total={filtered.length}
          limit={PAGE_SIZE}
          offset={start}
          onPageChange={setOffset}
        />
      </div>

      {modalOpen && (
        <div className="modal-backdrop" onClick={() => setModalOpen(false)}>
          <div
            className="card modal"
            style={{ maxWidth: 900 }}
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
                <div className="field">
                  <label className="label">{t("products.product_type")}</label>
                  <select
                    className="select"
                    value={form.product_type}
                    disabled={form.is_service}
                    onChange={(e) =>
                      setForm(withProductType(form, e.target.value, accounts))
                    }
                  >
                    <option value="">{t("products.product_type_pick")}</option>
                    {productTypes.map((type) => (
                      <option key={type.code} value={type.code}>
                        {type.code} — {type.name}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field" style={{ gridColumn: "1 / -1" }}>
                  <button
                    type="button"
                    className="btn"
                    disabled={aiBusy || !form.name.trim()}
                    onClick={suggestCn8}
                  >
                    {aiBusy ? t("products.cn8_ai_busy") : t("products.cn8_ai_btn")}
                  </button>
                  {aiNote && <span className="muted" style={{ marginLeft: 10 }}>{aiNote}</span>}
                </div>
                <div className="field" style={{ gridColumn: "1 / -1" }}>
                  <label className="label">{t("products.cn8_code")}</label>
                  <Cn8Picker
                    code={form.cn8_code}
                    name={form.cn8_name}
                    candidates={cn8Candidates}
                    onSelect={(cn8_code, cn8_name) =>
                      setForm({ ...form, cn8_code, cn8_name })
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
                {form.product_type === "10" && (
                  <>
                    <div className="field">
                      <label className="label">{t("products.expense_account")}</label>
                      <select
                        className="select"
                        value={String(form.expense_account_id)}
                        onChange={(e) =>
                          setForm({
                            ...form,
                            expense_account_id: Number(e.target.value) || 0,
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
                    <div className="field" style={{ gridColumn: "1 / -1" }}>
                      <label className="check-inline">
                        <input
                          type="checkbox"
                          checked={form.auto_write_off}
                          onChange={(e) =>
                            setForm({ ...form, auto_write_off: e.target.checked })
                          }
                        />
                        {t("products.auto_write_off")}
                      </label>
                      <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>
                        {form.auto_write_off
                          ? t("products.auto_write_off_hint")
                          : t("products.manual_write_off_hint")}
                      </div>
                    </div>
                  </>
                )}
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
                          product_type: e.target.checked ? "" : form.product_type,
                          expense_account_id: e.target.checked ? 0 : form.expense_account_id,
                          auto_write_off: e.target.checked ? false : form.auto_write_off,
                          cn8_code: e.target.checked && !form.cn8_code ? "00000000" : form.cn8_code,
                          cn8_name: e.target.checked && !form.cn8_name ? "Услуги" : form.cn8_name,
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
                        setForm({
                          ...form,
                          is_inventory: e.target.checked,
                          auto_write_off: e.target.checked ? form.auto_write_off : false,
                        })
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
                    <input className="input" value={form.stock_quantity} disabled readOnly />
                    <div className="muted" style={{ fontSize: 12 }}>
                      {t("products.stock_hint")} {editing?.stock_value ?? "0.00"}
                    </div>
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
