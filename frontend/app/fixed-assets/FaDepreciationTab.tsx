"use client";

import { DepPreview } from "./types";

export function FaDepreciationTab({
  t,
  depPeriod,
  setDepPeriod,
  handlePreview,
  handlePost,
  posting,
  preview,
}: {
  t: (k: string) => string;
  depPeriod: string;
  setDepPeriod: (v: string) => void;
  handlePreview: () => void;
  handlePost: () => void;
  posting: boolean;
  preview: DepPreview | null;
}) {
  return (
        <div className="card content">
          <div style={{ display: "flex", gap: 8, alignItems: "flex-end", marginBottom: 14 }}>
            <div className="field" style={{ margin: 0, flex: 1 }}>
              <label className="label">{t("vat.period")}</label>
              <input
                className="input"
                value={depPeriod}
                onChange={(e) => setDepPeriod(e.target.value)}
                placeholder="2026-09-01"
              />
            </div>
            <button className="btn" onClick={handlePreview} disabled={!depPeriod}>
              {t("fa.dep_preview")}
            </button>
            <button
              className="btn btn-primary"
              onClick={handlePost}
              disabled={!depPeriod || posting}
            >
              {t("fa.dep_post")}
            </button>
          </div>
          {preview && (
            <>
              <table className="table" style={{ maxWidth: 560 }}>
                <thead>
                  <tr>
                    <th>{t("fa.name")}</th>
                    <th>{t("fa.depreciation_method")}</th>
                    <th style={{ textAlign: "right" }}>{t("fa.dep_monthly")}</th>
                  </tr>
                </thead>
                <tbody>
                  {preview.items.map((it) => (
                    <tr key={it.id}>
                      <td>{it.name}</td>
                      <td>{t(`fa.depreciation_method.${it.method}`)}</td>
                      <td style={{ textAlign: "right" }}>{it.monthly_amount}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="muted" style={{ marginTop: 8 }}>
                {t("fa.dep_total")}: {preview.total_amount}
              </div>
            </>
          )}
        </div>
  );
}
