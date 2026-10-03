"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { api, ListResponse } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { IconButton } from "../../components/IconButton";

interface Account {
  id: number;
  number: string;
  name: string;
  analytic_type: string;
}

interface Counterpart {
  id: number;
  name: string;
}

interface Product {
  id: number;
  name: string;
  code: string;
}

interface OpeningBalance {
  id: number;
  fiscal_year: number;
  account_id: number;
  account_number?: string;
  account_name?: string;
  analytic_type?: string;
  natural_side?: string;
  account_kind?: string;
  debit: string;
  credit: string;
  counterpart_id: number;
  counterpart_name?: string;
  product_id: number;
  product_name?: string;
  quantity: string;
  description: string;
}

interface ObList {
  items: OpeningBalance[];
  count: number;
  fiscal_year: number;
  total_debit: string;
  total_credit: string;
  difference: string;
}

function naturalSide(number: string): "debit" | "credit" | "any" {
  const n = number || "";
  const d0 = n.slice(0, 1);
  if (d0 === "1") return "credit";
  if (d0 === "2") return n.startsWith("24") ? "credit" : "debit";
  if (d0 === "3") return "debit";
  if (n.startsWith("4531")) return "debit";
  if (n.startsWith("4532")) return "credit";
  if (n.startsWith("40")) return "credit";
  if (n.startsWith("41")) return "debit";
  if (d0 === "5" || d0 === "6") return "debit";
  if (d0 === "7") return "credit";
  return "any";
}

function accountKind(number: string): string {
  const n = number || "";
  const d0 = n.slice(0, 1);
  if (d0 === "1") return "capital";
  if (d0 === "2") return "fixed_asset";
  if (d0 === "3") return "inventory";
  if (n.startsWith("40")) return "payable";
  if (n.startsWith("41")) return "receivable";
  if (d0 === "4") return "settlement";
  if (d0 === "5") return "cash";
  if (d0 === "6") return "expense";
  if (d0 === "7") return "revenue";
  return "other";
}

