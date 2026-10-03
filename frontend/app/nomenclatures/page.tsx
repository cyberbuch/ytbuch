"use client";

// SAF-T BG номенклатури — официални кодове на НАП (V1.0.1).
// Каталог, не таблица: същото като мерни единици.

import { useCallback, useEffect, useState } from "react";
import { api, ListResponse } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";

const NOM_KINDS = [
  "stock_movements",
  "asset_movements",
  "tax_regimes",
  "payment_methods",
  "invoice_types",
  "tax_types",
  "tax_codes",
  "product_types",
  "units",
  "regions",
  "accounts",
] as const;

type NomKind = (typeof NOM_KINDS)[number];

type Col = { key: string; labelKey: string; mono?: boolean };

const COLS: Record<NomKind, Col[]> = {
  stock_movements: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
    { key: "name_en", labelKey: "saft.col.name_en" },
  ],
  asset_movements: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
    { key: "name_en", labelKey: "saft.col.name_en" },
  ],
  tax_regimes: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
    { key: "name_en", labelKey: "saft.col.name_en" },
  ],
  payment_methods: [
    { key: "method_code", labelKey: "saft.col.method", mono: true },
    { key: "mechanism_code", labelKey: "saft.col.mechanism", mono: true },
    { key: "name", labelKey: "saft.col.name" },
    { key: "name_en", labelKey: "saft.col.name_en" },
  ],
  invoice_types: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
    { key: "name_en", labelKey: "saft.col.name_en" },
  ],
  tax_types: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
  ],
  tax_codes: [
    { key: "tax_type", labelKey: "saft.col.tax_type", mono: true },
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "description", labelKey: "saft.col.description" },
    { key: "rate", labelKey: "saft.col.rate" },
    { key: "note", labelKey: "saft.col.note" },
  ],
  product_types: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
    { key: "name_en", labelKey: "saft.col.name_en" },
  ],
  units: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
    { key: "name_en", labelKey: "saft.col.name_en" },
    { key: "symbol", labelKey: "saft.col.symbol" },
    { key: "category", labelKey: "saft.col.category" },
    { key: "source", labelKey: "saft.col.source" },
  ],
  regions: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
  ],
  accounts: [
    { key: "code", labelKey: "saft.col.code", mono: true },
    { key: "name", labelKey: "saft.col.name" },
    { key: "section", labelKey: "saft.col.section", mono: true },
    { key: "section_name", labelKey: "saft.col.section_name" },
    { key: "group", labelKey: "saft.col.group", mono: true },
    { key: "group_name", labelKey: "saft.col.group_name" },
  ],
};

function cell(rec: Record<string, unknown>, key: string): string {
  const v = rec[key];
  if (v === null || v === undefined) return "";
  return String(v);
}

function NomenclaturesInner() {
  const { t } = useI18n();
  const [kind, setKind] = useState<NomKind>("stock_movements");
  const [q, setQ] = useState("");
  const [rows, setRows] = useState<Record<string, unknown>[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const qs = new URLSearchParams({ kind });
      if (q.trim()) qs.set("search", q.trim());
      const data = await api.get<ListResponse<Record<string, unknown>>>(
        `/v1/saft/nomenclatures?${qs.toString()}`
      );
      setRows(data.items ?? []);
      setTotal(typeof data.count === "number" ? data.count : (data.items ?? []).length);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setRows([]);
      setTotal(0);
    } finally {
      setLoading(false);
    }
  }, [kind, q]);

  useEffect(() => {
    const tmr = window.setTimeout(() => {
      load();
    }, 180);
    return () => window.clearTimeout(tmr);
  }, [load]);

  const cols = COLS[kind];

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("saft.noms.title")}</h1>
      </div>
      <p className="muted" style={{ margin: "0 0 12px" }}>
        {t("saft.noms.hint")}
      </p>
      <div className="tabs tabs-wrap">
        {NOM_KINDS.map((k) => (
          <button
            key={k}
            type="button"
            className={`tab${kind === k ? " tab-active" : ""}`}
            onClick={() => setKind(k)}
          >
            {t(`saft.nom.${k}`)}
          </button>
        ))}
      </div>
      <div className="toolbar" style={{ marginBottom: 12 }}>
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t("common.search")}
          style={{ maxWidth: 360 }}
        />
        <span className="muted">
          {t("saft.noms.count").replace("{n}", String(total))}
        </span>
      </div>
      {error && <div className="error-text">{error}</div>}
      <div className="card">
        {loading && rows.length === 0 ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : rows.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  {cols.map((c) => (
                    <th key={c.key}>{t(c.labelKey)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((rec, i) => (
                  <tr key={cell(rec, "code") + cell(rec, "method_code") + String(i)}>
                    {cols.map((c) => (
                      <td
                        key={c.key}
                        style={c.mono ? { fontFamily: "ui-monospace, monospace" } : undefined}
                      >
                        {cell(rec, c.key)}
                      </td>
                    ))}
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

export default function NomenclaturesPage() {
  return (
    <RequireAuth>
      <NomenclaturesInner />
    </RequireAuth>
  );
}
