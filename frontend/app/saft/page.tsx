"use client";

// SAF-T BG експорт — изтегляне на XML за периода (месечен/при поискване/годишен).

import { useState } from "react";
import Link from "next/link";
import { API_BASE, getToken } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";

function SaftInner() {
  const { t } = useI18n();
  const [period, setPeriod] = useState("");
  const [mode, setMode] = useState("monthly");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const handleDownload = async () => {
    if (!period) return;
    setBusy(true);
    setError("");
    try {
      const token = getToken();
      const res = await fetch(
        `${API_BASE}/v1/saft/export?period=${encodeURIComponent(period)}&mode=${mode}`,
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

      <div className="card content">
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("saft.period")}</label>
            <input
              className="input"
              value={period}
              onChange={(e) => setPeriod(e.target.value)}
              placeholder="2026-09-01"
            />
          </div>
          <div className="field">
            <label className="label">{t("saft.mode")}</label>
            <select className="select" value={mode} onChange={(e) => setMode(e.target.value)}>
              <option value="monthly">{t("saft.mode.monthly")}</option>
              <option value="ondemand">{t("saft.mode.ondemand")}</option>
              <option value="annual">{t("saft.mode.annual")}</option>
            </select>
          </div>
        </div>
        {error && <div className="error-text">{error}</div>}
        <div className="form-actions">
          <button
            className="btn btn-primary"
            onClick={handleDownload}
            disabled={busy || !period}
          >
            {t("saft.download")}
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