function ObInner() {
  const { t } = useI18n();
  const [year, setYear] = useState(2026);
  const [data, setData] = useState<ObList | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [counterparts, setCounterparts] = useState<Counterpart[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [show, setShow] = useState(false);
  const [saving, setSaving] = useState(false);
  const [accountId, setAccountId] = useState("");
  const [amount, setAmount] = useState("");
  const [counterpartId, setCounterpartId] = useState("");
  const [productId, setProductId] = useState("");
  const [quantity, setQuantity] = useState("");
  const [description, setDescription] = useState("");
  const [q, setQ] = useState("");

  const acc = accounts.find((a) => String(a.id) === accountId);
  const side = acc ? naturalSide(acc.number) : "any";
  const kind = acc ? accountKind(acc.number) : "other";
  const analytic = acc?.analytic_type || "none";

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [ob, accs, cps, prs] = await Promise.all([
        api.get<ObList>(`/v1/opening-balances?year=${year}`),
        api.get<ListResponse<Account>>("/v1/accounts"),
        api.get<ListResponse<Counterpart>>("/v1/counterparts"),
        api.get<ListResponse<Product>>("/v1/products"),
      ]);
      setData(ob);
      setAccounts(accs.items ?? []);
      setCounterparts(cps.items ?? []);
      setProducts(prs.items ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [year]);

  useEffect(() => {
    load();
  }, [load]);

  const reset = () => {
    setAccountId("");
    setAmount("");
    setCounterpartId("");
    setProductId("");
    setQuantity("");
    setDescription("");
  };

  const rows = useMemo(() => {
    const all = data?.items ?? [];
    const s = q.trim().toLowerCase();
    if (!s) return all;
    return all.filter((r) =>
      `${r.account_number} ${r.account_name} ${r.counterpart_name} ${r.product_name} ${r.description}`
        .toLowerCase()
        .includes(s)
    );
  }, [data, q]);
  const diff = Number(data?.difference || "0");
  const balanced = Math.abs(diff) < 0.005;

  const byKind = useMemo(() => {
    const map = new Map<string, { debit: number; credit: number }>();
    for (const r of rows) {
      const k = r.account_kind || "other";
      const cur = map.get(k) || { debit: 0, credit: 0 };
      cur.debit += Number(r.debit || 0);
      cur.credit += Number(r.credit || 0);
      map.set(k, cur);
    }
    return [...map.entries()];
  }, [rows]);

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!accountId) {
      setError(t("ob.pick_account"));
      return;
    }
    if (!amount || Number(amount) === 0) {
      setError(t("ob.need_amount"));
      return;
    }
    if (side === "any") {
      return;
    }
    setSaving(true);
    setError("");
    try {
      const debit = side === "credit" ? "0" : amount;
      const credit = side === "debit" ? "0" : amount;
      await api.post("/v1/opening-balances", {
        fiscal_year: year,
        account_id: Number(accountId),
        debit,
        credit,
        counterpart_id: analytic === "counterpart" ? Number(counterpartId) || 0 : 0,
        product_id: analytic === "product" ? Number(productId) || 0 : 0,
        quantity: analytic === "product" ? quantity || "0" : "0",
        description,
      });
      reset();
      setShow(false);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleAnySave = async (which: "debit" | "credit") => {
    if (!accountId || !amount) {
      setError(t("ob.need_amount"));
      return;
    }
    setSaving(true);
    setError("");
    try {
      await api.post("/v1/opening-balances", {
        fiscal_year: year,
        account_id: Number(accountId),
        debit: which === "debit" ? amount : "0",
        credit: which === "credit" ? amount : "0",
        counterpart_id: analytic === "counterpart" ? Number(counterpartId) || 0 : 0,
        product_id: analytic === "product" ? Number(productId) || 0 : 0,
        quantity: analytic === "product" ? quantity || "0" : "0",
        description,
      });
      reset();
      setShow(false);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const del = async (id: number) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/opening-balances/${id}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("ob.title")}</h1>
        <div className="btn-row">
          <label className="muted">{t("ob.year")}</label>
          <select
            className="select"
            value={year}
            onChange={(e) => setYear(Number(e.target.value))}
          >
            {[2024, 2025, 2026, 2027, 2028].map((y) => (
              <option key={y} value={y}>
                {y}
              </option>
            ))}
          </select>
          <button
            className="btn btn-primary"
            type="button"
            onClick={() => {
              reset();
              setShow((s) => !s);
            }}
          >
            {show ? t("common.cancel") : t("ob.new")}
          </button>
        </div>
      </div>
      <p className="muted">{t("ob.hint")}</p>
      {error && <div className="error-text">{error}</div>}
      <div className="card" style={{ marginBottom: 12 }}>
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t("common.search")}
        />
      </div>

      <div className="form-grid" style={{ marginBottom: 16 }}>
        <div className="card card-pad">
          <div className="muted">{t("ob.total_debit")}</div>
          <div style={{ fontWeight: 600 }}>{data?.total_debit ?? "0"}</div>
        </div>
        <div className="card card-pad">
          <div className="muted">{t("ob.total_credit")}</div>
          <div style={{ fontWeight: 600 }}>{data?.total_credit ?? "0"}</div>
        </div>
        <div className="card card-pad">
          <div className="muted">{t("ob.diff")}</div>
          <div style={{ fontWeight: 600, color: balanced ? "var(--success)" : "var(--danger)" }}>
            {data?.difference ?? "0"} {balanced ? `· ${t("ob.balanced")}` : ""}
          </div>
        </div>
      </div>

      {show && (
        <form className="card card-pad" style={{ marginBottom: 16 }} onSubmit={handleSave}>
          <div className="form-grid">
            <div className="field">
              <label className="label">{t("ob.account")} *</label>
              <select
                className="select"
                value={accountId}
                onChange={(e) => {
                  setAccountId(e.target.value);
                  setCounterpartId("");
                  setProductId("");
                }}
                required
              >
                <option value="">{t("ob.pick_account")}</option>
                {accounts.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.number} {a.name}
                  </option>
                ))}
              </select>
            </div>
            {acc && (
              <div className="field">
                <label className="label">{t("ob.kind")}</label>
                <input className="input" readOnly value={`${t(`ob.kind.${kind}`)} · ${t(`ob.side.${side}`)}`} />
              </div>
            )}
            {analytic === "counterpart" && (
              <div className="field">
                <label className="label">{t("ob.analytic")} *</label>
                <select
                  className="select"
                  value={counterpartId}
                  onChange={(e) => setCounterpartId(e.target.value)}
                  required
                >
                  <option value="">—</option>
                  {counterparts.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
              </div>
            )}
            {analytic === "product" && (
              <>
                <div className="field">
                  <label className="label">{t("ob.analytic")} *</label>
                  <select
                    className="select"
                    value={productId}
                    onChange={(e) => setProductId(e.target.value)}
                    required
                  >
                    <option value="">—</option>
                    {products.map((p) => (
                      <option key={p.id} value={p.id}>
                        {p.code ? `${p.code} ` : ""}
                        {p.name}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label className="label">{t("ob.qty")}</label>
                  <input className="input" value={quantity} onChange={(e) => setQuantity(e.target.value)} />
                </div>
              </>
            )}
            <div className="field">
              <label className="label">
                {side === "credit" ? t("ob.credit") : t("ob.debit")} *
              </label>
              <input
                className="input"
                inputMode="decimal"
                placeholder="-100.00"
                title={t("journal.sign_hint")}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                required={side !== "any"}
              />
            </div>
            <div className="field">
              <label className="label">{t("ob.desc")}</label>
              <input className="input" value={description} onChange={(e) => setDescription(e.target.value)} />
            </div>
          </div>
          <div className="form-actions">
            {side === "any" ? (
              <>
                <button className="btn" type="button" disabled={saving} onClick={() => handleAnySave("debit")}>
                  {t("ob.debit")}
                </button>
                <button className="btn" type="button" disabled={saving} onClick={() => handleAnySave("credit")}>
                  {t("ob.credit")}
                </button>
              </>
            ) : (
              <button className="btn btn-primary" disabled={saving}>
                {t("common.save")}
              </button>
            )}
          </div>
        </form>
      )}

      {byKind.length > 0 && (
        <div className="card" style={{ marginBottom: 16 }}>
          <table className="table">
            <thead>
              <tr>
                <th>{t("ob.kind")}</th>
                <th>{t("ob.debit")}</th>
                <th>{t("ob.credit")}</th>
              </tr>
            </thead>
            <tbody>
              {byKind.map(([k, v]) => (
                <tr key={k}>
                  <td>{t(`ob.kind.${k}`)}</td>
                  <td>{v.debit.toFixed(2)}</td>
                  <td>{v.credit.toFixed(2)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : rows.length === 0 ? (
          <div className="content muted">{t("ob.empty")}</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("ob.account")}</th>
                <th>{t("ob.kind")}</th>
                <th>{t("ob.analytic")}</th>
                <th>{t("ob.debit")}</th>
                <th>{t("ob.credit")}</th>
                <th>{t("ob.qty")}</th>
                <th>{t("ob.desc")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.id}>
                  <td>
                    {r.account_number} {r.account_name}
                  </td>
                  <td>{t(`ob.kind.${r.account_kind || "other"}`)}</td>
                  <td>{r.counterpart_name || r.product_name || "—"}</td>
                  <td>{Number(r.debit) ? r.debit : ""}</td>
                  <td>{Number(r.credit) ? r.credit : ""}</td>
                  <td>{Number(r.quantity) ? r.quantity : ""}</td>
                  <td>{r.description}</td>
                  <td>
                    <IconButton
                      icon="delete"
                      title={t("common.delete")}
                      danger
                      onClick={() => del(r.id)}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

export default function OpeningBalancesPage() {
  return (
    <RequireAuth>
      <ObInner />
    </RequireAuth>
  );
}
