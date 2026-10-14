"use client";

import { useCallback, useEffect, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { ACTIVE_COMPANY_EVENT, api, downloadFile } from "../../lib/api";
import { CompanyLocation, fetchCompanyLocations } from "../../lib/locations";
import { Pagination } from "../../components/Pagination";

const PAGE_SIZE = 50;

interface Flow {
  date: string;
  document: string;
  reason: string;
  product_name: string;
  location_id: number;
  quantity: string;
  value: string;
}

interface Balance {
  product_name: string;
  location_id: number;
  quantity: string;
  value: string;
}

interface Report {
  receipts: Flow[];
  issues: Flow[];
  balances: Balance[];
  warnings: string[];
}

function isoDay(d = new Date()): string {
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

function Inner() {
  const { t } = useI18n();
  const today = isoDay();
  const [from, setFrom] = useState(`${today.slice(0, 8)}01`);
  const [to, setTo] = useState(today);
  const [month, setMonth] = useState(today.slice(0, 7));
  const [until, setUntil] = useState(today);
  const [locationId, setLocationId] = useState(0);
  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [report, setReport] = useState<Report | null>(null);
  const [note, setNote] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [balOffset, setBalOffset] = useState(0);

  const locName = useCallback(
    (id: number) => locations.find((l) => l.id === id)?.name ?? (id > 0 ? String(id) : ""),
    [locations]
  );

  const load = useCallback(async () => {
    setBusy(true);
    setError("");
    try {
      const data = await api.get<Report>("/v1/stock/report", {
        from,
        to,
        location_id: locationId > 0 ? locationId : undefined,
      });
      setReport(data);
      setLocations(await fetchCompanyLocations());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [from, to, locationId]);

  useEffect(() => {
    load();
    const onChange = () => load();
    window.addEventListener(ACTIVE_COMPANY_EVENT, onChange);
    return () => window.removeEventListener(ACTIVE_COMPANY_EVENT, onChange);
  }, [load]);

  async function run(path: string, body: unknown, ok: string) {
    setBusy(true);
    setError("");
    setNote("");
    try {
      const data = await api.post<{ warnings?: string[]; amount?: string; balances?: number }>(path, body);
      const warnings = data.warnings ?? [];
      const extra = data.amount ? ` ${data.amount}` : "";
      const warn = warnings.length > 0 ? `\n${warnings.join("\n")}` : "";
      setNote(`${ok}${extra}${warn}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  }

  async function exp(fmt: string) {
    setError("");
    try {
      const loc = locationId > 0 ? `&location_id=${locationId}` : "";
      await downloadFile(
        `/v1/reports/export?kind=stock&from=${from}&to=${to}${loc}&format=${fmt}&bg=1`,
        `sklad-${from}-${to}.${fmt}`
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  const reason = (r: string) => {
    const key = `stock.reason.${r}`;
    const label = t(key);
    return label === key ? r : label;
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("stock.title")}</h1>
      </div>
      {error && <div className="error-text">{error}</div>}
      {note && <div className="card card-pad" style={{ whiteSpace: "pre-wrap" }}>{note}</div>}
      <div className="card card-pad" style={{ marginBottom: 12 }}>
        <div className="form-actions">
          <button type="button" className="btn" disabled={busy} onClick={() => run("/v1/stock/rebuild", {}, t("stock.rebuild"))}>
            {t("stock.rebuild")}
          </button>
          <input className="input" type="month" value={month} onChange={(e) => setMonth(e.target.value)} />
          <button type="button" className="btn btn-primary" disabled={busy || month.length !== 7} onClick={() => run("/v1/stock/writeoff", { month }, t("stock.writeoff_month"))}>
            {t("stock.writeoff_month")}
          </button>
          <input className="input" type="date" value={until} onChange={(e) => setUntil(e.target.value)} />
          <button type="button" className="btn btn-primary" disabled={busy || until.length !== 10} onClick={() => run("/v1/stock/writeoff", { until }, t("stock.writeoff_until"))}>
            {t("stock.writeoff_until")}
          </button>
        </div>
      </div>
      <div className="card card-pad" style={{ marginBottom: 12 }}>
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("stock.from")}</label>
            <input className="input" type="date" value={from} onChange={(e) => setFrom(e.target.value)} />
          </div>
          <div className="field">
            <label className="label">{t("stock.to")}</label>
            <input className="input" type="date" value={to} onChange={(e) => setTo(e.target.value)} />
          </div>
          <div className="field">
            <label className="label">{t("stock.location")}</label>
            <select className="select" value={String(locationId)} onChange={(e) => setLocationId(Number(e.target.value) || 0)}>
              <option value="0">{t("stock.all_locations")}</option>
              {locations.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.name}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="form-actions">
          <button type="button" className="btn btn-primary" disabled={busy} onClick={load}>
            {t("stock.load")}
          </button>
          <button type="button" className="btn" disabled={busy} onClick={() => exp("pdf")}>PDF</button>
          <button type="button" className="btn" disabled={busy} onClick={() => exp("xlsx")}>XLSX</button>
          <button type="button" className="btn" disabled={busy} onClick={() => exp("ods")}>ODS</button>
        </div>
      </div>
      {report && (
        <>
          {(report.warnings ?? []).length > 0 && (
            <div className="card card-pad" style={{ marginBottom: 12 }}>
              <strong>{t("stock.warnings")}</strong>
              <ul>
                {report.warnings.map((w, i) => (
                  <li key={`${i}-${w}`}>{w}</li>
                ))}
              </ul>
            </div>
          )}
          <FlowTable title={t("stock.receipts")} rows={report.receipts ?? []} locName={locName} reason={reason} t={t} />
          <FlowTable title={t("stock.issues")} rows={report.issues ?? []} locName={locName} reason={reason} t={t} />
          {(() => {
            const bals = report.balances ?? [];
            const pageCount = Math.max(1, Math.ceil(bals.length / PAGE_SIZE));
            const page = Math.min(Math.floor(balOffset / PAGE_SIZE), pageCount - 1);
            const start = page * PAGE_SIZE;
            const pagedBals = bals.slice(start, start + PAGE_SIZE);
            return (
              <div className="card" style={{ marginTop: 12 }}>
                <div className="content"><strong>{t("stock.balances")}</strong></div>
                <table className="table">
                  <thead>
                    <tr>
                      <th>{t("stock.product")}</th>
                      <th>{t("stock.location")}</th>
                      <th>{t("stock.quantity")}</th>
                      <th>{t("stock.value")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {bals.length === 0 ? (
                      <tr><td colSpan={4} className="muted">{t("common.empty")}</td></tr>
                    ) : (
                      pagedBals.map((b, i) => (
                        <tr key={`${b.product_name}-${b.location_id}-${i}`}>
                          <td>{b.product_name}</td>
                          <td>{locName(b.location_id)}</td>
                          <td>{b.quantity}</td>
                          <td>{b.value}</td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
                <Pagination
                  total={bals.length}
                  limit={PAGE_SIZE}
                  offset={start}
                  onPageChange={setBalOffset}
                />
              </div>
            );
          })()}
        </>
      )}
    </div>
  );
}

function FlowTable({
  title,
  rows,
  locName,
  reason,
  t,
}: {
  title: string;
  rows: Flow[];
  locName: (id: number) => string;
  reason: (r: string) => string;
  t: (key: string) => string;
}) {
  const [offset, setOffset] = useState(0);
  const pageCount = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const page = Math.min(Math.floor(offset / PAGE_SIZE), pageCount - 1);
  const start = page * PAGE_SIZE;
  const paged = rows.slice(start, start + PAGE_SIZE);
  return (
    <div className="card" style={{ marginTop: 12 }}>
      <div className="content"><strong>{title}</strong></div>
      <table className="table">
        <thead>
          <tr>
            <th>{t("stock.date")}</th>
            <th>{t("stock.document")}</th>
            <th>{t("stock.product")}</th>
            <th>{t("stock.location")}</th>
            <th>{t("stock.quantity")}</th>
            <th>{t("stock.value")}</th>
            <th>{t("stock.reason")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr><td colSpan={7} className="muted">{t("common.empty")}</td></tr>
          ) : (
            paged.map((r, i) => (
              <tr key={`${r.date}-${r.document}-${r.product_name}-${i}`}>
                <td>{r.date}</td>
                <td>{r.document}</td>
                <td>{r.product_name}</td>
                <td>{locName(r.location_id)}</td>
                <td>{r.quantity}</td>
                <td>{r.value}</td>
                <td>{reason(r.reason)}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
      <Pagination
        total={rows.length}
        limit={PAGE_SIZE}
        offset={start}
        onPageChange={setOffset}
      />
    </div>
  );
}

export default function StockPage() {
  return (
    <RequireAuth>
      <Inner />
    </RequireAuth>
  );
}
