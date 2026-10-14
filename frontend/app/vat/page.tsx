"use client";

// ДДС дневници и ППДДС — клетки 01-01..01-82 и файлове за НАП (Windows-1251).
// Данните идват от счетоводните записи с ДДС тип (ръчни + постнати фактури).

import { useCallback, useEffect, useMemo, useState } from "react";
import { api, downloadFile } from "../../lib/api";
import { formatBgDate } from "../../lib/dates";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { Pagination } from "../../components/Pagination";

const PAGE_SIZE = 50;

type Tab = "summary" | "purchases" | "sales" | "declaration" | "vies";

interface VatRow {
  id: number;
  row: number;
  document_type: string;
  document_number: string;
  document_date: string;
  counterpart_name: string;
  counterpart_vat_number: string;
  goods: string;
  base_no_credit: string;
  base_full_credit: string;
  vat_full_credit: string;
  total_base: string;
  total_vat: string;
  base_20: string;
  vat_20: string;
  base_9: string;
  vat_9: string;
  base_vod: string;
  base_exempt: string;
  base_amount: string;
  vat_amount: string;
}

interface VatPack {
  period: string;
  period_nap: string;
  company_name: string;
  company_vat: string;
  submitter: string;
  sales_count: number;
  purchase_count: number;
  purchases: VatRow[];
  sales: VatRow[];
  cells: Record<string, string>;
  due: string;
  refund: string;
  vies: {
    vat_number: string;
    country: string;
    base_vod: string;
    base_tri: string;
    base_services: string;
    total: string;
  }[];
}

function prevPeriod(): { year: string; month: string } {
  const d = new Date();
  d.setDate(1);
  d.setMonth(d.getMonth() - 1);
  return { year: String(d.getFullYear()), month: String(d.getMonth() + 1).padStart(2, "0") };
}

