"use client";

// Настройки → Финансови отчети: формули по бланка НСС 1 / СС 7.

import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { todayIso } from "../lib/dates";
import { useI18n } from "./I18nProvider";

type Statement = "balance" | "pl" | "cashflow" | "equity";

interface FsLine {
  code: string;
  parent: string;
  kind: string;
  side: string;
  name: string;
}

interface FsFormula {
  id: number;
  line_code: string;
  formula: string;
}

interface FsReportRow {
  code: string;
  kind: string;
  name: string;
  current: string;
  prior: string;
}

const STMTS: { id: Statement; key: string }[] = [
  { id: "balance", key: "fs.stmt.balance" },
  { id: "pl", key: "fs.stmt.pl" },
  { id: "cashflow", key: "fs.stmt.cash" },
  { id: "equity", key: "fs.stmt.equity" },
];

const KIND_TO_RPT: Record<Statement, string> = {
  balance: "balance_sheet",
  pl: "income_statement",
  cashflow: "cash_flow",
  equity: "equity",
};

function yearStart(): string {
  return `${new Date().getFullYear()}-01-01`;
}

function depthOf(code: string, byCode: Map<string, FsLine>): number {
  let d = 0;
  let cur = byCode.get(code);
  while (cur && cur.parent) {
    d += 1;
    cur = byCode.get(cur.parent);
  }
  return d;
}

export function FsFormulasTab() {
  const { t } = useI18n();
  const [stmt, setStmt] = useState<Statement>("balance");
  const [lines, setLines] = useState<FsLine[]>([]);
  const [formulas, setFormulas] = useState<Record<string, string>>({});
  const [sel, setSel] = useState("");
  const [draft, setDraft] = useState("");
  const [from, setFrom] = useState(yearStart);
  const [to, setTo] = useState(todayIso);
  const [preview, setPreview] = useState<FsReportRow[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState("");
  const [ok, setOk] = useState("");

  const byCode = useMemo(() => {
    const m = new Map<string, FsLine>();
    for (const l of lines) m.set(l.code, l);
    return m;
  }, [lines]);

  const selected = byCode.get(sel);

  const load = useCallback(async () => {
    setLoading(true);
    setErr("");
    try {
      const cat = await api.get<{ items: FsLine[] }>(`/v1/fs/lines?statement=${stmt}`);
      const f = await api.get<{ items: FsFormula[] }>(`/v1/fs/formulas?statement=${stmt}`);
      setLines(cat.items ?? []);
      const map: Record<string, string> = {};
      for (const row of f.items ?? []) map[row.line_code] = row.formula;
      setFormulas(map);
      setPreview(null);
      const first = (cat.items ?? []).find((x) => x.kind === "line") ?? cat.items?.[0];
      setSel(first?.code ?? "");
      setDraft(first ? map[first.code] ?? "" : "");
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [stmt]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!ok) return;
    const tmr = window.setTimeout(() => setOk(""), 2500);
    return () => window.clearTimeout(tmr);
  }, [ok]);

  const pick = (code: string) => {
    setSel(code);
    setDraft(formulas[code] ?? "");
  };

  const save = async () => {
    if (!sel) return;
    setSaving(true);
    setErr("");
    try {
      await api.put("/v1/fs/formulas", { statement: stmt, line_code: sel, formula: draft });
      setFormulas((prev) => ({ ...prev, [sel]: draft }));
      setOk(t("fs.saved"));
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const loadDefaults = async () => {
    setSaving(true);
    setErr("");
    try {
      await api.post("/v1/fs/formulas/defaults", { statement: stmt });
      setOk(t("fs.defaults_ok"));
      await load();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const clearAll = async () => {
    if (!window.confirm(t("fs.clear_confirm"))) return;
    setSaving(true);
    setErr("");
    try {
      await api.post("/v1/fs/formulas/clear", { statement: stmt });
      setOk(t("fs.cleared"));
      await load();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const runPreview = async () => {
    setSaving(true);
    setErr("");
    try {
      const data = await api.get<{ rows: FsReportRow[] }>(
        `/v1/reports?kind=${KIND_TO_RPT[stmt]}&from=${from}&to=${to}`
      );
      setPreview(data.rows ?? []);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div>
      <p className="muted" style={{ margin: "0 0 12px" }}>
        {t("fs.hint")}
      </p>
      <div className="tabs" style={{ marginBottom: 12 }}>
        {STMTS.map((s) => (
          <button
            key={s.id}
            type="button"
            className={`tab${stmt === s.id ? " tab-active" : ""}`}
            onClick={() => setStmt(s.id)}
          >
            {t(s.key)}
          </button>
        ))}
      </div>
      <div className="toolbar" style={{ marginBottom: 12 }}>
        <button className="btn" type="button" disabled={saving} onClick={() => void loadDefaults()}>
          {t("fs.defaults")}
        </button>
        <button className="btn" type="button" disabled={saving} onClick={() => void clearAll()}>
          {t("fs.clear")}
        </button>
        <div className="field" style={{ margin: 0 }}>
          <input className="input" type="date" value={from} onChange={(e) => setFrom(e.target.value)} />
        </div>
        <div className="field" style={{ margin: 0 }}>
          <input className="input" type="date" value={to} onChange={(e) => setTo(e.target.value)} />
        </div>
        <button className="btn btn-primary" type="button" disabled={saving} onClick={() => void runPreview()}>
          {t("fs.preview")}
        </button>
      </div>
      {err && <div className="error-text">{err}</div>}
      {ok && <div className="muted" style={{ color: "var(--success)", marginBottom: 12 }}>{ok}</div>}
      {loading ? (
        <div className="muted">{t("common.loading")}</div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
          <div className="card" style={{ maxHeight: 520, overflow: "auto" }}>
            <table className="table">
              <tbody>
                {lines.map((l) => (
                  <tr
                    key={l.code}
                    onClick={() => pick(l.code)}
                    style={{
                      cursor: "pointer",
                      background: sel === l.code ? "var(--primary-soft)" : undefined,
                      fontWeight: l.kind === "header" || l.kind === "total" ? 600 : 400,
                    }}
                  >
                    <td style={{ paddingLeft: 8 + depthOf(l.code, byCode) * 14 }}>
                      {l.name}
                      {formulas[l.code] ? (
                        <div className="muted" style={{ fontFamily: "ui-monospace, monospace", fontSize: 12 }}>
                          {formulas[l.code]}
                        </div>
                      ) : null}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div>
            {selected && (
              <div className="card content">
                <h3 className="section-title">{selected.name}</h3>
                <p className="muted" style={{ fontFamily: "ui-monospace, monospace" }}>{selected.code}</p>
                <p className="muted">{t("fs.legend")}</p>
                <textarea
                  className="input"
                  rows={4}
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  placeholder="+202Д-241К"
                  style={{ fontFamily: "ui-monospace, monospace", width: "100%" }}
                />
                <div className="form-actions">
                  <button className="btn btn-primary" type="button" disabled={saving} onClick={() => void save()}>
                    {t("common.save")}
                  </button>
                </div>
              </div>
            )}
            {preview && (
              <div className="card content" style={{ marginTop: 16 }}>
                <h3 className="section-title">{t("fs.preview")}</h3>
                <div className="table-wrap">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>{t("fs.col.line")}</th>
                        <th className="num">{t("fs.col.current")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {preview.map((r) => (
                        <tr key={r.code} style={{ fontWeight: r.kind === "total" ? 600 : 400 }}>
                          <td>{r.name}</td>
                          <td className="num">{r.kind === "header" ? "" : r.current}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
