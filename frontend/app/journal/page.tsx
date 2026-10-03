"use client";

// Дневник — счетоводни записи. Нов запис / редакция: голям модал с три
// таба (Покупки / Продажби / Без ДДС). Клик върху ред зарежда записа.

import { useCallback, useEffect, useMemo, useState, FormEvent, MouseEvent } from "react";
import { api, ListResponse } from "../../lib/api";
import { formatBgDate, todayIso } from "../../lib/dates";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { IconButton } from "../../components/IconButton";

interface JournalEntry {
  id: number;
  entry_date: string;
  document_type: string;
  document_id: number;
  description: string;
  vat_type: string;
  counterpart_name: string;
  counterpart_id?: number;
  document_number?: string;
  document_date?: string;
  vat_period?: string;
}

interface JournalLine {
  id: number;
  account_id: number;
  direction: string;
  amount: string;
  vat_amount?: string;
  account_number?: string;
  account_name?: string;
  location_id?: number;
  product_id?: number;
}

interface Account {
  id: number;
  number: string;
  name: string;
}

interface Counterpart {
  id: number;
  name: string;
  counterpart_type: string;
}

type Tab = "purchase" | "sales" | "no_vat";

interface LineDraft {
  accountId: string;
  direction: "debit" | "credit";
  amount: string;
  vatAmount: string;
  locationId: number;
  productId: number;
}

const emptyLine = (): LineDraft => ({
  accountId: "",
  direction: "debit",
  amount: "",
  vatAmount: "",
  locationId: 0,
  productId: 0,
});

const VAT_TYPE_LABEL: Record<string, string> = {
  purchase: "journal.tab.purchase",
  sales: "journal.tab.sales",
  no_vat: "journal.tab.no_vat",
};

function toDateInput(raw: string | undefined): string {
  if (!raw) return "";
  const s = String(raw).trim();
  const iso = s.match(/^(\d{4})-(\d{2})-(\d{2})/);
  if (iso) return `${iso[1]}-${iso[2]}-${iso[3]}`;
  const compact = s.match(/^(\d{4})(\d{2})(\d{2})$/);
  if (compact) return `${compact[1]}-${compact[2]}-${compact[3]}`;
  return s.slice(0, 10);
}

function toMonthInput(raw: string | undefined): string {
  if (!raw) return "";
  const s = String(raw).trim();
  const iso = s.match(/^(\d{4})-(\d{2})/);
  if (iso) return `${iso[1]}-${iso[2]}`;
  const compact = s.match(/^(\d{4})(\d{2})$/);
  if (compact) return `${compact[1]}-${compact[2]}`;
  return s.slice(0, 7);
}

