"use client";

// SAF-T BG експорт — режимът определя периода: месечен → месец,
// годишен → година, при поискване → дати от-до за материалните запаси.

import { useState } from "react";
import Link from "next/link";
import { API_BASE, getToken } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";

const MODES = ["monthly", "annual", "ondemand"] as const;

function today() {
  return new Date().toISOString().slice(0, 10);
}

function SaftInner() {
  const { t } = useI18n();
  const [mode, setMode] = useState("monthly");
  const [month, setMonth] = useState(today().slice(0, 7));
  const [year, setYear] = useState(today().slice(0, 4));
  const [from, setFrom] = useState(`${today().slice(0, 7)}-01`);
  const [to, setTo] = useState(today());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const period =
    mode === "annual" ? `${year}-01-01` : mode === "monthly" ? `${month}-01` : `${to.slice(0, 7)}-01`;
  const ready =
    mode === "annual"
      ? /^\d{4}$/.test(year)
      : mode === "monthly"
        ? Boolean(month)
        : Boolean(from && to && from <= to);

  const handleDownload = async () => {
    if (!ready) return;
    setBusy(true);
    setError("");
    try {
      const token = getToken();
      const range = mode === "ondemand" ? `&date_from=${from}&date_to=${to}` : "";
      const res = await fetch(
        `${API_BASE}/v1/saft/export?period=${encodeURIComponent(period)}&mode=${mode}${range}`,
        { headers: token ? { Authorization: `Bearer ${token}` } : {} }
      );
      if (!res.ok) {
        const detail = await res.text();
        setError(detail || String(res.status));
        return;
      }
      const text = await res.text();
      const blob = new Blob([text], { type: "application/xml" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `saft_${period}_${mode}.xml`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("saft.title")}</h1>
      </div>

      <p className="muted" style={{ margin: "0 0 16px" }}>
        <Link href="/nomenclatures">{t("saft.link.noms")}</Link>
        {" · "}
        <Link href="/saft-movement-mappings">{t("saft.link.mappings")}</Link>
      </p>

      <div className="card content" style={{ marginBottom: 12 }}>
        <h2 className="invoice-section-title">{t("saft.mode")}</h2>
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
            gap: 12,
          }}
        >
          {MODES.map((m) => (
            <button
              key={m}
              type="button"
              className={mode === m ? "btn btn-primary" : "btn"}
              style={{
                padding: "14px 16px",
                height: "auto",
                display: "block",
                textAlign: "left",
                whiteSpace: "normal",
              }}
              onClick={() => setMode(m)}
            >
              <span style={{ display: "block", fontWeight: 600 }}>
                {t(`saft.mode.${m}`)}
              </span>
              <span
                style={{
                  display: "block",
                  fontSize: 12,
                  fontWeight: 400,
                  marginTop: 4,
                  opacity: 0.85,
                }}
              >
                {t(`saft.mode.${m}_hint`)}
              </span>
            </button>
          ))}
        </div>
      </div>

      <div className="card content">
        <h2 className="invoice-section-title">{t("saft.period")}</h2>
        <div className="form-grid">
          {mode === "monthly" && (
            <div className="field">
              <label className="label">{t("saft.period.month")}</label>
              <input
                className="input"
                type="month"
                value={month}
                onChange={(e) => setMonth(e.target.value)}
                required
              />
            </div>
          )}
          {mode === "annual" && (
            <div className="field">
              <label className="label">{t("saft.period.year")}</label>
              <input
                className="input"
                type="number"
                min={2000}
                max={2100}
                value={year}
                onChange={(e) => setYear(e.target.value)}
                required
              />
            </div>
          )}
          {mode === "ondemand" && (
            <>
              <div className="field">
                <label className="label">{t("saft.period.from")}</label>
                <input
                  className="input"
                  type="date"
                  value={from}
                  onChange={(e) => setFrom(e.target.value)}
                  required
                />
              </div>
              <div className="field">
                <label className="label">{t("saft.period.to")}</label>
                <input
                  className="input"
                  type="date"
                  value={to}
                  onChange={(e) => setTo(e.target.value)}
                  required
                />
              </div>
            </>
          )}
        </div>
        {error && <div className="error-text">{error}</div>}
        <div className="form-actions">
          <button
            className="btn btn-primary"
            onClick={handleDownload}
            disabled={busy || !ready}
          >
            {busy ? t("saft.downloading") : t("saft.download")}
          </button>
        </div>
      </div>
    </div>
  );
}

export default function SaftPage() {
  return (
    <RequireAuth>
      <SaftInner />
    </RequireAuth>
  );
}
