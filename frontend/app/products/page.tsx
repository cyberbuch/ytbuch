"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { api, fetchAll, ListResponse } from "../../lib/api";
import { UnitOfMeasure, unitSymbol } from "../../lib/units";
import { IconButton } from "../../components/IconButton";
import { Pagination } from "../../components/Pagination";
import {
  ProductForm,
  ProductFormValues,
  emptyProductForm,
  productPayload,
  productAccountLabel,
} from "../../components/ProductForm";

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

function ProductsInner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<Product[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [units, setUnits] = useState<UnitOfMeasure[]>([]);
  const [productTypes, setProductTypes] = useState<Nomenclature[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Product | null>(null);
  const [initial, setInitial] = useState<ProductFormValues>(emptyProductForm);
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
    setInitial(emptyProductForm);
    setEditing(null);
    setModalOpen(true);
  };

  const openEdit = (p: Product) => {
    setInitial({
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
    setModalOpen(true);
  };

  const submit = async (values: ProductFormValues) => {
    const payload = productPayload(values);
    if (editing) {
      await api.patch(`/v1/products/${editing.id}`, payload);
    } else {
      await api.post("/v1/products", payload);
    }
    setModalOpen(false);
    await load();
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
    return a ? productAccountLabel(a) : "";
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
            <ProductForm
              key={editing?.id ?? "new"}
              accounts={accounts}
              units={units}
              initial={initial}
              stockValue={editing?.stock_value}
              onSubmit={submit}
              onCancel={() => setModalOpen(false)}
            />
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
