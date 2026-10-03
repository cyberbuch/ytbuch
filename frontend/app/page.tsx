"use client";

// Табло — начална страница след вход. Показва ключовите числа за активната
// фирма: чернови за довършване, вземания/задължения, ДДС, приходи/разходи
// за годината, наличности по банка и последните фактури.

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { api, ACTIVE_COMPANY_EVENT, Company, getActiveCompany } from "../lib/api";
import { Invoice, formatBgDate } from "../lib/invoice";
import { useI18n } from "../components/I18nProvider";
import { RequireAuth } from "../components/RequireAuth";

interface Dashboard {
  company_id: number;
  year: number;
  invoices_draft: number;
  invoices_posted: number;
  receivables: string;
  payables: string;
  vat_output: string;
  vat_input: string;
  revenue_ytd: string;
  expense_ytd: string;
  bank_total: string;
  recent: Invoice[];
}

// Число от backend-а ("1234.56") във вид за показване. Ако липсва → "0.00".
function money(v: string | undefined): string {
  if (!v) return "0.00";
  return v;
}

// Разлика (приходи − разходи) без библиотека: и двете са низове с 2 знака.
function diff(a: string | undefined, b: string | undefined): string {
  const x = Number(a ?? "0");
  const y = Number(b ?? "0");
  if (!Number.isFinite(x) || !Number.isFinite(y)) return "0.00";
  return (x - y).toFixed(2);
}

function DashboardInner() {
  const { t } = useI18n();
  const [data, setData] = useState<Dashboard | null>(null);
  const [company, setCompany] = useState<Company | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const d = await api.get<Dashboard>("/v1/dashboard");
      setData(d);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
    const onCompany = () => {
      getActiveCompany()
        .then((c) => setCompany("id" in c ? (c as Company) : null))
        .catch(() => setCompany(null));
      void load();
    };
    getActiveCompany()
      .then((c) => setCompany("id" in c ? (c as Company) : null))
      .catch(() => setCompany(null));
    window.addEventListener(ACTIVE_COMPANY_EVENT, onCompany);
    return () => window.removeEventListener(ACTIVE_COMPANY_EVENT, onCompany);
  }, [load]);

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("dashboard.title")}</h1>
        <div style={{ display: "flex", gap: 8 }}>
          <Link className="btn" href="/invoices">
            {t("invoices.title")}
          </Link>
          <Link className="btn btn-primary" href="/invoices/new">
            {t("invoices.new")}
          </Link>
        </div>
      </div>

      {company && (
        <div className="muted" style={{ marginBottom: 12 }}>
          {company.name} · {t("dashboard.year")} {data?.year ?? ""}
        </div>
      )}

      {error && <div className="error-text">{error}</div>}

      {loading ? (
        <div className="card content muted">{t("common.loading")}</div>
      ) : (
        <>
          <div className="summary-grid">
            <Link className="summary-box" href="/invoices?status=draft">
              <div className="summary-label">{t("dashboard.drafts")}</div>
              <div className="summary-value">{data?.invoices_draft ?? 0}</div>
            </Link>
            <Link className="summary-box" href="/invoices?status=posted">
              <div className="summary-label">{t("dashboard.posted")}</div>
              <div className="summary-value">{data?.invoices_posted ?? 0}</div>
            </Link>
            <Link className="summary-box" href="/invoices?direction=out">
              <div className="summary-label">{t("dashboard.receivables")}</div>
              <div className="summary-value">{money(data?.receivables)}</div>
            </Link>
            <Link className="summary-box" href="/invoices?direction=in">
              <div className="summary-label">{t("dashboard.payables")}</div>
              <div className="summary-value">{money(data?.payables)}</div>
            </Link>
          </div>

          <div className="summary-grid">
            <div className="summary-box">
              <div className="summary-label">{t("dashboard.revenue")}</div>
              <div className="summary-value">{money(data?.revenue_ytd)}</div>
            </div>
            <div className="summary-box">
              <div className="summary-label">{t("dashboard.expense")}</div>
              <div className="summary-value">{money(data?.expense_ytd)}</div>
            </div>
            <div className="summary-box">
              <div className="summary-label">{t("dashboard.profit")}</div>
              <div className="summary-value">
                {diff(data?.revenue_ytd, data?.expense_ytd)}
              </div>
            </div>
            <div className="summary-box">
              <div className="summary-label">{t("dashboard.bank")}</div>
              <div className="summary-value">{money(data?.bank_total)}</div>
            </div>
          </div>

          <div className="summary-grid">
            <div className="summary-box">
              <div className="summary-label">{t("dashboard.vat_output")}</div>
              <div className="summary-value">{money(data?.vat_output)}</div>
            </div>
            <div className="summary-box">
              <div className="summary-label">{t("dashboard.vat_input")}</div>
              <div className="summary-value">{money(data?.vat_input)}</div>
            </div>
            <div className="summary-box">
              <div className="summary-label">{t("dashboard.vat_due")}</div>
              <div className="summary-value">
                {diff(data?.vat_output, data?.vat_input)}
              </div>
            </div>
            <Link className="summary-box" href="/vat">
              <div className="summary-label">{t("nav.vat")}</div>
              <div className="summary-value">→</div>
            </Link>
          </div>

          <div className="card" style={{ marginTop: 16 }}>
            <div className="content">
              <div className="btn-row" style={{ marginBottom: 8 }}>
                <strong>{t("dashboard.recent")}</strong>
                <Link className="btn btn-sm" href="/invoices">
                  {t("dashboard.all_invoices")}
                </Link>
              </div>
              {(data?.recent ?? []).length === 0 ? (
                <div className="muted">{t("common.empty")}</div>
              ) : (
                <div className="table-wrap">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>{t("invoices.number")}</th>
                        <th>{t("invoices.issue_date")}</th>
                        <th>{t("invoices.counterpart")}</th>
                        <th>{t("invoices.total")}</th>
                        <th>{t("invoices.status")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {(data?.recent ?? []).map((inv) => (
                        <tr key={inv.id}>
                          <td>
                            <Link href={`/invoices/${inv.id}`}>{inv.number}</Link>
                          </td>
                          <td>{formatBgDate(inv.issue_date)}</td>
                          <td>{inv.counterpart_name || inv.counterpart_id}</td>
                          <td>
                            {inv.total_amount} {inv.currency}
                          </td>
                          <td>{t(`invoices.status.${inv.status}`)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

export default function DashboardPage() {
  return (
    <RequireAuth>
      <DashboardInner />
    </RequireAuth>
  );
}
