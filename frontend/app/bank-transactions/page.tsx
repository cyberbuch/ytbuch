"use client";

// Банкови транзакции: импорт от файл (преглед + дубликати), ръчно
// осчетоводяване и разнасяне с търсене на контрагент и сметка — като
// secret/baraba. Сумите остават десимални низове.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { api, fetchAll } from "../../lib/api";
import {
  Account,
  BankAccount,
  BankTransaction,
  Counterpart,
  JeLine,
  Preview,
} from "./types";
import { fmtAmount, isPositive, matchCounterpart } from "./helpers";
import { BankTxModals } from "./BankTxModals";
import { BankRulesPanel } from "./BankRulesPanel";
import { Pagination } from "../../components/Pagination";

interface ProcDetail {
  id: number;
  status: string;
  rule?: string;
  account?: string;
  counterpart?: string;
  amount?: string;
  text?: string;
  error?: string;
}
interface ProcResult {
  matched: number;
  unmatched: number;
  errors: number;
  details: ProcDetail[];
}

const PAGE_SIZE = 50;

function BankTransactionsInner() {
  const { t } = useI18n();
  const [accounts, setAccounts] = useState<BankAccount[]>([]);
  const [chartAccounts, setChartAccounts] = useState<Account[]>([]);
  const [counterparts, setCounterparts] = useState<Counterpart[]>([]);
  const [rows, setRows] = useState<BankTransaction[]>([]);
  const [selectedAccount, setSelectedAccount] = useState("");
  const [filterStatus, setFilterStatus] = useState("");
  const [q, setQ] = useState("");
  const [tab, setTab] = useState<"tx" | "rules">("tx");
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [selected, setSelected] = useState<number[]>([]);
  const [bulkBusy, setBulkBusy] = useState(false);
  const [procPreview, setProcPreview] = useState<ProcResult | null>(null);
  const [procBusy, setProcBusy] = useState(false);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");

  const [preview, setPreview] = useState<Preview | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [pendingContent, setPendingContent] = useState("");
  const [importing, setImporting] = useState(false);

  const [bookTx, setBookTx] = useState<BankTransaction | null>(null);
  const [bookContra, setBookContra] = useState(0);
  const [bookDebit, setBookDebit] = useState(0);
  const [bookCredit, setBookCredit] = useState(0);
  const [bookCpId, setBookCpId] = useState(0);
  const [bookCpSearch, setBookCpSearch] = useState("");
  const [bookAccSearch, setBookAccSearch] = useState("");
  const [bookLoading, setBookLoading] = useState(false);

  const [reallocTx, setReallocTx] = useState<BankTransaction | null>(null);
  const [reallocAccount, setReallocAccount] = useState(0);
  const [reallocCpId, setReallocCpId] = useState(0);
  const [reallocCpSearch, setReallocCpSearch] = useState("");
  const [reallocAccSearch, setReallocAccSearch] = useState("");
  const [reallocLines, setReallocLines] = useState<JeLine[]>([]);
  const [reallocLoading, setReallocLoading] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);

  const loadMeta = useCallback(async () => {
    try {
      const [b, a, c] = await Promise.all([
        fetchAll<BankAccount>("/v1/bank-accounts"),
        fetchAll<Account>("/v1/accounts"),
        fetchAll<Counterpart>("/v1/counterparts"),
      ]);
      setAccounts(b);
      setChartAccounts(a);
      setCounterparts(c);
    } catch {
      /* silent */
    }
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const extra: Record<string, string | undefined> = {};
      if (selectedAccount) extra.bank_account_id = selectedAccount;
      if (filterStatus) extra.status = filterStatus;
      setRows(await fetchAll<BankTransaction>("/v1/bank-transactions", extra));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [selectedAccount, filterStatus]);

  useEffect(() => {
    loadMeta();
  }, [loadMeta]);

  useEffect(() => {
    load();
  }, [load]);

  const filteredRows = useMemo(() => {
    const s = q.trim().toLowerCase();
    return rows.filter((tx) => {
      if (dateFrom && tx.transaction_date < dateFrom) return false;
      if (dateTo && tx.transaction_date > dateTo) return false;
      if (!s) return true;
      return [tx.description, tx.counterpart_name, tx.reference, tx.amount, tx.transaction_date]
        .join(" ")
        .toLowerCase()
        .includes(s);
    });
  }, [rows, q, dateFrom, dateTo]);

  const pageCount = Math.max(1, Math.ceil(filteredRows.length / PAGE_SIZE));
  const page = Math.min(Math.floor(offset / PAGE_SIZE), pageCount - 1);
  const start = page * PAGE_SIZE;
  const pagedRows = filteredRows.slice(start, start + PAGE_SIZE);

  const toggleSel = (id: number, on: boolean) =>
    setSelected((p) => (on ? [...p, id] : p.filter((x) => x !== id)));
  const allSelected = pagedRows.length > 0 && pagedRows.every((tx) => selected.includes(tx.id));
  const toggleAll = (on: boolean) =>
    setSelected((p) =>
      on
        ? Array.from(new Set([...p, ...pagedRows.map((tx) => tx.id)]))
        : p.filter((id) => !pagedRows.some((tx) => tx.id === id))
    );

  const handleBulkDelete = async () => {
    if (selected.length === 0) return;
    if (!window.confirm(t("bank_tx.bulk_confirm"))) return;
    setBulkBusy(true);
    setError("");
    try {
      const r = await api.post<{ deleted: number; failed: number }>("/v1/bank-transactions/bulk/delete", { ids: selected });
      setSuccess(`${t("bank_tx.bulk_deleted")}: ${r.deleted}, ${t("bank_tx.bulk_failed")}: ${r.failed}`);
      setSelected([]);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBulkBusy(false);
    }
  };

  const openProcess = async () => {
    setProcBusy(true);
    setError("");
    try {
      const r = await api.post<ProcResult>("/v1/bank-transactions/auto-allocate/preview", {});
      setProcPreview(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setProcBusy(false);
    }
  };

  const applyProcess = async () => {
    setProcBusy(true);
    setError("");
    try {
      const r = await api.post<ProcResult>("/v1/bank-transactions/auto-allocate", {});
      setProcPreview(null);
      setSuccess(`${t("brules.matched")}: ${r.matched}, ${t("brules.unmatched")}: ${r.unmatched}, ${t("brules.errors")}: ${r.errors}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setProcBusy(false);
    }
  };

  const handleFile = async (file: File) => {
    if (!selectedAccount) {
      setError(t("bank_tx.select_account"));
      return;
    }
    setError("");
    setSuccess("");
    const content = await file.text();
    setPendingContent(content);
    setPreview(null);
    setPreviewOpen(true);
    setPreviewLoading(true);
    try {
      const data = await api.post<Preview>("/v1/bank-transactions/preview", {
        bank_account_id: Number(selectedAccount),
        file_content: content,
      });
      setPreview(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPreviewOpen(false);
    } finally {
      setPreviewLoading(false);
    }
  };

  const handleImport = async () => {
    setImporting(true);
    try {
      const r = await api.post<{ imported: number; journal_count: number }>(
        "/v1/bank-transactions/import",
        {
          bank_account_id: Number(selectedAccount),
          file_content: pendingContent,
        }
      );
      const msg =
        r.journal_count > 0
          ? t("bank_tx.imported_with_journal")
              .replace("{n}", String(r.imported))
              .replace("{j}", String(r.journal_count))
          : t("bank_tx.imported").replace("{n}", String(r.imported));
      setSuccess(msg);
      setError("");
      setPreviewOpen(false);
      setPreview(null);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setImporting(false);
    }
  };

  const openBook = (tx: BankTransaction) => {
    const m = matchCounterpart(tx.counterpart_name || "", counterparts);
    setBookTx(tx);
    setBookContra(0);
    setBookDebit(0);
    setBookCredit(0);
    setBookCpId(m?.id ?? 0);
    setBookCpSearch(tx.counterpart_name || "");
    setBookAccSearch("");
  };

  const openReallocate = async (tx: BankTransaction) => {
    const m = matchCounterpart(tx.counterpart_name || "", counterparts);
    setReallocTx(tx);
    setReallocAccount(0);
    setReallocCpId(m?.id ?? 0);
    setReallocCpSearch(tx.counterpart_name || "");
    setReallocAccSearch("");
    setReallocLines([]);
    if (tx.journal_entry_id > 0) {
      try {
        const je = await api.get<{ lines?: JeLine[] }>(
          `/v1/journal/${tx.journal_entry_id}`
        );
        setReallocLines(je.lines ?? []);
      } catch {
        setReallocLines([]);
      }
    }
  };

  const bankGlId = (tx: BankTransaction | null): number => {
    if (!tx) return 0;
    const ba = accounts.find((a) => a.id === tx.bank_account_id);
    return ba?.gl_account_id ?? 0;
  };

  const handleBook = async () => {
    if (!bookTx) return;
    const gl = bankGlId(bookTx);
    const payload: Record<string, number> = {};
    if (gl > 0) {
      if (!bookContra) {
        setError(t("bank_tx.pick_contra"));
        return;
      }
      payload.contra_account_id = bookContra;
    } else {
      if (!bookDebit || !bookCredit) {
        setError(t("bank_tx.pick_both_accounts"));
        return;
      }
      payload.debit_account_id = bookDebit;
      payload.credit_account_id = bookCredit;
    }
    if (bookCpId > 0) payload.counterpart_id = bookCpId;
    setBookLoading(true);
    try {
      await api.post(`/v1/bank-transactions/${bookTx.id}/book`, payload);
      setSuccess(t("bank_tx.booked"));
      setBookTx(null);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBookLoading(false);
    }
  };

  const handleReallocate = async () => {
    if (!reallocTx) return;
    if (!reallocAccount) {
      setError(t("bank_tx.pick_account"));
      return;
    }
    const payload: Record<string, number> = { account_id: reallocAccount };
    if (reallocCpId > 0) payload.counterpart_id = reallocCpId;
    setReallocLoading(true);
    try {
      const r = await api.post<{ message: string }>(
        `/v1/bank-transactions/${reallocTx.id}/reallocate`,
        payload
      );
      setSuccess(r.message || t("bank_tx.reallocated"));
      setReallocTx(null);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setReallocLoading(false);
    }
  };

  const handleDelete = async (tx: BankTransaction) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/bank-transactions/${tx.id}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const statusBadge = (tx: BankTransaction) => {
    if (tx.is_allocated)
      return (
        <span className="badge badge-success">{t("bank_tx.status.allocated")}</span>
      );
    if (tx.is_booked)
      return (
        <span className="badge badge-warning">{t("bank_tx.status.booked")}</span>
      );
    return <span className="badge badge-danger">{t("bank_tx.status.new")}</span>;
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("bank_tx.title")}</h1>
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          <select
            className="select"
            value={selectedAccount}
            onChange={(e) => setSelectedAccount(e.target.value)}
          >
            <option value="">{t("bank_tx.all_accounts")}</option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name} ({a.iban})
              </option>
            ))}
          </select>
          <select
            className="select"
            value={filterStatus}
            onChange={(e) => setFilterStatus(e.target.value)}
          >
            <option value="">{t("bank_tx.all_statuses")}</option>
            <option value="new">{t("bank_tx.status.new")}</option>
            <option value="booked">{t("bank_tx.status.booked")}</option>
            <option value="allocated">{t("bank_tx.status.allocated")}</option>
          </select>
          <input
            ref={fileInputRef}
            type="file"
            accept=".xml,.sta,.mt940,.txt,.STA,.csv,.CSV"
            style={{ display: "none" }}
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) handleFile(f);
              e.target.value = "";
            }}
          />
          <button
            className="btn btn-primary"
            onClick={() => fileInputRef.current?.click()}
            type="button"
          >
            {t("bank_tx.import_file")}
          </button>
        </div>
      </div>

      <div className="btn-row" style={{ marginBottom: 12 }}>
        <button
          className={`btn btn-sm${tab === "tx" ? " btn-primary" : ""}`}
          type="button"
          onClick={() => setTab("tx")}
        >
          {t("bank_tx.tab_tx")}
        </button>
        <button
          className={`btn btn-sm${tab === "rules" ? " btn-primary" : ""}`}
          type="button"
          onClick={() => setTab("rules")}
        >
          {t("brules.title")}
        </button>
      </div>

      {error && <div className="error-text">{error}</div>}
      {success && (
        <div className="muted" style={{ color: "var(--success)", marginBottom: 12 }}>
          {success}
        </div>
      )}

      {tab === "tx" && (
        <>
      <div className="card" style={{ marginBottom: 12 }}>
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t("bank_tx.search_placeholder")}
        />
        <div className="btn-row" style={{ marginTop: 8, flexWrap: "wrap", alignItems: "center" }}>
          <label className="muted" style={{ fontSize: 13 }}>{t("bank_tx.date_from")}</label>
          <input className="input" type="date" style={{ width: 160 }} value={dateFrom} onChange={(e) => { setDateFrom(e.target.value); setOffset(0); }} />
          <label className="muted" style={{ fontSize: 13 }}>{t("bank_tx.date_to")}</label>
          <input className="input" type="date" style={{ width: 160 }} value={dateTo} onChange={(e) => { setDateTo(e.target.value); setOffset(0); }} />
          <button className="btn btn-sm" type="button" disabled={procBusy} onClick={() => void openProcess()}>
            {t("bank_tx.process")}
          </button>
          {selected.length > 0 && (
            <button className="btn btn-sm btn-danger" type="button" disabled={bulkBusy} onClick={() => void handleBulkDelete()}>
              {t("bank_tx.bulk_delete")} ({selected.length})
            </button>
          )}
        </div>
      </div>

      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : filteredRows.length === 0 ? (
          <div className="content muted">{t("bank_tx.empty")}</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 32 }}>
                  <input type="checkbox" checked={allSelected} onChange={(e) => toggleAll(e.target.checked)} />
                </th>
                <th>{t("bank_tx.date")}</th>
                <th>{t("bank_tx.description")}</th>
                <th>{t("bank_tx.counterpart")}</th>
                <th style={{ textAlign: "right" }}>{t("bank_tx.amount")}</th>
                <th style={{ textAlign: "center" }}>{t("bank_tx.status")}</th>
                <th style={{ textAlign: "right" }}>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {pagedRows.map((tx) => (
                <tr key={tx.id}>
                  <td>
                    <input type="checkbox" checked={selected.includes(tx.id)} onChange={(e) => toggleSel(tx.id, e.target.checked)} />
                  </td>
                  <td style={{ whiteSpace: "nowrap" }}>{tx.transaction_date}</td>
                  <td
                    style={{ maxWidth: 300, overflow: "hidden", textOverflow: "ellipsis" }}
                    title={tx.description}
                  >
                    {tx.description}
                  </td>
                  <td>{tx.counterpart_name}</td>
                  <td
                    style={{
                      textAlign: "right",
                      whiteSpace: "nowrap",
                      fontWeight: 500,
                      color: isPositive(tx.amount)
                        ? "var(--success)"
                        : "var(--danger)",
                    }}
                  >
                    {fmtAmount(tx.amount)} {tx.currency}
                  </td>
                  <td style={{ textAlign: "center" }}>{statusBadge(tx)}</td>
                  <td style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                    <div className="icon-actions" style={{ justifyContent: "flex-end" }}>
                      {!tx.is_booked && (
                        <>
                          <button className="btn btn-sm" onClick={() => openBook(tx)}>
                            {t("bank_tx.book")}
                          </button>
                          <button
                            className="btn btn-sm btn-danger"
                            onClick={() => handleDelete(tx)}
                          >
                            {t("common.delete")}
                          </button>
                        </>
                      )}
                      {tx.is_booked && !tx.is_allocated && (
                        <button
                          className="btn btn-sm"
                          onClick={() => {
                            void openReallocate(tx);
                          }}
                        >
                          {t("bank_tx.reallocate")}
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <Pagination
          total={filteredRows.length}
          limit={PAGE_SIZE}
          offset={start}
          onPageChange={setOffset}
        />
      </div>
        </>
      )}

      {tab === "rules" && (
        <BankRulesPanel accounts={chartAccounts} onChanged={() => void load()} />
      )}

      {procPreview && (
        <div className="modal-backdrop" onClick={() => setProcPreview(null)}>
          <div className="card modal picker-modal" style={{ maxWidth: 720 }} onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">{t("bank_tx.process")}</h2>
            <p className="muted" style={{ fontSize: 13 }}>
              {t("brules.preview_hint")}: {procPreview.matched} · {procPreview.unmatched} {t("brules.unmatched")}
            </p>
            <div className="picker-list">
              {procPreview.details.filter((d) => d.status === "matched").map((d) => (
                <div key={d.id} className="picker-item">
                  <b>#{d.id} · {d.rule}</b>
                  <span className="muted">
                    {d.account}
                    {d.counterpart ? ` · ${d.counterpart}` : ""} · {d.amount}
                  </span>
                </div>
              ))}
              {procPreview.matched === 0 && <div className="muted">{t("brules.empty")}</div>}
            </div>
            <div className="form-actions">
              <button className="btn btn-primary" type="button" disabled={procBusy || procPreview.matched === 0} onClick={() => void applyProcess()}>
                {t("brules.apply")}
              </button>
              <button className="btn" type="button" onClick={() => setProcPreview(null)}>
                {t("common.cancel")}
              </button>
            </div>
          </div>
        </div>
      )}

      <BankTxModals
        t={t}
        counterparts={counterparts}
        chartAccounts={chartAccounts}
        previewOpen={previewOpen}
        setPreviewOpen={setPreviewOpen}
        previewLoading={previewLoading}
        preview={preview}
        importing={importing}
        handleImport={handleImport}
        bookTx={bookTx}
        setBookTx={setBookTx}
        bookCpId={bookCpId}
        bookCpSearch={bookCpSearch}
        setBookCpSearch={setBookCpSearch}
        setBookCpId={setBookCpId}
        bookContra={bookContra}
        bookAccSearch={bookAccSearch}
        setBookAccSearch={setBookAccSearch}
        setBookContra={setBookContra}
        bookDebit={bookDebit}
        setBookDebit={setBookDebit}
        bookCredit={bookCredit}
        setBookCredit={setBookCredit}
        handleBook={handleBook}
        bookLoading={bookLoading}
        reallocTx={reallocTx}
        setReallocTx={setReallocTx}
        reallocLines={reallocLines}
        reallocCpId={reallocCpId}
        reallocCpSearch={reallocCpSearch}
        setReallocCpSearch={setReallocCpSearch}
        setReallocCpId={setReallocCpId}
        reallocAccount={reallocAccount}
        reallocAccSearch={reallocAccSearch}
        setReallocAccSearch={setReallocAccSearch}
        setReallocAccount={setReallocAccount}
        handleReallocate={handleReallocate}
        reallocLoading={reallocLoading}
        bankGlId={bankGlId}
      />
    </div>
  );
}

export default function BankTransactionsPage() {
  return (
    <RequireAuth>
      <BankTransactionsInner />
    </RequireAuth>
  );
}
