"use client";

import { BankTransaction, Counterpart, Account, JeLine, Preview } from "./types";
import { fmtAmount, isPositive } from "./helpers";
import { AccountSelect, CounterpartSelect } from "./BankTxSelects";

export function BankTxModals(props: {
  t: (k: string) => string;
  counterparts: Counterpart[];
  chartAccounts: Account[];
  previewOpen: boolean;
  setPreviewOpen: (v: boolean) => void;
  previewLoading: boolean;
  preview: Preview | null;
  importing: boolean;
  handleImport: () => void;
  bookTx: BankTransaction | null;
  setBookTx: (v: BankTransaction | null) => void;
  bookCpId: number;
  bookCpSearch: string;
  setBookCpSearch: (v: string) => void;
  setBookCpId: (v: number) => void;
  bookContra: number;
  bookAccSearch: string;
  setBookAccSearch: (v: string) => void;
  setBookContra: (v: number) => void;
  bookDebit: number;
  setBookDebit: (v: number) => void;
  bookCredit: number;
  setBookCredit: (v: number) => void;
  handleBook: () => void;
  bookLoading: boolean;
  reallocTx: BankTransaction | null;
  setReallocTx: (v: BankTransaction | null) => void;
  reallocLines: JeLine[];
  reallocCpId: number;
  reallocCpSearch: string;
  setReallocCpSearch: (v: string) => void;
  setReallocCpId: (v: number) => void;
  reallocAccount: number;
  reallocAccSearch: string;
  setReallocAccSearch: (v: string) => void;
  setReallocAccount: (v: number) => void;
  handleReallocate: () => void;
  reallocLoading: boolean;
  bankGlId: (tx: BankTransaction | null) => number;
}) {
  const {
    t, counterparts, chartAccounts,
    previewOpen, setPreviewOpen, previewLoading, preview, importing, handleImport,
    bookTx, setBookTx, bookCpId, bookCpSearch, setBookCpSearch, setBookCpId,
    bookContra, bookAccSearch, setBookAccSearch, setBookContra, bookDebit, setBookDebit,
    bookCredit, setBookCredit, handleBook, bookLoading,
    reallocTx, setReallocTx, reallocLines, reallocCpId, reallocCpSearch, setReallocCpSearch,
    setReallocCpId, reallocAccount, reallocAccSearch, setReallocAccSearch, setReallocAccount,
    handleReallocate, reallocLoading, bankGlId,
  } = props;

  const txSummary = (tx: BankTransaction) => (
    <div className="summary-grid" style={{ marginBottom: 16 }}>
      <div className="summary-box">
        <div className="summary-label">{t("bank_tx.date")}</div>
        <div className="summary-value">{tx.transaction_date}</div>
      </div>
      <div className="summary-box">
        <div className="summary-label">{t("bank_tx.amount")}</div>
        <div
          className="summary-value"
          style={{ color: isPositive(tx.amount) ? "var(--success)" : "var(--danger)" }}
        >
          {fmtAmount(tx.amount)} {tx.currency}
        </div>
      </div>
      {tx.reference ? (
        <div className="summary-box">
          <div className="summary-label">{t("bank_tx.reference")}</div>
          <div className="summary-value" style={{ fontSize: 13 }}>
            {tx.reference}
          </div>
        </div>
      ) : null}
    </div>
  );

  return (
    <>
      {previewOpen && (
        <div className="modal-backdrop" onClick={() => setPreviewOpen(false)}>
          <div
            className="card modal"
            style={{ maxWidth: 840, maxHeight: "85vh", overflowY: "auto" }}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">{t("bank_tx.preview_title")}</h2>
            {previewLoading ? (
              <div className="content muted">{t("common.loading")}</div>
            ) : preview ? (
              <>
                <div className="summary-grid">
                  <div className="summary-box">
                    <div className="summary-label">{t("bank_tx.preview.format")}</div>
                    <div className="summary-value">{preview.format_name}</div>
                  </div>
                  <div className="summary-box">
                    <div className="summary-label">{t("bank_tx.preview.total")}</div>
                    <div className="summary-value">{preview.total_count}</div>
                  </div>
                  <div className="summary-box">
                    <div className="summary-label">{t("bank_tx.preview.new")}</div>
                    <div className="summary-value" style={{ color: "var(--success)" }}>
                      {preview.new_count}
                    </div>
                  </div>
                  <div className="summary-box">
                    <div className="summary-label">{t("bank_tx.preview.duplicates")}</div>
                    <div className="summary-value" style={{ color: "var(--danger)" }}>
                      {preview.duplicate_count}
                    </div>
                  </div>
                </div>
                {preview.account_iban && (
                  <div className="muted" style={{ margin: "12px 0" }}>
                    {t("bank_tx.preview.iban")}:{" "}
                    <strong>{preview.account_iban}</strong>
                  </div>
                )}
                <div style={{ maxHeight: 300, overflowY: "auto" }}>
                  <table className="table">
                    <thead>
                      <tr>
                        <th>{t("bank_tx.date")}</th>
                        <th>{t("bank_tx.description")}</th>
                        <th>{t("bank_tx.counterpart")}</th>
                        <th style={{ textAlign: "right" }}>{t("bank_tx.amount")}</th>
                        <th style={{ textAlign: "center" }}></th>
                      </tr>
                    </thead>
                    <tbody>
                      {preview.transactions.map((tx, i) => (
                        <tr
                          key={i}
                          style={
                            tx.is_duplicate
                              ? { opacity: 0.6, background: "var(--danger-soft)" }
                              : undefined
                          }
                        >
                          <td style={{ whiteSpace: "nowrap" }}>{tx.date}</td>
                          <td
                            style={{
                              maxWidth: 300,
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                            }}
                            title={tx.description}
                          >
                            {tx.description}
                          </td>
                          <td>{tx.contra_name}</td>
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
                          <td style={{ textAlign: "center" }}>
                            {tx.is_duplicate && (
                              <span className="badge badge-danger">
                                {t("bank_tx.duplicate")}
                              </span>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <div className="form-actions">
                  <button
                    className="btn"
                    onClick={() => setPreviewOpen(false)}
                    type="button"
                  >
                    {t("common.cancel")}
                  </button>
                  {preview.new_count > 0 && (
                    <button
                      className="btn btn-primary"
                      onClick={handleImport}
                      disabled={importing}
                      type="button"
                    >
                      {importing
                        ? t("bank_tx.importing")
                        : t("bank_tx.import_n").replace(
                            "{n}",
                            String(preview.new_count)
                          )}
                    </button>
                  )}
                </div>
              </>
            ) : null}
          </div>
        </div>
      )}

      {bookTx && (
        <div className="modal-backdrop" onClick={() => setBookTx(null)}>
          <div
            className="card modal"
            style={{ maxWidth: 560 }}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">{t("bank_tx.book_title")}</h2>
            {txSummary(bookTx)}
            {bookTx.description ? (
              <p className="muted" style={{ marginBottom: 12 }}>
                {bookTx.description}
              </p>
            ) : null}
            {bookTx.counterpart_name ? (
              <div className="field" style={{ marginBottom: 12 }}>
                <label className="label">{t("bank_tx.bank_counterpart")}</label>
                <div>{bookTx.counterpart_name}</div>
              </div>
            ) : null}
            <p className="muted" style={{ marginBottom: 16 }}>
              {t("bank_tx.book_hint")}
            </p>
            <div className="field" style={{ marginBottom: 12 }}>
              <label className="label">{t("bank_tx.counterpart")}</label>
              <CounterpartSelect
                t={t}
                counterparts={counterparts}
                value={bookCpId}
                search={bookCpSearch}
                onSearch={setBookCpSearch}
                onChange={setBookCpId}
              />
            </div>
            {bankGlId(bookTx) > 0 ? (
              <div className="field" style={{ marginBottom: 16 }}>
                <label className="label">{t("bank_tx.contra_account")}</label>
                <AccountSelect
                  t={t}
                  chartAccounts={chartAccounts}
                  value={bookContra}
                  search={bookAccSearch}
                  onSearch={setBookAccSearch}
                  onChange={setBookContra}
                />
              </div>
            ) : (
              <>
                <p className="muted" style={{ marginBottom: 12 }}>
                  {t("bank_tx.no_gl")}
                </p>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("bank_tx.debit")}</label>
                  <AccountSelect
                    t={t}
                    chartAccounts={chartAccounts}
                    value={bookDebit}
                    search={bookAccSearch}
                    onSearch={setBookAccSearch}
                    onChange={setBookDebit}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("bank_tx.credit")}</label>
                  <AccountSelect
                    t={t}
                    chartAccounts={chartAccounts}
                    value={bookCredit}
                    search={bookAccSearch}
                    onSearch={setBookAccSearch}
                    onChange={setBookCredit}
                  />
                </div>
              </div>
              </>
            )}
            <div className="form-actions">
              <button className="btn" onClick={() => setBookTx(null)} type="button">
                {t("common.cancel")}
              </button>
              <button
                className="btn btn-primary"
                onClick={handleBook}
                disabled={bookLoading}
                type="button"
              >
                {t("bank_tx.book")}
              </button>
            </div>
          </div>
        </div>
      )}

      {reallocTx && (
        <div className="modal-backdrop" onClick={() => setReallocTx(null)}>
          <div
            className="card modal"
            style={{ maxWidth: 640, maxHeight: "90vh", overflowY: "auto" }}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">{t("bank_tx.reallocate_title")}</h2>
            {txSummary(reallocTx)}
            {reallocTx.description ? (
              <p className="muted" style={{ marginBottom: 12 }}>
                {reallocTx.description}
              </p>
            ) : null}
            {reallocTx.counterpart_name ? (
              <div className="field" style={{ marginBottom: 12 }}>
                <label className="label">{t("bank_tx.bank_counterpart")}</label>
                <div>{reallocTx.counterpart_name}</div>
              </div>
            ) : null}
            {reallocLines.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, margin: "0 0 8px" }}>
                  {t("bank_tx.je_lines")}
                </h3>
                <table className="table">
                  <thead>
                    <tr>
                      <th>{t("bank_tx.target_account")}</th>
                      <th style={{ textAlign: "right" }}>{t("bank_tx.debit")}</th>
                      <th style={{ textAlign: "right" }}>{t("bank_tx.credit")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {reallocLines.map((ln) => (
                      <tr key={ln.id}>
                        <td>
                          {(ln.account_number || "") +
                            (ln.account_name ? ` ${ln.account_name}` : "")}
                        </td>
                        <td style={{ textAlign: "right" }}>
                          {ln.direction === "debit" ? fmtAmount(ln.amount) : ""}
                        </td>
                        <td style={{ textAlign: "right" }}>
                          {ln.direction === "credit" ? fmtAmount(ln.amount) : ""}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            <p className="muted" style={{ marginBottom: 16 }}>
              {t("bank_tx.reallocate_hint")}
            </p>
            <div className="field" style={{ marginBottom: 12 }}>
              <label className="label">{t("bank_tx.counterpart")}</label>
              <CounterpartSelect
                t={t}
                counterparts={counterparts}
                value={reallocCpId}
                search={reallocCpSearch}
                onSearch={setReallocCpSearch}
                onChange={setReallocCpId}
              />
            </div>
            <div className="field" style={{ marginBottom: 16 }}>
              <label className="label">{t("bank_tx.contra_account")}</label>
              <AccountSelect
                t={t}
                chartAccounts={chartAccounts}
                value={reallocAccount}
                search={reallocAccSearch}
                onSearch={setReallocAccSearch}
                onChange={setReallocAccount}
              />
            </div>
            <div className="form-actions">
              <button
                className="btn"
                onClick={() => setReallocTx(null)}
                type="button"
              >
                {t("common.cancel")}
              </button>
              <button
                className="btn btn-primary"
                onClick={handleReallocate}
                disabled={reallocLoading}
                type="button"
              >
                {t("bank_tx.reallocate")}
              </button>
            </div>
          </div>
        </div>
      )}

    </>
  );
}
