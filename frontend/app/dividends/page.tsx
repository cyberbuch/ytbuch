"use client";

// Дивиденти — разпределение на печалба към действителните собственици
// от Настройки → SAF-T. Данък 5% по подразбиране (ЗДДФЛ).

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { api } from "../../lib/api";
import { formatBgDate, todayIso } from "../../lib/dates";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { IconButton } from "../../components/IconButton";

interface Owner {
  id: number;
  first_name_bg: string;
  last_name_bg: string;
  egn: string;
  ownership_percentage: string;
}

interface Dividend {
  id: number;
  beneficial_owner_id: number;
  owner_name: string;
  owner_egn: string;
  ownership_percentage: string;
  gross_amount: string;
  tax_rate: string;
  tax_amount: string;
  net_amount: string;
  is_paid: number;
  payment_date: string;
}

interface Dist {
  id: number;
  year: number;
  total_amount: string;
  decision_date: string;
  decision_number: string;
  status: string;
  notes: string;
  dividends_count: number;
  total_net: string;
  total_paid: string;
  total_tax: string;
  dividends?: Dividend[];
}

interface DistList {
  items: Dist[];
  count: number;
  owners: Owner[];
  currency: string;
}

function money(raw: string | undefined): string {
  const n = Number(String(raw ?? "").replace(",", "."));
  if (!Number.isFinite(n)) return raw || "0,00";
  return n.toLocaleString("bg-BG", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function DividendsInner() {
  const { t } = useI18n();
  const [list, setList] = useState<Dist[]>([]);
  const [owners, setOwners] = useState<Owner[]>([]);
  const [currency, setCurrency] = useState("BGN");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [detail, setDetail] = useState<Dist | null>(null);
  const [modal, setModal] = useState(false);
  const [editing, setEditing] = useState<Dist | null>(null);
  const [saving, setSaving] = useState(false);
  const now = new Date();
  const [form, setForm] = useState({
    year: String(now.getFullYear()),
    total_amount: "",
    tax_rate: "5",
    decision_date: todayIso(),
    decision_number: "",
    notes: "",
  });

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<DistList>("/v1/dividend-distributions");
      setList(data.items ?? []);
      setOwners(data.owners ?? []);
      setCurrency(data.currency || "BGN");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const openCreate = () => {
    setEditing(null);
    setForm({
      year: String(new Date().getFullYear()),
      total_amount: "",
      tax_rate: "5",
      decision_date: todayIso(),
      decision_number: "",
      notes: "",
    });
    setModal(true);
  };

  const openEdit = (d: Dist) => {
    setEditing(d);
    setForm({
      year: String(d.year),
      total_amount: d.total_amount,
      tax_rate: d.dividends?.[0]?.tax_rate ?? "5",
      decision_date: d.decision_date,
      decision_number: d.decision_number ?? "",
      notes: d.notes ?? "",
    });
    setModal(true);
  };

  const eligible = useMemo(
    () => owners.filter((o) => Number(String(o.ownership_percentage).replace(",", ".")) > 0),
    [owners]
  );
  const totalPct = eligible.reduce(
    (a, o) => a + Number(String(o.ownership_percentage).replace(",", ".")),
    0
  );

  const preview = useMemo(() => {
    const total = Number(String(form.total_amount).replace(",", "."));
    const rate = Number(String(form.tax_rate).replace(",", "."));
    if (!Number.isFinite(total) || total <= 0 || totalPct <= 0) return [];
    return eligible.map((o, i) => {
      const pct = Number(String(o.ownership_percentage).replace(",", "."));
      let gross =
        i === eligible.length - 1
          ? 0
          : Math.round((total * pct) / totalPct * 100) / 100;
      return { o, pct, gross, tax: 0, net: 0 };
    }).map((row, i, arr) => {
      let gross = row.gross;
      if (i === arr.length - 1) {
        const assigned = arr.slice(0, -1).reduce((s, r) => s + r.gross, 0);
        gross = Math.round((total - assigned) * 100) / 100;
      }
      const tax = Math.round((gross * rate) / 100 * 100) / 100;
      return { ...row, gross, tax, net: Math.round((gross - tax) * 100) / 100 };
    });
  }, [eligible, form.total_amount, form.tax_rate, totalPct]);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError("");
    const payload = {
      year: Number(form.year),
      total_amount: form.total_amount.replace(",", "."),
      tax_rate: form.tax_rate.replace(",", "."),
      decision_date: form.decision_date,
      decision_number: form.decision_number,
      notes: form.notes,
    };
    try {
      if (editing) {
        const d = await api.patch<Dist>(`/v1/dividend-distributions/${editing.id}`, payload);
        setDetail(d);
      } else {
        const d = await api.post<Dist>("/v1/dividend-distributions", payload);
        setDetail(d);
      }
      setModal(false);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const openDetail = async (id: number) => {
    setError("");
    try {
      const d = await api.get<Dist>(`/v1/dividend-distributions/${id}`);
      setDetail(d);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const patchStatus = async (id: number, status: string) => {
    setError("");
    try {
      const d = await api.patch<Dist>(`/v1/dividend-distributions/${id}`, { status });
      setDetail(d);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const remove = async (id: number) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    setError("");
    try {
      await api.del(`/v1/dividend-distributions/${id}`);
      setDetail(null);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const pay = async (row: Dividend, isPaid: boolean) => {
    setError("");
    try {
      await api.post(`/v1/dividends/${row.id}/pay`, {
        is_paid: isPaid,
        payment_date: isPaid ? todayIso() : "",
      });
      if (detail) await openDetail(detail.id);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const printProtocol = (d: Dist) => {
    const rows = d.dividends ?? [];
    const tax = rows.reduce((s, r) => s + Number(r.tax_amount), 0);
    const net = rows.reduce((s, r) => s + Number(r.net_amount), 0);
    const html = `<!doctype html><html><head><meta charset="utf-8"><title>${t("div.protocol")} ${d.year}</title>
<style>body{font-family:sans-serif;padding:24px}table{width:100%;border-collapse:collapse}td,th{border:1px solid #ccc;padding:6px;text-align:left}.num{text-align:right}h1{font-size:18px}</style></head><body>
<h1>${t("div.protocol")} ${d.year}</h1>
<p>${t("div.decision_date")}: ${formatBgDate(d.decision_date)} · ${t("div.decision_number")}: ${d.decision_number || "—"}</p>
<table><thead><tr><th>${t("div.owner")}</th><th>${t("div.egn")}</th><th class="num">${t("div.share")}</th><th class="num">${t("div.gross")}</th><th class="num">${t("div.tax")}</th><th class="num">${t("div.net")}</th></tr></thead><tbody>
${rows.map((r) => `<tr><td>${r.owner_name}</td><td>${r.owner_egn || ""}</td><td class="num">${money(r.ownership_percentage)}</td><td class="num">${money(r.gross_amount)}</td><td class="num">${money(r.tax_amount)}</td><td class="num">${money(r.net_amount)}</td></tr>`).join("")}
<tr><th colspan="3">${t("div.total")}</th><th class="num">${money(d.total_amount)}</th><th class="num">${money(String(tax))}</th><th class="num">${money(String(net))}</th></tr>
</tbody></table></body></html>`;
    const w = window.open("", "_blank");
    if (!w) return;
    w.document.write(html);
    w.document.close();
    w.focus();
    w.print();
  };

  const statusLabel = (s: string) => t(`div.status.${s}`);

  if (detail) {
    const anyPaid = (detail.dividends ?? []).some((r) => r.is_paid);
    const canPay = detail.status !== "draft";
    return (
      <div>
        <div className="page-head">
          <h1 className="page-title">
            {t("div.title")} {detail.year}
          </h1>
          <div className="icon-actions">
            <button className="btn" onClick={() => setDetail(null)}>{t("common.back")}</button>
            <button className="btn" onClick={() => printProtocol(detail)}>{t("div.print")}</button>
            {!anyPaid && (
              <button className="btn" onClick={() => openEdit(detail)}>{t("common.edit")}</button>
            )}
            {detail.status === "draft" && (
              <button className="btn btn-primary" onClick={() => void patchStatus(detail.id, "approved")}>
                {t("div.approve")}
              </button>
            )}
            {detail.status !== "draft" && (
              <button className="btn" onClick={() => void patchStatus(detail.id, "draft")}>
                {t("div.revert")}
              </button>
            )}
            {detail.status === "draft" && (
              <button className="btn btn-danger" onClick={() => void remove(detail.id)}>
                {t("common.delete")}
              </button>
            )}
          </div>
        </div>
        {error && <div className="error-text">{error}</div>}
        <div className="muted" style={{ marginBottom: 12 }}>
          {statusLabel(detail.status)} · {t("div.decision_date")}: {formatBgDate(detail.decision_date)}
          {detail.decision_number ? ` · ${detail.decision_number}` : ""}
        </div>
        <div className="card content" style={{ marginBottom: 18 }}>
          <table className="table" style={{ maxWidth: 520 }}>
            <tbody>
              <tr><td>{t("div.gross")}</td><td className="num">{money(detail.total_amount)} {currency}</td></tr>
              <tr><td>{t("div.tax")}</td><td className="num">{money(detail.total_tax)} {currency}</td></tr>
              <tr><td>{t("div.net")}</td><td className="num">{money(detail.total_net)} {currency}</td></tr>
              <tr><td>{t("div.paid")}</td><td className="num">{money(detail.total_paid)} {currency}</td></tr>
            </tbody>
          </table>
        </div>
        <div className="card">
          <div className="content table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>{t("div.owner")}</th>
                  <th>{t("div.egn")}</th>
                  <th className="num">{t("div.share")}</th>
                  <th className="num">{t("div.gross")}</th>
                  <th className="num">{t("div.tax")}</th>
                  <th className="num">{t("div.net")}</th>
                  <th>{t("div.pay_status")}</th>
                  <th>{t("common.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {(detail.dividends ?? []).length === 0 ? (
                  <tr>
                    <td colSpan={8} className="muted">
                      {t("div.no_owners")}{" "}
                      <Link href="/settings">{t("div.to_settings")}</Link>
                    </td>
                  </tr>
                ) : (
                  (detail.dividends ?? []).map((r) => (
                    <tr key={r.id}>
                      <td>{r.owner_name}</td>
                      <td>{r.owner_egn}</td>
                      <td className="num">{money(r.ownership_percentage)}</td>
                      <td className="num">{money(r.gross_amount)}</td>
                      <td className="num">{money(r.tax_amount)}</td>
                      <td className="num">{money(r.net_amount)}</td>
                      <td>{r.is_paid ? `${t("div.paid")} ${formatBgDate(r.payment_date)}` : t("div.unpaid")}</td>
                      <td>
                        {canPay ? (
                          <button className="btn btn-sm" onClick={() => void pay(r, !r.is_paid)}>
                            {r.is_paid ? t("div.unpay") : t("div.mark_paid")}
                          </button>
                        ) : (
                          <span className="muted">{t("div.approve_first")}</span>
                        )}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>
        {modal && editModal()}
      </div>
    );
  }

  function editModal() {
    return (
      <div className="modal-backdrop" onClick={() => setModal(false)}>
        <div className="card modal" style={{ maxWidth: 640 }} onClick={(e) => e.stopPropagation()}>
          <h2 className="modal-title">{editing ? t("common.edit") : t("div.new")}</h2>
          <form onSubmit={(e) => void save(e)}>
            <div className="form-grid">
              <div className="field">
                <label className="label">{t("div.year")} *</label>
                <input className="input" value={form.year} onChange={(e) => setForm({ ...form, year: e.target.value })} required />
              </div>
              <div className="field">
                <label className="label">{t("div.total")} *</label>
                <input className="input" value={form.total_amount} onChange={(e) => setForm({ ...form, total_amount: e.target.value })} required />
              </div>
              <div className="field">
                <label className="label">{t("div.tax_rate")}</label>
                <input className="input" value={form.tax_rate} onChange={(e) => setForm({ ...form, tax_rate: e.target.value })} />
              </div>
              <div className="field">
                <label className="label">{t("div.decision_date")} *</label>
                <input className="input" type="date" value={form.decision_date} onChange={(e) => setForm({ ...form, decision_date: e.target.value })} required />
              </div>
              <div className="field">
                <label className="label">{t("div.decision_number")}</label>
                <input className="input" value={form.decision_number} onChange={(e) => setForm({ ...form, decision_number: e.target.value })} />
              </div>
              <div className="field">
                <label className="label">{t("div.notes")}</label>
                <input className="input" value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} />
              </div>
            </div>
            {eligible.length === 0 ? (
              <p className="muted">
                {t("div.no_owners")} <Link href="/settings">{t("div.to_settings")}</Link>
              </p>
            ) : (
              <div className="table-wrap" style={{ marginTop: 12 }}>
                <table className="table">
                  <thead>
                    <tr>
                      <th>{t("div.owner")}</th>
                      <th className="num">{t("div.share")}</th>
                      <th className="num">{t("div.gross")}</th>
                      <th className="num">{t("div.tax")}</th>
                      <th className="num">{t("div.net")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {preview.map((r) => (
                      <tr key={r.o.id}>
                        <td>{r.o.first_name_bg} {r.o.last_name_bg}</td>
                        <td className="num">{money(String(r.pct))}</td>
                        <td className="num">{money(String(r.gross))}</td>
                        <td className="num">{money(String(r.tax))}</td>
                        <td className="num">{money(String(r.net))}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            <div className="form-actions">
              <button type="button" className="btn" onClick={() => setModal(false)}>{t("common.cancel")}</button>
              <button className="btn btn-primary" type="submit" disabled={saving || eligible.length === 0}>
                {t("common.save")}
              </button>
            </div>
          </form>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("div.title")}</h1>
        <button className="btn btn-primary" onClick={openCreate}>{t("div.new")}</button>
      </div>
      <p className="muted" style={{ margin: "0 0 12px" }}>{t("div.hint")}</p>
      {error && <div className="error-text">{error}</div>}
      {loading && <div className="muted">{t("common.loading")}</div>}
      {!loading && list.length === 0 && <div className="muted">{t("common.empty")}</div>}
      {list.length > 0 && (
        <div className="card">
          <div className="content table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>{t("div.year")}</th>
                  <th>{t("div.decision_date")}</th>
                  <th>{t("div.decision_number")}</th>
                  <th className="num">{t("div.gross")}</th>
                  <th className="num">{t("div.net")}</th>
                  <th>{t("div.status_col")}</th>
                  <th>{t("common.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {list.map((d) => (
                  <tr key={d.id}>
                    <td>{d.year}</td>
                    <td>{formatBgDate(d.decision_date)}</td>
                    <td>{d.decision_number}</td>
                    <td className="num">{money(d.total_amount)}</td>
                    <td className="num">{money(d.total_net)}</td>
                    <td>{statusLabel(d.status)}</td>
                    <td>
                      <div className="icon-actions">
                        <IconButton icon="view" title={t("div.open")} onClick={() => void openDetail(d.id)} />
                        {d.status === "draft" && (
                          <IconButton icon="delete" title={t("common.delete")} danger onClick={() => void remove(d.id)} />
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
      {modal && editModal()}
    </div>
  );
}

export default function DividendsPage() {
  return (
    <RequireAuth>
      <DividendsInner />
    </RequireAuth>
  );
}
