"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, ListResponse } from "../../lib/api";
import { Invoice, docTypesFor, formatBgDate } from "../../lib/invoice";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { IconButton } from "../../components/IconButton";

interface Counterpart {
  id: number;
  name: string;
}

interface BulkResult {
  posted?: number;
  unposted?: number;
  deleted?: number;
  failed: number;
}

type BulkAction = "post" | "unpost" | "delete";

function InvoicesInner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<Invoice[]>([]);
  const [counterparts, setCounterparts] = useState<Counterpart[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [search, setSearch] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [dirFilter, setDirFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [selected, setSelected] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);

  // Филтри от адреса (/invoices?status=draft) — така таблото може да води
  // право към чернови или вземания. Пазено в URL, не в Suspense.
  useEffect(() => {
    const q = new URLSearchParams(window.location.search);
    setDirFilter(q.get("direction") ?? "");
    setStatusFilter(q.get("status") ?? "");
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<ListResponse<Invoice>>("/v1/invoices");
      setRows(data.items ?? []);
      setSelected([]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
    api
      .get<ListResponse<Counterpart>>("/v1/counterparts")
      .then((d) => setCounterparts(d.items ?? []))
      .catch(() => setCounterparts([]));
  }, [load]);

  const cpName = (inv: Invoice) =>
    inv.counterpart_name ||
    counterparts.find((c) => c.id === inv.counterpart_id)?.name ||
    String(inv.counterpart_id);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    return rows.filter((inv) => {
      if (typeFilter && inv.document_type !== typeFilter) return false;
      if (dirFilter && inv.direction !== dirFilter) return false;
      if (statusFilter && inv.status !== statusFilter) return false;
      if (!q) return true;
      const name = (
        inv.counterpart_name ||
        counterparts.find((c) => c.id === inv.counterpart_id)?.name ||
        ""
      ).toLowerCase();
      return inv.number.toLowerCase().includes(q) || name.includes(q);
    });
  }, [rows, search, typeFilter, dirFilter, statusFilter, counterparts]);

  const statusClass = (s: string) => {
    if (s === "posted" || s === "paid") return "badge-success";
    if (s === "cancelled") return "badge-danger";
    return "badge-warning";
  };

  const toggleOne = (id: number) => {
    setSelected((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]
    );
  };

  const allSelected =
    filtered.length > 0 && filtered.every((inv) => selected.includes(inv.id));

  const toggleAll = () => {
    setSelected(allSelected ? [] : filtered.map((inv) => inv.id));
  };

  const handleDelete = async (inv: Invoice) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/invoices/${inv.id}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  // Групови действия: една заявка с всички избрани id-та. Backend-ът връща
  // колко са успешни и кои са се провалили (по документ), затова показваме
  // обобщение и презареждаме списъка.
  const runBulk = async (action: BulkAction) => {
    if (selected.length === 0) {
      setError(t("invoices.bulk.none"));
      return;
    }
    if (action === "delete" && !window.confirm(t("invoices.bulk.confirm_delete"))) {
      return;
    }
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const res = await api.post<BulkResult>(`/v1/invoices/bulk/${action}`, {
        ids: selected,
      });
      const ok = (res.posted ?? 0) + (res.unposted ?? 0) + (res.deleted ?? 0);
      setNotice(
        `${t("invoices.bulk.result")}: ${ok}, ${t("invoices.bulk.failed")}: ${res.failed}`
      );
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("invoices.title")}</h1>
        <div style={{ display: "flex", gap: 8 }}>
          <Link className="btn" href="/scan">
            {t("invoices.scan")}
          </Link>
          <Link className="btn btn-primary" href="/invoices/new">
            {t("invoices.new")}
          </Link>
        </div>
      </div>

      {error && <div className="error-text">{error}</div>}
      {notice && <div className="muted" style={{ marginBottom: 8 }}>{notice}</div>}

      <div className="card" style={{ padding: 16, marginBottom: 12 }}>
        <div className="form-grid">
          <div className="field">
            <input
              className="input"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t("invoices.search")}
            />
          </div>
          <div className="field">
            <select
              className="select"
              value={typeFilter}
              onChange={(e) => setTypeFilter(e.target.value)}
            >
              <option value="">{t("invoices.all_types")}</option>
              {docTypesFor("out").map((dt) => (
                <option key={dt} value={dt}>
                  {dt === "proforma" ? "" : `${dt} — `}
                  {t(`invoices.document_type.${dt}`)}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <select
              className="select"
              value={dirFilter}
              onChange={(e) => setDirFilter(e.target.value)}
            >
              <option value="">{t("invoices.all_directions")}</option>
              <option value="out">{t("invoices.direction.out")}</option>
              <option value="in">{t("invoices.direction.in")}</option>
            </select>
          </div>
          <div className="field">
            <select
              className="select"
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
            >
              <option value="">{t("invoices.all_statuses")}</option>
              <option value="draft">{t("invoices.status.draft")}</option>
              <option value="posted">{t("invoices.status.posted")}</option>
            </select>
          </div>
        </div>
      </div>

      {selected.length > 0 && (
        <div className="card" style={{ padding: 12, marginBottom: 12 }}>
          <div className="btn-row wrap">
            <strong>
              {selected.length} {t("invoices.bulk.selected")}
            </strong>
            <button
              className="btn btn-primary btn-sm"
              disabled={busy}
              onClick={() => void runBulk("post")}
            >
              {t("invoices.bulk.post")}
            </button>
            <button
              className="btn btn-sm"
              disabled={busy}
              onClick={() => void runBulk("unpost")}
            >
              {t("invoices.bulk.unpost")}
            </button>
            <button
              className="btn btn-danger btn-sm"
              disabled={busy}
              onClick={() => void runBulk("delete")}
            >
              {t("invoices.bulk.delete")}
            </button>
            <button
              className="btn btn-ghost btn-sm"
              disabled={busy}
              onClick={() => setSelected([])}
            >
              {t("invoices.bulk.clear")}
            </button>
          </div>
        </div>
      )}

      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : filtered.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 32 }}>
                  <input
                    type="checkbox"
                    checked={allSelected}
                    onChange={toggleAll}
                    title={t("invoices.bulk.select_all")}
                  />
                </th>
                <th>{t("invoices.document_type")}</th>
                <th>{t("invoices.number")}</th>
                <th>{t("invoices.issue_date")}</th>
                <th>{t("invoices.counterpart")}</th>
                <th>{t("invoices.total")}</th>
                <th>{t("invoices.status")}</th>
                <th>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((inv) => (
                <tr key={inv.id}>
                  <td>
                    <input
                      type="checkbox"
                      checked={selected.includes(inv.id)}
                      onChange={() => toggleOne(inv.id)}
                    />
                  </td>
                  <td>
                    <Link href={`/invoices/${inv.id}`}>
                      {t(`invoices.document_type.${inv.document_type}`)}
                    </Link>
                  </td>
                  <td>
                    <Link href={`/invoices/${inv.id}`}>{inv.number}</Link>
                  </td>
                  <td>{formatBgDate(inv.issue_date)}</td>
                  <td>{cpName(inv)}</td>
                  <td>
                    {inv.total_amount} {inv.currency}
                  </td>
                  <td>
                    <span className={`badge ${statusClass(inv.status)}`}>
                      {t(`invoices.status.${inv.status}`)}
                    </span>
                  </td>
                  <td>
                    <div className="icon-actions">
                      <IconButton
                        icon="view"
                        title={t("common.view")}
                        href={`/invoices/${inv.id}`}
                      />
                      {inv.status === "draft" && (
                        <IconButton
                          icon="edit"
                          title={t("common.edit")}
                          href={`/invoices/${inv.id}/edit`}
                        />
                      )}
                      <IconButton
                        icon="delete"
                        title={t("common.delete")}
                        danger
                        onClick={() => void handleDelete(inv)}
                      />
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          </div>
        )}
      </div>
    </div>
  );
}

export default function InvoicesPage() {
  return (
    <RequireAuth>
      <InvoicesInner />
    </RequireAuth>
  );
}
