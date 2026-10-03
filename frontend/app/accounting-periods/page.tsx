"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ListResponse } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";

interface Period {
  id?: number;
  year: number;
  month: number;
  status: string;
  closed_at?: string;
  notes?: string;
}

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

function PeriodsInner() {
  const { t } = useI18n();
  const [year, setYear] = useState(2026);
  const [items, setItems] = useState<Period[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(0);
  const [closeMonth, setCloseMonth] = useState(0);
  const [notes, setNotes] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<ListResponse<Period>>(`/v1/accounting-periods?year=${year}`);
      setItems(data.items ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [year]);

  useEffect(() => {
    load();
  }, [load]);

  const closed = items.filter((p) => p.status === "closed").length;
  const open = 12 - closed;

  const act = async (path: string, month: number, extra?: Record<string, string>) => {
    setBusy(month);
    setError("");
    try {
      const data = await api.post<ListResponse<Period>>(path, {
        year,
        month,
        ...extra,
      });
      setItems(data.items ?? []);
      setCloseMonth(0);
      setNotes("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(0);
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("per.title")}</h1>
        <div className="btn-row">
          <button className="btn" type="button" onClick={() => setYear((y) => y - 1)}>
            ‹
          </button>
          <strong>{year}</strong>
          <button className="btn" type="button" onClick={() => setYear((y) => y + 1)}>
            ›
          </button>
        </div>
      </div>
      <p className="muted">{t("per.hint")}</p>
      {error && <div className="error-text">{error}</div>}
      <div className="form-grid" style={{ marginBottom: 16 }}>
        <div className="card card-pad">
          <div className="muted">{t("per.open_n")}</div>
          <div style={{ fontSize: 24, fontWeight: 600, color: "var(--success)" }}>{open}</div>
        </div>
        <div className="card card-pad">
          <div className="muted">{t("per.closed_n")}</div>
          <div style={{ fontSize: 24, fontWeight: 600, color: "var(--danger)" }}>{closed}</div>
        </div>
      </div>
      {loading ? (
        <p className="muted">{t("common.loading")}</p>
      ) : (
        <div className="form-grid">
          {items.map((p) => {
            const closedP = p.status === "closed";
            return (
              <div className="card card-pad" key={p.month}>
                <h3 className="section-title" style={{ marginTop: 0 }}>
                  {t(`per.m${p.month}`)}
                </h3>
                <span className={`badge ${closedP ? "badge-danger" : "badge-success"}`}>
                  {closedP ? t("per.closed") : t("per.open")}
                </span>
                {p.closed_at ? (
                  <p className="muted" style={{ fontSize: 12 }}>
                    {p.closed_at}
                  </p>
                ) : null}
                {p.notes ? (
                  <p className="muted" style={{ fontSize: 12 }}>
                    {p.notes}
                  </p>
                ) : null}
                <div className="form-actions" style={{ marginTop: 12 }}>
                  {closedP ? (
                    <button
                      className="btn"
                      type="button"
                      disabled={busy === p.month}
                      onClick={() => act("/v1/accounting-periods/reopen", p.month)}
                    >
                      {t("per.reopen")}
                    </button>
                  ) : (
                    <button
                      className="btn btn-danger"
                      type="button"
                      disabled={busy === p.month}
                      onClick={() => {
                        setCloseMonth(p.month);
                        setNotes("");
                      }}
                    >
                      {t("per.close")}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
      {closeMonth > 0 && (
        <div className="card card-pad" style={{ marginTop: 16 }}>
          <h3 className="section-title">
            {t("per.close_title")}: {t(`per.m${closeMonth}`)} {year}
          </h3>
          <div className="field">
            <label className="label">{t("per.notes")}</label>
            <input className="input" value={notes} onChange={(e) => setNotes(e.target.value)} />
          </div>
          <div className="form-actions">
            <button className="btn" type="button" onClick={() => setCloseMonth(0)}>
              {t("common.cancel")}
            </button>
            <button
              className="btn btn-danger"
              type="button"
              onClick={() =>
                act("/v1/accounting-periods/close", closeMonth, {
                  notes,
                  closed_at: todayIso(),
                })
              }
            >
              {t("per.close")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default function AccountingPeriodsPage() {
  return (
    <RequireAuth>
      <PeriodsInner />
    </RequireAuth>
  );
}