function money(raw: string | undefined): string {
  const n = Number(String(raw ?? "").replace(",", "."));
  if (!Number.isFinite(n)) return raw || "0,00";
  return n.toLocaleString("bg-BG", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

const SECTION_A: string[] = [
  "01-01", "01-20", "01-11", "01-21", "01-12", "01-22", "01-23",
  "01-13", "01-24", "01-14", "01-15", "01-16", "01-17", "01-18", "01-19",
];
const SECTION_B: string[] = ["01-30", "01-31", "01-41", "01-32", "01-42", "01-43"];
const SECTION_C: string[] = [
  "01-33", "01-40", "01-50", "01-60", "01-70", "01-71", "01-80", "01-81", "01-82",
];

function cellKey(code: string): string {
  return `vat.c_${code.replace("-", "_")}`;
}

function CellTable({
  codes,
  cells,
  t,
}: {
  codes: string[];
  cells: Record<string, string>;
  t: (k: string) => string;
}) {
  return (
    <div className="table-wrap">
      <table className="table">
        <tbody>
          {codes.map((code) => (
            <tr key={code}>
              <td>{t(cellKey(code))}</td>
              <td className="num">{money(cells[code])}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function sumField<T>(rows: T[], field: keyof T): number {
  return rows.reduce((acc, r) => {
    const n = Number(String(r[field] ?? "").replace(",", "."));
    return acc + (Number.isFinite(n) ? n : 0);
  }, 0);
}

const MANUAL_CELLS: { code: string; key: string }[] = [
  { code: "01-33", key: "cell_33" },
  { code: "01-70", key: "cell_70" },
  { code: "01-71", key: "cell_71" },
  { code: "01-80", key: "cell_80" },
  { code: "01-81", key: "cell_81" },
  { code: "01-82", key: "cell_82" },
];

function VatInner() {
  const { t } = useI18n();
  const initial = useMemo(() => prevPeriod(), []);
  const [year, setYear] = useState(initial.year);
  const [month, setMonth] = useState(initial.month);
  const [tab, setTab] = useState<Tab>("summary");
  const [data, setData] = useState<VatPack | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [manual, setManual] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [purOffset, setPurOffset] = useState(0);
  const [salOffset, setSalOffset] = useState(0);

  const period = `${year}-${month}`;

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const r = await api.get<VatPack>(`/v1/vat/registers?period=${encodeURIComponent(period)}`);
      setData(r);
      const m: Record<string, string> = {};
      for (const { code } of MANUAL_CELLS) {
        m[code] = String(r.cells?.[code] ?? "0.00");
      }
      setManual(m);
      setSaved(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setData(null);
    } finally {
      setLoading(false);
    }
  }, [period]);

  useEffect(() => {
    void load();
  }, [load]);

  const saveManual = async () => {
    setSaving(true);
    setError("");
    setSaved(false);
    try {
      const body: Record<string, string> = {};
      for (const { code, key } of MANUAL_CELLS) {
        body[key] = manual[code] ?? "0.00";
      }
      await api.put(`/v1/vat/manual?period=${encodeURIComponent(period)}`, body);
      setSaved(true);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const download = async (type: string, filename: string) => {
    setError("");
    try {
      await downloadFile(
        `/v1/vat/export?period=${encodeURIComponent(period)}&type=${encodeURIComponent(type)}`,
        filename
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const viesRows = data?.vies ?? [];
  const dueN = Number(data?.due ?? "0");
  const refundN = Number(data?.refund ?? "0");

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("vat.title")}</h1>
      </div>
      <p className="muted" style={{ margin: "0 0 12px" }}>
        {t("vat.hint")}
      </p>

      <div className="card content" style={{ marginBottom: 18 }}>
        <div className="toolbar">
          <div className="field" style={{ margin: 0 }}>
            <label className="label">{t("vat.month")}</label>
            <select className="input" value={month} onChange={(e) => setMonth(e.target.value)}>
              {Array.from({ length: 12 }, (_, i) => {
                const m = String(i + 1).padStart(2, "0");
                return (
                  <option key={m} value={m}>
                    {m} — {t(`per.m${i + 1}`)}
                  </option>
                );
              })}
            </select>
          </div>
          <div className="field" style={{ margin: 0 }}>
            <label className="label">{t("vat.year")}</label>
            <input
              className="input"
              value={year}
              onChange={(e) => setYear(e.target.value.replace(/\D/g, "").slice(0, 4))}
              inputMode="numeric"
              style={{ width: 100 }}
            />
          </div>
          <button className="btn btn-primary" onClick={() => void load()} disabled={loading || year.length !== 4}>
            {t("vat.show")}
          </button>
          {data && (
            <>
              <button className="btn" onClick={() => void download("zip", `PPDDS_${data.period_nap}.zip`)}>
                {t("vat.download_zip")}
              </button>
              <button className="btn" onClick={() => void download("deklar", "DEKLAR.TXT")}>
                {t("vat.download_deklar")}
              </button>
              <button className="btn" onClick={() => void download("pokupki", "POKUPKI.TXT")}>
                {t("vat.download_pokupki")}
              </button>
              <button className="btn" onClick={() => void download("prodagbi", "PRODAGBI.TXT")}>
                {t("vat.download_prodagbi")}
              </button>
              <button className="btn" onClick={() => void download("vies", `VIES_${data.period_nap}.TXT`)}>
                {t("vat.download_vies")}
              </button>
            </>
          )}
        </div>
      </div>

      {error && <div className="error-text">{error}</div>}
      {loading && <div className="muted">{t("common.loading")}</div>}

      {data && (
        <>
          <div className="tabs tabs-wrap">
            {(["summary", "purchases", "sales", "declaration", "vies"] as Tab[]).map((k) => (
              <button
                key={k}
                type="button"
                className={`tab${tab === k ? " tab-active" : ""}`}
                onClick={() => setTab(k)}
              >
                {t(`vat.tab.${k}`)}
              </button>
            ))}
          </div>

          {tab === "summary" && (
            <>
              <div className="card content" style={{ marginBottom: 18 }}>
                <div className="muted">{t("vat.company")}</div>
                <div style={{ fontWeight: 650, marginBottom: 8 }}>
                  {data.company_name} · {data.company_vat || "—"}
                </div>
                {data.submitter && (
                  <div className="muted">
                    {t("vat.submitter")}: {data.submitter}
                  </div>
                )}
              </div>
              <div className="card content" style={{ marginBottom: 18 }}>
                <table className="table" style={{ maxWidth: 520 }}>
                  <tbody>
                    <tr>
                      <td>{t("vat.sales_count")}</td>
                      <td className="num">{data.sales_count}</td>
                    </tr>
                    <tr>
                      <td>{t("vat.purchase_count")}</td>
                      <td className="num">{data.purchase_count}</td>
                    </tr>
                    <tr>
                      <td>{t("vat.c_01_20")}</td>
                      <td className="num">{money(data.cells["01-20"])}</td>
                    </tr>
                    <tr>
                      <td>{t("vat.c_01_40")}</td>
                      <td className="num">{money(data.cells["01-40"])}</td>
                    </tr>
                    <tr>
                      <td>{t("vat.due")}</td>
                      <td className="num" style={{ fontWeight: dueN > 0 ? 700 : 400 }}>
                        {money(data.due)}
                      </td>
                    </tr>
                    <tr>
                      <td>{t("vat.refund")}</td>
                      <td className="num" style={{ fontWeight: refundN > 0 ? 700 : 400 }}>
                        {money(data.refund)}
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </>
          )}

          {tab === "purchases" && (() => {
            const pCount = Math.max(1, Math.ceil(data.purchases.length / PAGE_SIZE));
            const pPage = Math.min(Math.floor(purOffset / PAGE_SIZE), pCount - 1);
            const pStart = pPage * PAGE_SIZE;
            const pagedPur = data.purchases.slice(pStart, pStart + PAGE_SIZE);
            return (
              <div className="card" style={{ marginBottom: 18 }}>
                <div className="content">
                  <h3 style={{ margin: "0 0 10px" }}>{t("vat.purchases")}</h3>
                  {data.purchases.length === 0 ? (
                    <div className="muted">{t("common.empty")}</div>
                  ) : (
                    <div className="table-wrap">
                      <table className="table">
                        <thead>
                          <tr>
                            <th>{t("vat.row")}</th>
                            <th>{t("vat.doc_type")}</th>
                            <th>{t("vat.document")}</th>
                            <th>{t("vat.date")}</th>
                            <th>{t("vat.vat_number")}</th>
                            <th>{t("vat.counterpart")}</th>
                            <th>{t("vat.goods")}</th>
                            <th className="num">{t("vat.col_no_credit")}</th>
                            <th className="num">{t("vat.col_full_base")}</th>
                            <th className="num">{t("vat.col_full_vat")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {pagedPur.map((r) => (
                            <tr key={r.id}>
                              <td>{r.row}</td>
                              <td>{r.document_type}</td>
                              <td>{r.document_number}</td>
                              <td>{formatBgDate(r.document_date)}</td>
                              <td>{r.counterpart_vat_number}</td>
                              <td>{r.counterpart_name}</td>
                              <td>{r.goods}</td>
                              <td className="num">{money(r.base_no_credit)}</td>
                              <td className="num">{money(r.base_full_credit)}</td>
                              <td className="num">{money(r.vat_full_credit)}</td>
                            </tr>
                          ))}
                        </tbody>
                        <tfoot>
                          <tr>
                            <td colSpan={7}>{t("vat.total")}</td>
                            <td className="num">{money(String(sumField(data.purchases, "base_no_credit")))}</td>
                            <td className="num">{money(String(sumField(data.purchases, "base_full_credit")))}</td>
                            <td className="num">{money(String(sumField(data.purchases, "vat_full_credit")))}</td>
                          </tr>
                        </tfoot>
                      </table>
                    </div>
                  )}
                </div>
                <Pagination
                  total={data.purchases.length}
                  limit={PAGE_SIZE}
                  offset={pStart}
                  onPageChange={setPurOffset}
                />
              </div>
            );
          })()}

          {tab === "sales" && (() => {
            const sCount = Math.max(1, Math.ceil(data.sales.length / PAGE_SIZE));
            const sPage = Math.min(Math.floor(salOffset / PAGE_SIZE), sCount - 1);
            const sStart = sPage * PAGE_SIZE;
            const pagedSal = data.sales.slice(sStart, sStart + PAGE_SIZE);
            return (
              <div className="card" style={{ marginBottom: 18 }}>
                <div className="content">
                  <h3 style={{ margin: "0 0 10px" }}>{t("vat.sales")}</h3>
                  {data.sales.length === 0 ? (
                    <div className="muted">{t("common.empty")}</div>
                  ) : (
                    <div className="table-wrap">
                      <table className="table">
                        <thead>
                          <tr>
                            <th>{t("vat.row")}</th>
                            <th>{t("vat.doc_type")}</th>
                            <th>{t("vat.document")}</th>
                            <th>{t("vat.date")}</th>
                            <th>{t("vat.vat_number")}</th>
                            <th>{t("vat.counterpart")}</th>
                            <th>{t("vat.goods")}</th>
                            <th className="num">{t("vat.base")}</th>
                            <th className="num">{t("vat.vat")}</th>
                            <th className="num">{t("vat.col_base20")}</th>
                            <th className="num">{t("vat.col_vat20")}</th>
                            <th className="num">{t("vat.col_base9")}</th>
                            <th className="num">{t("vat.col_vat9")}</th>
                            <th className="num">{t("vat.col_vod")}</th>
                            <th className="num">{t("vat.col_exempt")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {pagedSal.map((r) => (
                            <tr key={r.id}>
                              <td>{r.row}</td>
                              <td>{r.document_type}</td>
                              <td>{r.document_number}</td>
                              <td>{formatBgDate(r.document_date)}</td>
                              <td>{r.counterpart_vat_number}</td>
                              <td>{r.counterpart_name}</td>
                              <td>{r.goods}</td>
                              <td className="num">{money(r.total_base)}</td>
                              <td className="num">{money(r.total_vat)}</td>
                              <td className="num">{money(r.base_20)}</td>
                              <td className="num">{money(r.vat_20)}</td>
                              <td className="num">{money(r.base_9)}</td>
                              <td className="num">{money(r.vat_9)}</td>
                              <td className="num">{money(r.base_vod)}</td>
                              <td className="num">{money(r.base_exempt)}</td>
                            </tr>
                          ))}
                        </tbody>
                        <tfoot>
                          <tr>
                            <td colSpan={7}>{t("vat.total")}</td>
                            <td className="num">{money(String(sumField(data.sales, "total_base")))}</td>
                            <td className="num">{money(String(sumField(data.sales, "total_vat")))}</td>
                            <td className="num">{money(String(sumField(data.sales, "base_20")))}</td>
                            <td className="num">{money(String(sumField(data.sales, "vat_20")))}</td>
                            <td className="num">{money(String(sumField(data.sales, "base_9")))}</td>
                            <td className="num">{money(String(sumField(data.sales, "vat_9")))}</td>
                            <td className="num">{money(String(sumField(data.sales, "base_vod")))}</td>
                            <td className="num">{money(String(sumField(data.sales, "base_exempt")))}</td>
                          </tr>
                        </tfoot>
                      </table>
                    </div>
                  )}
                </div>
                <Pagination
                  total={data.sales.length}
                  limit={PAGE_SIZE}
                  offset={sStart}
                  onPageChange={setSalOffset}
                />
              </div>
            );
          })()}

          {tab === "declaration" && (
            <>
              <div className="card content" style={{ marginBottom: 18 }}>
                <h3 className="section-title">{t("vat.section_a")}</h3>
                <CellTable codes={SECTION_A} cells={data.cells} t={t} />
              </div>
              <div className="card content" style={{ marginBottom: 18 }}>
                <h3 className="section-title">{t("vat.section_b")}</h3>
                <CellTable codes={SECTION_B} cells={data.cells} t={t} />
              </div>
              <div className="card content" style={{ marginBottom: 18 }}>
                <h3 className="section-title">{t("vat.section_c")}</h3>
                <CellTable codes={SECTION_C} cells={data.cells} t={t} />
              </div>
              <div className="card content" style={{ marginBottom: 18 }}>
                <h3 className="section-title">{t("vat.manual_cells")}</h3>
                <p className="muted" style={{ marginTop: 0 }}>{t("vat.manual_hint")}</p>
                <div className="table-wrap">
                  <table className="table">
                    <tbody>
                      {MANUAL_CELLS.map(({ code }) => (
                        <tr key={code}>
                          <td>{t(cellKey(code))}</td>
                          <td className="num" style={{ width: 160 }}>
                            <input
                              className="input"
                              value={manual[code] ?? ""}
                              onChange={(e) => {
                                setManual((prev) => ({ ...prev, [code]: e.target.value }));
                                setSaved(false);
                              }}
                              inputMode="decimal"
                            />
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <div className="toolbar" style={{ marginTop: 12 }}>
                  <button className="btn btn-primary" onClick={() => void saveManual()} disabled={saving}>
                    {t("common.save")}
                  </button>
                  {saved && <span className="muted">{t("common.saved")}</span>}
                </div>
              </div>
            </>
          )}

          {tab === "vies" && (
            <div className="card content" style={{ marginBottom: 18 }}>
              <h3 style={{ margin: "0 0 10px" }}>{t("vat.tab.vies")}</h3>
              <p className="muted">{t("vat.vies_hint")}</p>
              {viesRows.length === 0 ? (
                <div className="muted">{t("vat.vies_empty")}</div>
              ) : (
                <div className="table-wrap">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>{t("vat.country")}</th>
                        <th>{t("vat.vat_number")}</th>
                        <th className="num">{t("vat.col_vod")}</th>
                        <th className="num">{t("vat.col_tri")}</th>
                        <th className="num">{t("vat.col_services")}</th>
                        <th className="num">{t("vat.total")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {viesRows.map((r) => (
                        <tr key={r.vat_number}>
                          <td>{r.country}</td>
                          <td>{r.vat_number}</td>
                          <td className="num">{money(r.base_vod)}</td>
                          <td className="num">{money(r.base_tri)}</td>
                          <td className="num">{money(r.base_services)}</td>
                          <td className="num">{money(r.total)}</td>
                        </tr>
                      ))}
                    </tbody>
                    <tfoot>
                      <tr>
                        <td colSpan={2}>{t("vat.total")}</td>
                        <td className="num">{money(String(sumField(viesRows, "base_vod")))}</td>
                        <td className="num">{money(String(sumField(viesRows, "base_tri")))}</td>
                        <td className="num">{money(String(sumField(viesRows, "base_services")))}</td>
                        <td className="num">{money(String(sumField(viesRows, "total")))}</td>
                      </tr>
                    </tfoot>
                  </table>
                </div>
              )}
              <div className="toolbar" style={{ marginTop: 12 }}>
                <button className="btn btn-primary" onClick={() => void download("vies", `VIES_${data?.period_nap ?? "period"}.TXT`)}>
                  {t("vat.download_vies")}
                </button>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default function VatPage() {
  return (
    <RequireAuth>
      <VatInner />
    </RequireAuth>
  );
}
