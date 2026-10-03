"use client";

// Справки — оборотна ведомост, главна книга, по кореспонденции
// (водещ Дт и водещ Кт), хронологичен журнал, по контрагент.
// Експорт PDF/XLSX/ODS през същия reportbaga път като печата на фактури.

import { useCallback, useEffect, useState } from "react";
import { api, downloadFile, ListResponse } from "../../lib/api";
import { formatBgDate, todayIso } from "../../lib/dates";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { IconButton } from "../../components/IconButton";
import {
  ChronoData,
  Counterpart,
  CorrData,
  FsData,
  Kind,
  LedgerData,
  TrialData,
  firmLine,
  isCp,
  isFs,
} from "./types";
import { ChronoTable, CorrSide, FirmTableRow, LedgerTables, TrialTable } from "./tables";

const TABS: { kind: Kind; key: string }[] = [
  { kind: "trial_balance", key: "reports.tab.trial" },
  { kind: "general_ledger", key: "reports.tab.ledger" },
  { kind: "correspondence_ledger", key: "reports.tab.corr" },
  { kind: "balance_sheet", key: "reports.tab.balance" },
  { kind: "income_statement", key: "reports.tab.pl" },
  { kind: "cash_flow", key: "reports.tab.cash" },
  { kind: "equity", key: "reports.tab.equity" },
  { kind: "chronological", key: "reports.tab.chrono" },
  { kind: "counterpart_trial", key: "reports.tab.cp_trial" },
  { kind: "counterpart_chrono", key: "reports.tab.cp_chrono" },
];

function yearStart(): string {
  return `${new Date().getFullYear()}-01-01`;
}

function qs(kind: Kind, from: string, to: string, account: string, cpid: string): string {
  const p = new URLSearchParams();
  p.set("kind", kind);
  p.set("from", from);
  p.set("to", to);
  if (account.trim()) p.set("account", account.trim());
  if (isCp(kind) && cpid) p.set("counterpart_id", cpid);
  return p.toString();
}