function JournalInner() {
  const { t } = useI18n();
  const [entries, setEntries] = useState<JournalEntry[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [counterparts, setCounterparts] = useState<Counterpart[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [q, setQ] = useState("");

  // модал
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [loadingEntry, setLoadingEntry] = useState(false);
  const [tab, setTab] = useState<Tab>("purchase");
  const [entryDate, setEntryDate] = useState("");
  const [description, setDescription] = useState("");
  const [counterpartId, setCounterpartId] = useState("");
  const [documentNumber, setDocumentNumber] = useState("");
  const [documentDate, setDocumentDate] = useState("");
  const [vatPeriod, setVatPeriod] = useState("");
  const [lines, setLines] = useState<LineDraft[]>([emptyLine()]);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<ListResponse<JournalEntry>>("/v1/journal");
      setEntries(data.items ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const loadRefs = useCallback(async () => {
    try {
      const a = await api.get<ListResponse<Account>>("/v1/accounts");
      setAccounts(a.items ?? []);
      const c = await api.get<ListResponse<Counterpart>>("/v1/counterparts");
      setCounterparts(c.items ?? []);
    } catch {
      /* празни списъци */
    }
  }, []);

  useEffect(() => {
    load();
    loadRefs();
  }, [load, loadRefs]);

  const resetForm = (date: string) => {
    setTab("purchase");
    setEntryDate(date);
    setDescription("");
    setCounterpartId("");
    setDocumentNumber("");
    setDocumentDate("");
    setVatPeriod("");
    setLines([emptyLine()]);
    setFormError("");
  };

  const openCreate = () => {
    setEditingId(null);
    setLoadingEntry(false);
    resetForm(todayIso());
    setModalOpen(true);
  };

  const applyEntry = (data: JournalEntry & { lines?: JournalLine[] }) => {
    const vt = data.vat_type;
    setTab(vt === "purchase" || vt === "sales" || vt === "no_vat" ? vt : "no_vat");
    setEntryDate(toDateInput(data.entry_date));
    setDescription(data.description ?? "");
    setCounterpartId(data.counterpart_id ? String(data.counterpart_id) : "");
    setDocumentNumber(data.document_number ?? "");
    setDocumentDate(toDateInput(data.document_date));
    setVatPeriod(toMonthInput(data.vat_period));
    const loaded = (data.lines ?? []).map((l) => ({
      accountId: l.account_id ? String(l.account_id) : "",
      direction: l.direction === "credit" ? ("credit" as const) : ("debit" as const),
      amount: l.amount ?? "",
      vatAmount: l.vat_amount && l.vat_amount !== "0" ? l.vat_amount : "",
      locationId: l.location_id ?? 0,
      productId: l.product_id ?? 0,
    }));
    setLines(loaded.length > 0 ? loaded : [emptyLine()]);
  };

  const openEdit = async (entry: JournalEntry, e?: MouseEvent) => {
    e?.stopPropagation();
    setEditingId(entry.id);
    setFormError("");
    setLoadingEntry(true);
    setModalOpen(true);
    applyEntry(entry);
    try {
      const data = await api.get<JournalEntry & { lines: JournalLine[] }>(
        `/v1/journal/${entry.id}`
      );
      applyEntry(data);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingEntry(false);
    }
  };

  const setLine = (
    i: number,
    f: "accountId" | "direction" | "amount" | "vatAmount",
    v: string
  ) =>
    setLines((prev) =>
      prev.map((l, idx) => (idx === i ? { ...l, [f]: v } : l))
    );

  const addLine = () => setLines((prev) => [...prev, emptyLine()]);

  const removeLine = (i: number) =>
    setLines((prev) => prev.filter((_, idx) => idx !== i));

  const debitSum = lines
    .filter((l) => l.direction === "debit")
    .reduce((acc, l) => acc + (parseFloat(l.amount) || 0), 0);
  const creditSum = lines
    .filter((l) => l.direction === "credit")
    .reduce((acc, l) => acc + (parseFloat(l.amount) || 0), 0);
  const balanced = Math.abs(debitSum - creditSum) < 0.0001;

  const filteredEntries = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return entries;
    return entries.filter((en) =>
      `${en.entry_date} ${en.document_type} ${en.document_id} ${en.document_number ?? ""} ${en.description} ${en.counterpart_name}`
        .toLowerCase()
        .includes(s)
    );
  }, [entries, q]);

  const filteredCounterparts = counterparts.filter((c) => {
    if (String(c.id) === counterpartId) return true;
    if (tab === "purchase") return c.counterpart_type !== "customer";
    if (tab === "sales") return c.counterpart_type !== "supplier";
    return true;
  });

  const docLabel = (en: JournalEntry) => {
    if (en.document_number) return en.document_number;
    if (en.document_type === "manual" || !en.document_type) {
      return t("journal.doc.manual");
    }
    return en.document_id ? `${en.document_type} #${en.document_id}` : en.document_type;
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (loadingEntry) return;
    setSaving(true);
    setFormError("");
    try {
      const payload = {
        entry_date: entryDate,
        description,
        vat_type: tab,
        counterpart_id: Number(counterpartId) || 0,
        document_number: documentNumber,
        document_date: documentDate,
        vat_period: tab === "no_vat" ? "" : vatPeriod,
        lines: lines.map((l) => ({
          account_id: Number(l.accountId),
          direction: l.direction,
          amount: l.amount,
          vat_amount: l.vatAmount || "0",
          location_id: l.locationId || 0,
          product_id: l.productId || 0,
        })),
      };
      if (editingId) {
        await api.put(`/v1/journal/${editingId}`, payload);
      } else {
        await api.post("/v1/journal", payload);
      }
      setModalOpen(false);
      await load();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("journal.title")}</h1>
        <button className="btn btn-primary" onClick={openCreate}>
          {t("journal.new_entry")}
        </button>
      </div>

      {error && <div className="error-text">{error}</div>}
      <div className="card" style={{ marginBottom: 12 }}>
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t("common.search")}
        />
      </div>

      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : filteredEntries.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>{t("journal.entry_date")}</th>
                  <th>{t("journal.document")}</th>
                  <th>{t("journal.description")}</th>
                  <th>{t("journal.counterpart")}</th>
                  <th>{t("journal.vat_type")}</th>
                  <th>{t("common.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {filteredEntries.map((en) => (
                  <tr
                    key={en.id}
                    onClick={() => void openEdit(en)}
                    style={{ cursor: "pointer" }}
                  >
                    <td>{formatBgDate(en.entry_date)}</td>
                    <td>{docLabel(en)}</td>
                    <td>{en.description}</td>
                    <td>{en.counterpart_name}</td>
                    <td>
                      <span
                        className={`badge ${
                          en.vat_type === "no_vat"
                            ? "badge-muted"
                            : en.vat_type === "sales"
                              ? "badge-success"
                              : "badge-warning"
                        }`}
                      >
                        {t(VAT_TYPE_LABEL[en.vat_type] ?? "journal.tab.no_vat")}
                      </span>
                    </td>
                    <td onClick={(e) => e.stopPropagation()}>
                      <div className="icon-actions">
                        <IconButton
                          icon="edit"
                          title={t("common.edit")}
                          onClick={(e) => void openEdit(en, e)}
                        />
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {modalOpen && (
        <div className="modal-backdrop" onClick={() => setModalOpen(false)}>
          <div
            className="card modal modal-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">
              {editingId ? t("journal.edit_entry") : t("journal.new_entry")}
            </h2>

            {loadingEntry ? (
              <div className="content muted">{t("journal.loading_entry")}</div>
            ) : null}

            <div style={{ display: "flex", gap: 8, marginBottom: 18, flexWrap: "wrap" }}>
              {(
                [
                  ["purchase", "journal.tab.purchase"],
                  ["sales", "journal.tab.sales"],
                  ["no_vat", "journal.tab.no_vat"],
                ] as const
              ).map(([value, key]) => (
                <button
                  key={value}
                  type="button"
                  className={`btn btn-sm ${tab === value ? "btn-primary" : ""}`}
                  onClick={() => setTab(value)}
                  disabled={loadingEntry}
                >
                  {t(key)}
                </button>
              ))}
            </div>

            <form onSubmit={(e) => void handleSave(e)}>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("journal.entry_date")} *</label>
                  <input
                    className="input"
                    type="date"
                    value={entryDate}
                    onChange={(e) => setEntryDate(e.target.value)}
                    required
                    disabled={loadingEntry}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("journal.description")} *</label>
                  <input
                    className="input"
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    required
                    disabled={loadingEntry}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("journal.counterpart")}</label>
                  <select
                    className="select"
                    value={counterpartId}
                    onChange={(e) => setCounterpartId(e.target.value)}
                    disabled={loadingEntry}
                  >
                    <option value="">—</option>
                    {filteredCounterparts.map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.name}
                      </option>
                    ))}
                  </select>
                </div>
                {tab !== "no_vat" && (
                  <div className="field">
                    <label className="label">{t("journal.vat_period")}</label>
                    <input
                      className="input"
                      type="month"
                      value={vatPeriod}
                      onChange={(e) => setVatPeriod(e.target.value)}
                      disabled={loadingEntry}
                    />
                  </div>
                )}
                <div className="field">
                  <label className="label">{t("journal.document_number")}</label>
                  <input
                    className="input"
                    value={documentNumber}
                    onChange={(e) => setDocumentNumber(e.target.value)}
                    disabled={loadingEntry}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("journal.document_date")}</label>
                  <input
                    className="input"
                    type="date"
                    value={documentDate}
                    onChange={(e) => setDocumentDate(e.target.value)}
                    disabled={loadingEntry}
                  />
                </div>
              </div>

              <h3 style={{ margin: "16px 0 8px" }}>{t("journal.lines")}</h3>
              <p className="muted" style={{ margin: "0 0 8px" }}>
                {t("journal.sign_hint")}
              </p>
              <div className={tab === "no_vat" ? "journal-lines-novat" : undefined}>
                <div className="journal-line-head">
                  <span>{t("journal.account")}</span>
                  <span>{t("journal.debit")} / {t("journal.credit")}</span>
                  <span>{t("journal.amount")}</span>
                  {tab !== "no_vat" ? <span>{t("journal.vat_amount")}</span> : null}
                  <span />
                </div>
                {lines.map((l, i) => (
                  <div key={i} className="journal-line-row">
                    <select
                      className="select"
                      value={l.accountId}
                      onChange={(e) => setLine(i, "accountId", e.target.value)}
                      required
                      disabled={loadingEntry}
                    >
                      <option value="" disabled>
                        {t("journal.account")}…
                      </option>
                      {accounts.map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.number} {a.name}
                        </option>
                      ))}
                    </select>
                    <select
                      className="select"
                      value={l.direction}
                      onChange={(e) =>
                        setLine(i, "direction", e.target.value as "debit" | "credit")
                      }
                      disabled={loadingEntry}
                    >
                      <option value="debit">{t("journal.debit")}</option>
                      <option value="credit">{t("journal.credit")}</option>
                    </select>
                    <input
                      className="input"
                      inputMode="decimal"
                      placeholder="-100.00"
                      title={t("journal.sign_hint")}
                      value={l.amount}
                      onChange={(e) => setLine(i, "amount", e.target.value)}
                      required
                      disabled={loadingEntry}
                    />
                    {tab !== "no_vat" && (
                      <input
                        className="input"
                        inputMode="decimal"
                        placeholder={t("journal.vat_amount")}
                        title={t("journal.sign_hint")}
                        value={l.vatAmount}
                        onChange={(e) => setLine(i, "vatAmount", e.target.value)}
                        disabled={loadingEntry}
                      />
                    )}
                    <button
                      type="button"
                      className="btn btn-danger btn-sm"
                      onClick={() => removeLine(i)}
                      disabled={loadingEntry || lines.length < 2}
                    >
                      ✕
                    </button>
                  </div>
                ))}
              </div>
              <button
                type="button"
                className="btn btn-sm"
                onClick={addLine}
                disabled={loadingEntry}
              >
                + {t("journal.add_line")}
              </button>

              <div className="muted" style={{ marginTop: 12 }}>
                {t("journal.balance")}: {debitSum.toFixed(2)} /{" "}
                {creditSum.toFixed(2)}{" "}
                {balanced ? "✓" : "✗"}
              </div>

              {formError && <div className="error-text">{formError}</div>}
              <div className="form-actions">
                <button
                  type="button"
                  className="btn"
                  onClick={() => setModalOpen(false)}
                >
                  {t("common.cancel")}
                </button>
                <button
                  className="btn btn-primary"
                  disabled={saving || !balanced || loadingEntry}
                >
                  {t("common.save")}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}

export default function JournalPage() {
  return (
    <RequireAuth>
      <JournalInner />
    </RequireAuth>
  );
}
