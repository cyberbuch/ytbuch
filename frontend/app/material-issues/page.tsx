"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { ACTIVE_COMPANY_EVENT, api, fetchAll, ListResponse } from "../../lib/api";
import { CompanyLocation, fetchCompanyLocations } from "../../lib/locations";
import { Pagination } from "../../components/Pagination";
import { AuditHistoryPanel, AuditStampBadge } from "../../components/AuditHistory";
import { AuditEntry } from "../../lib/invoice";

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
  is_inventory: number;
  inventory_account_id: number;
  expense_account_id?: number;
  auto_write_off?: number;
}

interface Rule {
  id: number;
  kind: string;
  inventory_account_id: number;
  offset_account_id: number;
}

interface Line {
  product_id: number;
  quantity: string;
  account_id: number;
  inventory_account_id: number;
  unit_cost?: string;
  amount?: string;
}

interface Doc {
  id: number;
  number: string;
  doc_date: string;
  location_id: number;
  status: string;
  notes: string;
  lines?: Line[];
  created_by?: string;
  created_at?: string;
  updated_by?: string;
  updated_at?: string;
  edit_count?: number;
  audit_history?: AuditEntry[];
}

function isoDay(): string {
  const d = new Date();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

function Inner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<Doc[]>([]);
  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [rules, setRules] = useState<Rule[]>([]);
  const [doc, setDoc] = useState<Doc | null>(null);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [list, locs, prods, acc, rulePack] = await Promise.all([
        fetchAll<Doc>("/v1/material-issues"),
        fetchCompanyLocations(),
        fetchAll<Product>("/v1/products"),
        fetchAll<Account>("/v1/accounts"),
        api.get<ListResponse<Rule>>("/v1/stock/rules"),
      ]);
      setRows(list);
      setLocations(locs);
      setProducts(prods.filter((p) => Number(p.is_inventory) === 1 && Number(p.auto_write_off) !== 1));
      setAccounts(acc);
      setRules((rulePack.items ?? []).filter((r) => r.kind === "material"));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
    const onChange = () => {
      setDoc(null);
      setCreating(false);
      load();
    };
    window.addEventListener(ACTIVE_COMPANY_EVENT, onChange);
    return () => window.removeEventListener(ACTIVE_COMPANY_EVENT, onChange);
  }, [load]);

  const locName = useCallback(
    (id: number) => locations.find((l) => l.id === id)?.name ?? "",
    [locations]
  );
  const accLabel = (id: number) => {
    const a = accounts.find((x) => x.id === id);
    return a ? `${a.number} ${a.name}` : "";
  };

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((d) =>
      `${d.number} ${d.doc_date} ${locName(d.location_id)} ${d.status} ${d.notes}`
        .toLowerCase()
        .includes(s)
    );
  }, [rows, q, locName]);

  const pageCount = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const page = Math.min(Math.floor(offset / PAGE_SIZE), pageCount - 1);
  const start = page * PAGE_SIZE;
  const paged = filtered.slice(start, start + PAGE_SIZE);

  async function open(id: number) {
    setError("");
    try {
      const full = await api.get<Doc>(`/v1/material-issues/${id}`);
      setDoc(full);
      setCreating(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function startNew() {
    setDoc({
      id: 0,
      number: "",
      doc_date: isoDay(),
      location_id: locations[0]?.id ?? 0,
      status: "draft",
      notes: "",
      lines: [{ product_id: 0, quantity: "1", account_id: 0, inventory_account_id: 0 }],
    });
    setCreating(true);
  }

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("mat.title")}</h1>
        {!doc && (
          <button type="button" className="btn btn-primary" onClick={startNew}>
            {t("mat.new")}
          </button>
        )}
      </div>
      {error && <div className="error-text">{error}</div>}
      {doc ? (
        <Editor
          t={t}
          doc={doc}
          creating={creating}
          products={products}
          rules={rules}
          locations={locations}
          accLabel={accLabel}
          onCancel={() => {
            setDoc(null);
            setCreating(false);
          }}
          onSaved={async (id) => {
            await load();
            await open(id);
          }}
          onDeleted={async () => {
            setDoc(null);
            setCreating(false);
            await load();
          }}
          onError={setError}
        />
      ) : loading ? (
        <div className="content muted">{t("common.loading")}</div>
      ) : (
        <>
          <div className="card" style={{ marginBottom: 12 }}>
            <input
              className="input"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder={t("common.search")}
            />
          </div>
          <div className="card">
            {filtered.length === 0 ? (
              <div className="content muted">{t("common.empty")}</div>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("mat.number")}</th>
                    <th>{t("mat.date")}</th>
                    <th>{t("mat.location")}</th>
                    <th>{t("mat.status")}</th>
                    <th>{t("audit.col_stamp")}</th>
                  </tr>
                </thead>
                <tbody>
                  {paged.map((d) => (
                    <tr key={d.id}>
                      <td>
                        <button type="button" className="btn-ghost" onClick={() => open(d.id)}>
                          {d.number}
                        </button>
                      </td>
                      <td>{d.doc_date}</td>
                      <td>{locName(d.location_id)}</td>
                      <td>
                        <span className={`badge ${d.status === "confirmed" ? "badge-success" : "badge-warning"}`}>
                          {t(`mat.status.${d.status}`)}
                        </span>
                      </td>
                      <td>
                        <AuditStampBadge
                          createdBy={d.created_by}
                          createdAt={d.created_at}
                          updatedBy={d.updated_by}
                          updatedAt={d.updated_at}
                          editCount={d.edit_count}
                        />
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
        </>
      )}
    </div>
  );
}

function offsets(rules: Rule[], inventory: number, expense = 0): number[] {
  const ids = rules.filter((r) => r.inventory_account_id === inventory).map((r) => r.offset_account_id);
  if (expense > 0) ids.unshift(expense);
  return [...new Set(ids)];
}

function Editor({
  t,
  doc,
  creating,
  products,
  rules,
  locations,
  accLabel,
  onCancel,
  onSaved,
  onDeleted,
  onError,
}: {
  t: (key: string) => string;
  doc: Doc;
  creating: boolean;
  products: Product[];
  rules: Rule[];
  locations: CompanyLocation[];
  accLabel: (id: number) => string;
  onCancel: () => void;
  onSaved: (id: number) => Promise<void>;
  onDeleted: () => Promise<void>;
  onError: (msg: string) => void;
}) {
  const [date, setDate] = useState(doc.doc_date);
  const [locationId, setLocationId] = useState(doc.location_id);
  const [notes, setNotes] = useState(doc.notes || "");
  const [lines, setLines] = useState<Line[]>(doc.lines ?? []);
  const [busy, setBusy] = useState(false);
  const locked = doc.status === "confirmed";

  useEffect(() => {
    setDate(doc.doc_date);
    setLocationId(doc.location_id);
    setNotes(doc.notes || "");
    setLines(doc.lines ?? []);
  }, [doc]);

  function setLine(i: number, patch: Partial<Line>) {
    setLines((prev) => prev.map((line, n) => (n === i ? { ...line, ...patch } : line)));
  }

  function pickProduct(i: number, productId: number) {
    const product = products.find((p) => p.id === productId);
    const inventory = product?.inventory_account_id ?? 0;
    const choices = offsets(rules, inventory, product?.expense_account_id ?? 0);
    setLine(i, {
      product_id: productId,
      inventory_account_id: inventory,
      account_id: product?.expense_account_id || (choices.length === 1 ? choices[0] : 0),
    });
  }

  async function save(e?: FormEvent) {
    e?.preventDefault();
    setBusy(true);
    onError("");
    const body = {
      doc_date: date,
      location_id: locationId,
      notes,
      lines: lines.map((l) => ({
        product_id: l.product_id,
        quantity: l.quantity,
        account_id: l.account_id,
        inventory_account_id: l.inventory_account_id,
      })),
    };
    try {
      if (creating) {
        const created = await api.post<Doc>("/v1/material-issues", body);
        await onSaved(created.id);
      } else {
        await api.patch(`/v1/material-issues/${doc.id}`, body);
        await onSaved(doc.id);
      }
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function act(path: string) {
    setBusy(true);
    onError("");
    try {
      if (!creating && !locked) await save();
      await api.post(path, {});
      await onSaved(doc.id);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    setBusy(true);
    onError("");
    try {
      await api.del(`/v1/material-issues/${doc.id}`);
      await onDeleted();
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="card card-pad" onSubmit={save}>
      <div className="page-head">
        <h2 style={{ margin: 0 }}>{creating ? t("mat.new") : doc.number}</h2>
        <button type="button" className="btn" onClick={onCancel}>
          {t("mat.back")}
        </button>
      </div>
      <div className="form-grid">
        <div className="field">
          <label className="label">{t("mat.date")}</label>
          <input className="input" type="date" value={date} disabled={locked} onChange={(e) => setDate(e.target.value)} />
        </div>
        <div className="field">
          <label className="label">{t("mat.location")}</label>
          <select className="select" value={String(locationId)} disabled={locked} onChange={(e) => setLocationId(Number(e.target.value) || 0)}>
            <option value="0">{t("stock.rules.pick")}</option>
            {locations.map((l) => (
              <option key={l.id} value={l.id}>{l.name}</option>
            ))}
          </select>
        </div>
        <div className="field">
          <label className="label">{t("mat.notes")}</label>
          <input className="input" value={notes} disabled={locked} onChange={(e) => setNotes(e.target.value)} />
        </div>
      </div>
      <table className="table">
        <thead>
          <tr>
            <th>{t("mat.product")}</th>
            <th>{t("mat.quantity")}</th>
            <th>{t("mat.account")}</th>
            <th>{t("mat.cost")}</th>
          </tr>
        </thead>
        <tbody>
          {lines.map((line, i) => {
            const product = products.find((p) => p.id === line.product_id);
            const choices = offsets(rules, line.inventory_account_id, product?.expense_account_id ?? 0);
            return (
              <tr key={i}>
                <td>
                  <select className="select" value={String(line.product_id)} disabled={locked} onChange={(e) => pickProduct(i, Number(e.target.value) || 0)}>
                    <option value="0">{t("stock.rules.pick")}</option>
                    {products.map((p) => (
                      <option key={p.id} value={p.id}>{p.code ? `${p.code} ` : ""}{p.name}</option>
                    ))}
                  </select>
                </td>
                <td>
                  <input className="input" value={line.quantity} disabled={locked} onChange={(e) => setLine(i, { quantity: e.target.value })} />
                </td>
                <td>
                  {choices.length === 0 ? (
                    <span className="muted">{t("mat.no_rule")}</span>
                  ) : (
                    <select className="select" value={String(line.account_id)} disabled={locked} onChange={(e) => setLine(i, { account_id: Number(e.target.value) || 0 })}>
                      <option value="0">{t("mat.pick_account")}</option>
                      {choices.map((id) => (
                        <option key={id} value={id}>{accLabel(id)}</option>
                      ))}
                    </select>
                  )}
                </td>
                <td>{line.amount || ""}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {!locked && (
        <button
          type="button"
          className="btn"
          onClick={() => setLines((prev) => [...prev, { product_id: 0, quantity: "1", account_id: 0, inventory_account_id: 0 }])}
        >
          {t("mat.add_line")}
        </button>
      )}
      <div className="form-actions">
        {!locked && (
          <button className="btn btn-primary" disabled={busy || locationId <= 0}>
            {t("mat.save")}
          </button>
        )}
        {!creating && !locked && (
          <button type="button" className="btn btn-primary" disabled={busy || locationId <= 0} onClick={() => act(`/v1/material-issues/${doc.id}/confirm`)}>
            {t("mat.confirm")}
          </button>
        )}
        {!creating && locked && (
          <button type="button" className="btn" disabled={busy} onClick={() => act(`/v1/material-issues/${doc.id}/unpost`)}>
            {t("mat.unpost")}
          </button>
        )}
        {!creating && !locked && (
          <button type="button" className="btn" disabled={busy} onClick={remove}>
            {t("mat.delete")}
          </button>
        )}
      </div>
      {!creating && (
        <AuditHistoryPanel
          history={doc.audit_history}
          createdBy={doc.created_by}
          createdAt={doc.created_at}
          updatedBy={doc.updated_by}
          updatedAt={doc.updated_at}
          editCount={doc.edit_count}
        />
      )}
    </form>
  );
}

export default function MaterialIssuesPage() {
  return (
    <RequireAuth>
      <Inner />
    </RequireAuth>
  );
}
