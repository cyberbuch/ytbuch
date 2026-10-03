"use client";

// Производство — рецепти (BOM) и поръчки. Потвърждението изписва материали
// и заприходява готова продукция през склада (WAC/FIFO/LIFO) + запис 611.

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { api, ListResponse } from "../../lib/api";
import { formatBgDate, todayIso } from "../../lib/dates";
import { fetchCompanyLocations, CompanyLocation } from "../../lib/locations";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { IconButton } from "../../components/IconButton";

type Tab = "recipes" | "orders";

interface Product {
  id: number;
  name: string;
  code: string;
  unit: string;
  is_inventory: number;
  stock_quantity: string;
}

interface RecipeLine {
  product_id: number;
  quantity: string;
  unit: string;
  wastage_percent: string;
}

interface Recipe {
  id: number;
  code: string;
  name: string;
  output_product_id: number;
  output_quantity: string;
  output_unit: string;
  notes: string;
  lines?: RecipeLine[];
}

interface OrderLine {
  id: number;
  product_id: number;
  product_name: string;
  quantity: string;
  unit: string;
  unit_cost: string;
  amount: string;
}

interface Order {
  id: number;
  number: string;
  recipe_id: number;
  output_product_id: number;
  quantity: string;
  location_id: number;
  order_date: string;
  status: string;
  notes: string;
  material_cost: string;
  lines?: OrderLine[];
}

function money(raw: string | undefined): string {
  const n = Number(String(raw ?? "").replace(",", "."));
  if (!Number.isFinite(n)) return raw || "0";
  return n.toLocaleString("bg-BG", { minimumFractionDigits: 2, maximumFractionDigits: 4 });
}

function pname(products: Product[], id: number): string {
  const p = products.find((x) => x.id === id);
  return p ? `${p.code ? p.code + " " : ""}${p.name}` : `#${id}`;
}

