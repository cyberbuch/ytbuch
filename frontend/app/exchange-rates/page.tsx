"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";

interface RateRow {
  id: number;
  currency_code: string;
  rate: string;
  date: string;
}

interface RateList {
  date: string;
  base: string;
  items: RateRow[];
}

interface ImportResult {
  mode: string;
  inserted: number;
  updated: number;
  from_date: string;
  to_date: string;
}

function RatesInner() {
  const { t } = useI18n();
  const [date, setDate] = useState("");
  const [data, setData] = useState<RateList | null>(null);
  const [summary, setSummary] = useState("");
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState("");

  const load = useCallback(async (d: string) => {
    setLoading(true);
    setError("");
    try {
      const url = d
        ? `/v1/exchange-rates?date=${encodeURIComponent(d)}`
        : "/v1/exchange-rates";
      setData(await api.get<RateList>(url));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load("");
  }, [load]);

  const doImport = async (mode: "daily" | "history") => {
    setImporting(true);
    setError("");
    setSummary("");
    try {
      const r = await api.post<ImportResult>("/v1/exchange-rates/import", { mode });
      setSummary(
        `${t("rates.imported")}: ${r.inserted} · ${t("rates.updated")}: ${r.updated}` +
          (r.from_date ? ` · ${r.from_date} — ${r.to_date}` : "")
      );
      await load(date);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("rates.title")}</h1>
      </div>

      <div className="card content" style={{ marginBottom: 18 }}>
        <div style={{ display: "flex", gap: 8, alignItems: "flex-end", flexWrap: "wrap" }}>
          <div className="field" style={{ margin: 0 }}>
            <label className="label">{t("rates.date")}</label>
            <input
              className="input"
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
            />
          </div>
          <button className="btn btn-primary" onClick={() => load(date)} disabled={loading}>
            {t("rates.show")}
          </button>
          <button
            className="btn btn-ghost"
            onClick={() => doImport("daily")}
            disabled={importing}
          >
            {t("rates.import_daily")}
          </button>
          <button
            className="btn btn-ghost"
            onClick={() => doImport("history")}
            disabled={importing}
          >
            {t("rates.import_history")}
          </button>
        </div>
        {error && <div className="error-text" style={{ marginTop: 8 }}>{error}</div>}
        {summary && <div className="muted" style={{ marginTop: 8 }}>{summary}</div>}
        <div className="muted" style={{ marginTop: 8 }}>
          {t("rates.source")}
        </div>
      </div>

      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : !data || data.items.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("rates.currency")}</th>
                <th style={{ textAlign: "right" }}>{t("rates.rate")}</th>
                <th>{t("rates.date")}</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((r) => (
                <tr key={r.id}>
                  <td>{r.currency_code}</td>
                  <td style={{ textAlign: "right" }}>{r.rate}</td>
                  <td>{r.date}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

export default function ExchangeRatesPage() {
  return (
    <RequireAuth>
      <RatesInner />
    </RequireAuth>
  );
}