function ReportsInner() {
  const { t } = useI18n();
  const [kind, setKind] = useState<Kind>("trial_balance");
  const [from, setFrom] = useState(yearStart);
  const [to, setTo] = useState(todayIso);
  const [account, setAccount] = useState("");
  const [cpid, setCpid] = useState("");
  const [counterparts, setCounterparts] = useState<Counterpart[]>([]);
  const [trial, setTrial] = useState<TrialData | null>(null);
  const [chrono, setChrono] = useState<ChronoData | null>(null);
  const [ledger, setLedger] = useState<LedgerData | null>(null);
  const [corr, setCorr] = useState<CorrData | null>(null);
  const [fs, setFs] = useState<FsData | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    api
      .get<ListResponse<Counterpart>>("/v1/counterparts")
      .then((d) => setCounterparts(d.items ?? []))
      .catch(() => setCounterparts([]));
  }, []);

  const load = useCallback(async () => {
    if (isCp(kind) && !cpid) {
      setError(t("reports.need_counterpart"));
      return;
    }
    setLoading(true);
    setError("");
    setTrial(null);
    setChrono(null);
    setLedger(null);
    setCorr(null);
    setFs(null);
    try {
      const path = `/v1/reports?${qs(kind, from, to, account, cpid)}`;
      if (kind === "general_ledger") {
        setLedger(await api.get<LedgerData>(path));
      } else if (kind === "correspondence_ledger") {
        setCorr(await api.get<CorrData>(path));
      } else if (isFs(kind)) {
        setFs(await api.get<FsData>(path));
      } else if (kind === "chronological" || kind === "counterpart_chrono") {
        setChrono(await api.get<ChronoData>(path));
      } else {
        setTrial(await api.get<TrialData>(path));
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [kind, from, to, account, cpid, t]);

  const exportFmt = async (fmt: "pdf" | "xlsx" | "ods") => {
    if (isCp(kind) && !cpid) {
      setError(t("reports.need_counterpart"));
      return;
    }
    setBusy(fmt);
    setError("");
    try {
      const slug =
        kind === "trial_balance"
          ? "oborotna"
          : kind === "general_ledger"
            ? "glavna-kniga"
            : kind === "correspondence_ledger"
              ? "glavna-kniga-korespondencii"
              : kind === "balance_sheet"
                ? "balans"
                : kind === "income_statement"
                  ? "opr"
                  : kind === "cash_flow"
                    ? "parichen-potok"
                    : kind === "equity"
                      ? "sobstven-kapital"
              : kind === "chronological"
                ? "hronologichen"
                : kind === "counterpart_trial"
                  ? "oborotna-kontragent"
                  : "hronologichen-kontragent";
      const head = trial ?? chrono ?? ledger ?? corr ?? fs;
      const name = (head?.company_name || "").trim().replace(/[\\/:*?"<>|]+/g, "").replace(/\s+/g, "_");
      const eik = (head?.company_eik || "").trim();
      let fname = slug;
      if (name) fname = `${fname}-${name}`;
      if (eik) fname = `${fname}-${eik}`;
      await downloadFile(
        `/v1/reports/export?${qs(kind, from, to, account, cpid)}&format=${fmt}`,
        `${fname}-${from}-${to}.${fmt}`
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const hasData = !!(trial || chrono || ledger || corr || fs);

  return (
    <div>
      <div className="page-head no-print">
        <h1 className="page-title">{t("reports.title")}</h1>
      </div>
      <p className="muted no-print" style={{ margin: "0 0 12px" }}>
        {t("reports.hint")}
      </p>
      <div className="tabs tabs-wrap no-print">
        {TABS.map((tab) => (
          <button
            key={tab.kind}
            type="button"
            className={`tab${kind === tab.kind ? " tab-active" : ""}`}
            onClick={() => {
              setKind(tab.kind);
              setTrial(null);
              setChrono(null);
              setLedger(null);
              setCorr(null);
              setFs(null);
            }}
          >
            {t(tab.key)}
          </button>
        ))}
      </div>

      <div className="card content no-print" style={{ marginBottom: 18 }}>
        <div className="toolbar">
          <div className="field" style={{ margin: 0 }}>
            <label className="label">{t("reports.from")}</label>
            <input className="input" type="date" value={from} onChange={(e) => setFrom(e.target.value)} />
          </div>
          <div className="field" style={{ margin: 0 }}>
            <label className="label">{t("reports.to")}</label>
            <input className="input" type="date" value={to} onChange={(e) => setTo(e.target.value)} />
          </div>
          <div className="field" style={{ margin: 0 }}>
            <label className="label">{t("reports.account")}</label>
            <input
              className="input"
              value={account}
              onChange={(e) => setAccount(e.target.value)}
              placeholder="401"
            />
          </div>
          {isCp(kind) && (
            <div className="field" style={{ margin: 0, minWidth: 220 }}>
              <label className="label">{t("reports.counterpart")}</label>
              <select className="select" value={cpid} onChange={(e) => setCpid(e.target.value)}>
                <option value="">{t("reports.pick_counterpart")}</option>
                {counterparts.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </div>
          )}
          <button className="btn btn-primary" onClick={load} disabled={loading || !from || !to}>
            {t("reports.show")}
          </button>
        </div>
      </div>

      {error && <div className="error-text no-print">{error}</div>}
      {loading && <div className="muted">{t("common.loading")}</div>}

      {hasData && (
        <div className="btn-row wrap no-print" style={{ marginBottom: 12 }}>
          <IconButton icon="print" title={t("reports.print")} onClick={() => window.print()} />
          {(["pdf", "xlsx", "ods"] as const).map((fmt) => (
            <button
              key={fmt}
              type="button"
              className="btn btn-sm"
              disabled={!!busy}
              onClick={() => exportFmt(fmt)}
            >
              {fmt.toUpperCase()}
            </button>
          ))}
        </div>
      )}

      {trial && (kind === "trial_balance" || kind === "counterpart_trial") && (
        <div className="card content">
          {firmLine(trial) && (
            <p style={{ margin: "0 0 4px", fontWeight: 600 }}>{firmLine(trial)}</p>
          )}
          <h2 style={{ margin: "0 0 8px" }}>{trial.title}</h2>
          <p className="muted">
            {formatBgDate(trial.from)} — {formatBgDate(trial.to)}
          </p>
          <TrialTable data={trial} t={t} />
        </div>
      )}
      {chrono && (kind === "chronological" || kind === "counterpart_chrono") && (
        <div className="card content">
          {firmLine(chrono) && (
            <p style={{ margin: "0 0 4px", fontWeight: 600 }}>{firmLine(chrono)}</p>
          )}
          <h2 style={{ margin: "0 0 8px" }}>{chrono.title}</h2>
          <p className="muted">
            {formatBgDate(chrono.from)} — {formatBgDate(chrono.to)}
          </p>
          <ChronoTable data={chrono} t={t} />
        </div>
      )}
      {ledger && kind === "general_ledger" && (
        <div className="card content">
          {firmLine(ledger) && (
            <p style={{ margin: "0 0 4px", fontWeight: 600 }}>{firmLine(ledger)}</p>
          )}
          <h2 style={{ margin: "0 0 8px" }}>{ledger.title}</h2>
          <p className="muted">
            {formatBgDate(ledger.from)} — {formatBgDate(ledger.to)}
          </p>
          {(ledger.accounts ?? []).length === 0 ? (
            <div className="muted">{t("common.empty")}</div>
          ) : (
            <LedgerTables data={ledger} t={t} />
          )}
        </div>
      )}
      {fs && isFs(kind) && (
        <div className="card content">
          {firmLine(fs) && (
            <p style={{ margin: "0 0 4px", fontWeight: 600 }}>{firmLine(fs)}</p>
          )}
          <h2 style={{ margin: "0 0 8px" }}>{fs.title}</h2>
          <p className="muted">
            {formatBgDate(fs.from)} — {formatBgDate(fs.to)}
          </p>
          <div className="table-wrap print-sheet">
            <table className="table">
              <thead>
                <FirmTableRow d={fs} cols={3} />
                <tr>
                  <th>{t("fs.col.line")}</th>
                  <th className="num">{t("fs.col.current")}</th>
                  <th className="num">{t("reports.col.prior")}</th>
                </tr>
              </thead>
              <tbody>
                {(fs.rows ?? []).map((r) => (
                  <tr key={r.code} style={{ fontWeight: r.kind === "total" ? 600 : 400 }}>
                    <td>{r.name}</td>
                    <td className="num">{r.kind === "header" ? "" : r.current}</td>
                    <td className="num">{r.kind === "header" ? "" : r.prior}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
      {corr && kind === "correspondence_ledger" && (
        <div className="card content">
          {firmLine(corr) && (
            <p style={{ margin: "0 0 4px", fontWeight: 600 }}>{firmLine(corr)}</p>
          )}
          <h2 style={{ margin: "0 0 8px" }}>{corr.title}</h2>
          <p className="muted">
            {formatBgDate(corr.from)} — {formatBgDate(corr.to)}
          </p>
          <CorrSide
            title={t("reports.lead.debit")}
            accounts={corr.debit_lead ?? []}
            t={t}
            firm={corr}
            lead="debit"
          />
          <CorrSide
            title={t("reports.lead.credit")}
            accounts={corr.credit_lead ?? []}
            t={t}
            firm={corr}
            lead="credit"
          />
        </div>
      )}
    </div>
  );
}

export default function ReportsPage() {
  return (
    <RequireAuth>
      <ReportsInner />
    </RequireAuth>
  );
}