function ProductionInner() {
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>("orders");
  const [recipes, setRecipes] = useState<Recipe[]>([]);
  const [orders, setOrders] = useState<Order[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [recipeModal, setRecipeModal] = useState(false);
  const [orderModal, setOrderModal] = useState(false);
  const [editing, setEditing] = useState<Recipe | null>(null);
  const [detail, setDetail] = useState<Order | null>(null);
  const [saving, setSaving] = useState(false);
  const [rform, setRform] = useState({
    code: "",
    name: "",
    output_product_id: 0,
    output_quantity: "1",
    output_unit: "",
    notes: "",
    lines: [{ product_id: 0, quantity: "1", unit: "", wastage_percent: "0" }] as RecipeLine[],
  });
  const [oform, setOform] = useState({
    recipe_id: 0,
    quantity: "1",
    location_id: 0,
    order_date: todayIso(),
    notes: "",
  });

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [r, o, p, loc] = await Promise.all([
        api.get<ListResponse<Recipe>>("/v1/recipes"),
        api.get<ListResponse<Order>>("/v1/production-orders"),
        api.get<ListResponse<Product>>("/v1/products"),
        fetchCompanyLocations(),
      ]);
      setRecipes(r.items ?? []);
      setOrders(o.items ?? []);
      setProducts(p.items ?? []);
      setLocations(loc);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const invProducts = useMemo(
    () => products.filter((p) => p.is_inventory),
    [products]
  );

  const openRecipe = async (r?: Recipe) => {
    if (r) {
      const full = await api.get<Recipe>(`/v1/recipes/${r.id}`);
      setEditing(full);
      setRform({
        code: full.code ?? "",
        name: full.name,
        output_product_id: full.output_product_id,
        output_quantity: full.output_quantity || "1",
        output_unit: full.output_unit ?? "",
        notes: full.notes ?? "",
        lines: full.lines?.length
          ? full.lines
          : [{ product_id: 0, quantity: "1", unit: "", wastage_percent: "0" }],
      });
    } else {
      setEditing(null);
      setRform({
        code: "",
        name: "",
        output_product_id: 0,
        output_quantity: "1",
        output_unit: "",
        notes: "",
        lines: [{ product_id: 0, quantity: "1", unit: "", wastage_percent: "0" }],
      });
    }
    setRecipeModal(true);
  };

  const saveRecipe = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError("");
    const payload = {
      ...rform,
      output_product_id: Number(rform.output_product_id),
      lines: rform.lines.filter((l) => l.product_id > 0),
    };
    try {
      if (editing) await api.patch(`/v1/recipes/${editing.id}`, payload);
      else await api.post("/v1/recipes", payload);
      setRecipeModal(false);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const delRecipe = async (id: number) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/recipes/${id}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const saveOrder = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError("");
    try {
      const d = await api.post<Order>("/v1/production-orders", {
        recipe_id: Number(oform.recipe_id),
        quantity: oform.quantity.replace(",", "."),
        location_id: Number(oform.location_id),
        order_date: oform.order_date,
        notes: oform.notes,
      });
      setOrderModal(false);
      await load();
      setDetail(d);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const openOrder = async (id: number) => {
    try {
      const d = await api.get<Order>(`/v1/production-orders/${id}`);
      setDetail(d);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const confirmOrder = async (id: number) => {
    setSaving(true);
    setError("");
    try {
      const d = await api.post<Order>(`/v1/production-orders/${id}/confirm`);
      setDetail(d);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const delOrder = async (id: number) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/production-orders/${id}`);
      setDetail(null);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const locName = (id: number) => locations.find((l) => l.id === id)?.name ?? String(id);

  if (detail) {
    return (
      <div>
        <div className="page-head">
          <h1 className="page-title">
            {t("mfg.order")} {detail.number}
          </h1>
          <div className="icon-actions">
            <button className="btn" onClick={() => setDetail(null)}>{t("common.back")}</button>
            {detail.status === "draft" && (
              <>
                <button className="btn btn-primary" disabled={saving} onClick={() => void confirmOrder(detail.id)}>
                  {t("mfg.confirm")}
                </button>
                <button className="btn btn-danger" onClick={() => void delOrder(detail.id)}>
                  {t("common.delete")}
                </button>
              </>
            )}
          </div>
        </div>
        {error && <div className="error-text">{error}</div>}
        <div className="card content" style={{ marginBottom: 18 }}>
          <div className="muted">
            {t(`mfg.status.${detail.status}`)} · {formatBgDate(detail.order_date)} · {locName(detail.location_id)}
          </div>
          <p>
            {t("mfg.output")}: {pname(products, detail.output_product_id)} × {money(detail.quantity)}
          </p>
          {detail.status === "completed" && (
            <p>
              {t("mfg.material_cost")}: {money(detail.material_cost)}
            </p>
          )}
        </div>
        <div className="card">
          <div className="content table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>{t("mfg.material")}</th>
                  <th className="num">{t("mfg.qty")}</th>
                  <th>{t("mfg.unit")}</th>
                  <th className="num">{t("mfg.unit_cost")}</th>
                  <th className="num">{t("mfg.amount")}</th>
                </tr>
              </thead>
              <tbody>
                {(detail.lines ?? []).map((l) => (
                  <tr key={l.id}>
                    <td>{l.product_name || pname(products, l.product_id)}</td>
                    <td className="num">{money(l.quantity)}</td>
                    <td>{l.unit}</td>
                    <td className="num">{money(l.unit_cost)}</td>
                    <td className="num">{money(l.amount)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("mfg.title")}</h1>
        {tab === "recipes" ? (
          <button className="btn btn-primary" onClick={() => void openRecipe()}>{t("mfg.new_recipe")}</button>
        ) : (
          <button className="btn btn-primary" onClick={() => { setOform({ recipe_id: recipes[0]?.id ?? 0, quantity: "1", location_id: locations[0]?.id ?? 0, order_date: todayIso(), notes: "" }); setOrderModal(true); }}>{t("mfg.new_order")}</button>
        )}
      </div>
      <p className="muted" style={{ margin: "0 0 12px" }}>{t("mfg.hint")}</p>
      <div className="tabs">
        <button type="button" className={`tab${tab === "orders" ? " tab-active" : ""}`} onClick={() => setTab("orders")}>
          {t("mfg.tab.orders")}
        </button>
        <button type="button" className={`tab${tab === "recipes" ? " tab-active" : ""}`} onClick={() => setTab("recipes")}>
          {t("mfg.tab.recipes")}
        </button>
      </div>
      {error && <div className="error-text">{error}</div>}
      {loading && <div className="muted">{t("common.loading")}</div>}

      {tab === "recipes" && !loading && (
        <div className="card">
          <div className="content table-wrap">
            {recipes.length === 0 ? (
              <div className="muted">{t("common.empty")}</div>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("mfg.code")}</th>
                    <th>{t("mfg.recipe")}</th>
                    <th>{t("mfg.output")}</th>
                    <th className="num">{t("mfg.qty")}</th>
                    <th>{t("common.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {recipes.map((r) => (
                    <tr key={r.id}>
                      <td>{r.code}</td>
                      <td>{r.name}</td>
                      <td>{pname(products, r.output_product_id)}</td>
                      <td className="num">{money(r.output_quantity)}</td>
                      <td>
                        <div className="icon-actions">
                          <IconButton icon="edit" title={t("common.edit")} onClick={() => void openRecipe(r)} />
                          <IconButton icon="delete" title={t("common.delete")} danger onClick={() => void delRecipe(r.id)} />
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      )}

      {tab === "orders" && !loading && (
        <div className="card">
          <div className="content table-wrap">
            {orders.length === 0 ? (
              <div className="muted">{t("common.empty")}</div>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("mfg.number")}</th>
                    <th>{t("mfg.date")}</th>
                    <th>{t("mfg.output")}</th>
                    <th className="num">{t("mfg.qty")}</th>
                    <th>{t("mfg.status_col")}</th>
                    <th>{t("common.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {orders.map((o) => (
                    <tr key={o.id}>
                      <td>{o.number}</td>
                      <td>{formatBgDate(o.order_date)}</td>
                      <td>{pname(products, o.output_product_id)}</td>
                      <td className="num">{money(o.quantity)}</td>
                      <td>{t(`mfg.status.${o.status}`)}</td>
                      <td>
                        <div className="icon-actions">
                          <IconButton icon="view" title={t("mfg.open")} onClick={() => void openOrder(o.id)} />
                          {o.status === "draft" && (
                            <IconButton icon="delete" title={t("common.delete")} danger onClick={() => void delOrder(o.id)} />
                          )}
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      )}

      {recipeModal && (
        <div className="modal-backdrop" onClick={() => setRecipeModal(false)}>
          <div className="card modal" style={{ maxWidth: 720 }} onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">{editing ? t("common.edit") : t("mfg.new_recipe")}</h2>
            <form onSubmit={(e) => void saveRecipe(e)}>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("mfg.code")}</label>
                  <input className="input" value={rform.code} onChange={(e) => setRform({ ...rform, code: e.target.value })} />
                </div>
                <div className="field">
                  <label className="label">{t("mfg.recipe")} *</label>
                  <input className="input" required value={rform.name} onChange={(e) => setRform({ ...rform, name: e.target.value })} />
                </div>
                <div className="field">
                  <label className="label">{t("mfg.output")} *</label>
                  <select className="input" required value={rform.output_product_id} onChange={(e) => setRform({ ...rform, output_product_id: Number(e.target.value) })}>
                    <option value={0}>—</option>
                    {invProducts.map((p) => (
                      <option key={p.id} value={p.id}>{pname(products, p.id)}</option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label className="label">{t("mfg.output_qty")}</label>
                  <input className="input" value={rform.output_quantity} onChange={(e) => setRform({ ...rform, output_quantity: e.target.value })} />
                </div>
              </div>
              <h3 className="section-title" style={{ marginTop: 16 }}>{t("mfg.materials")}</h3>
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("mfg.material")}</th>
                    <th>{t("mfg.qty")}</th>
                    <th>{t("mfg.wastage")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {rform.lines.map((line, i) => (
                    <tr key={i}>
                      <td>
                        <select className="input" value={line.product_id} onChange={(e) => {
                          const lines = rform.lines.slice();
                          const p = products.find((x) => x.id === Number(e.target.value));
                          lines[i] = { ...line, product_id: Number(e.target.value), unit: p?.unit ?? line.unit };
                          setRform({ ...rform, lines });
                        }}>
                          <option value={0}>—</option>
                          {invProducts.map((p) => (
                            <option key={p.id} value={p.id}>{pname(products, p.id)}</option>
                          ))}
                        </select>
                      </td>
                      <td>
                        <input className="input" value={line.quantity} onChange={(e) => {
                          const lines = rform.lines.slice();
                          lines[i] = { ...line, quantity: e.target.value };
                          setRform({ ...rform, lines });
                        }} />
                      </td>
                      <td>
                        <input className="input" value={line.wastage_percent} onChange={(e) => {
                          const lines = rform.lines.slice();
                          lines[i] = { ...line, wastage_percent: e.target.value };
                          setRform({ ...rform, lines });
                        }} />
                      </td>
                      <td>
                        <button type="button" className="btn btn-sm btn-danger" onClick={() => setRform({ ...rform, lines: rform.lines.filter((_, j) => j !== i) })}>−</button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <button type="button" className="btn btn-sm" onClick={() => setRform({ ...rform, lines: [...rform.lines, { product_id: 0, quantity: "1", unit: "", wastage_percent: "0" }] })}>
                {t("mfg.add_line")}
              </button>
              <div className="form-actions">
                <button type="button" className="btn" onClick={() => setRecipeModal(false)}>{t("common.cancel")}</button>
                <button className="btn btn-primary" type="submit" disabled={saving}>{t("common.save")}</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {orderModal && (
        <div className="modal-backdrop" onClick={() => setOrderModal(false)}>
          <div className="card modal" style={{ maxWidth: 520 }} onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">{t("mfg.new_order")}</h2>
            <form onSubmit={(e) => void saveOrder(e)}>
              <div className="field">
                <label className="label">{t("mfg.recipe")} *</label>
                <select className="input" required value={oform.recipe_id} onChange={(e) => setOform({ ...oform, recipe_id: Number(e.target.value) })}>
                  <option value={0}>—</option>
                  {recipes.map((r) => (
                    <option key={r.id} value={r.id}>{r.code ? `${r.code} ` : ""}{r.name}</option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label className="label">{t("mfg.qty")} *</label>
                <input className="input" required value={oform.quantity} onChange={(e) => setOform({ ...oform, quantity: e.target.value })} />
              </div>
              <div className="field">
                <label className="label">{t("mfg.location")} *</label>
                <select className="input" required value={oform.location_id} onChange={(e) => setOform({ ...oform, location_id: Number(e.target.value) })}>
                  <option value={0}>—</option>
                  {locations.map((l) => (
                    <option key={l.id} value={l.id}>{l.name}</option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label className="label">{t("mfg.date")}</label>
                <input className="input" type="date" value={oform.order_date} onChange={(e) => setOform({ ...oform, order_date: e.target.value })} />
              </div>
              <div className="form-actions">
                <button type="button" className="btn" onClick={() => setOrderModal(false)}>{t("common.cancel")}</button>
                <button className="btn btn-primary" type="submit" disabled={saving || !oform.recipe_id || !oform.location_id}>{t("common.save")}</button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}

export default function ProductionPage() {
  return (
    <RequireAuth>
      <ProductionInner />
    </RequireAuth>
  );
}
