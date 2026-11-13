"use client";

// Дневник — счетоводни записи. Нов запис / редакция: голям модал с три
// таба (Покупки / Продажби / Без ДДС). Клик върху ред зарежда записа.

import { useCallback, useEffect, useMemo, useState, FormEvent, MouseEvent } from "react";
import { api, fetchAll, getActiveCompany, ACTIVE_COMPANY_EVENT } from "../../lib/api";
import { formatBgDate, todayIso } from "../../lib/dates";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { IconButton } from "../../components/IconButton";
import { Pagination } from "../../components/Pagination";
import { AuditHistoryPanel, AuditStampBadge } from "../../components/AuditHistory";
import { AuditEntry } from "../../lib/invoice";

const PAGE_SIZE = 50;

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
  created_by?: string;
  created_at?: string;
  updated_by?: string;
  updated_at?: string;
  edit_count?: number;
  audit_history?: AuditEntry[];
}

interface JournalLine {
  id: number;
  account_id: number;
  direction: string;
  amount: string;
  vat_amount?: string;
  currency?: string;
  fx_quantity?: string;
  fx_rate?: string;
  account_number?: string;
  account_name?: string;
  location_id?: number;
  product_id?: number;
}

interface Account {
  id: number;
  number: string;
  name: string;
  is_currency?: boolean;
}

interface Counterpart {
  id: number;
  name: string;
  counterpart_type: string;
}

interface BulkResult {
  deleted?: number;
  failed: number;
}

type Tab = "purchase" | "sales" | "no_vat";

function isManualEntry(en: JournalEntry): boolean {
  return (en.document_type === "manual" || !en.document_type) && (!en.document_id || en.document_id === 0);
}

interface LineDraft {
  accountId: string;
  direction: "debit" | "credit";
  amount: string;
  vatAmount: string;
  currency: string;
  fxQuantity: string;
  fxRate: string;
  locationId: number;
  productId: number;
}

const emptyLine = (): LineDraft => ({
  accountId: "",
  direction: "debit",
  amount: "",
  vatAmount: "",
  currency: "",
  fxQuantity: "",
  fxRate: "1",
  locationId: 0,
  productId: 0,
});

const VAT_TYPE_LABEL: Record<string, string> = {
  purchase: "journal.tab.purchase",
  sales: "journal.tab.sales",
  both: "journal.tab.both",
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

function accountLabel(accounts: Account[], id: string): string {
  const a = accounts.find((x) => String(x.id) === id);
  return a ? `${a.number} ${a.name}` : "";
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
  const [notice, setNotice] = useState("");
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);

  // модал
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [readOnly, setReadOnly] = useState(false);
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
  const [activeEntryMeta, setActiveEntryMeta] = useState<JournalEntry | null>(null);
  // модалът за избор на сметка е отворен за този индекс ред
  const [accountModalFor, setAccountModalFor] = useState<number | null>(null);
  const [accountQuery, setAccountQuery] = useState("");
  // базова валута на активната фирма — за нея курсът е 1
  const [baseCurrency, setBaseCurrency] = useState("EUR");

  useEffect(() => {
    const readBase = () => {
      getActiveCompany()
        .then((c) => {
          const cur = (c as { currency?: unknown }).currency;
          if (typeof cur === "string" && cur) setBaseCurrency(cur.toUpperCase());
        })
        .catch(() => {});
    };
    readBase();
    window.addEventListener(ACTIVE_COMPANY_EVENT, readBase);
    return () => window.removeEventListener(ACTIVE_COMPANY_EVENT, readBase);
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      setEntries(await fetchAll<JournalEntry>("/v1/journal"));
      setSelected([]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const loadRefs = useCallback(async () => {
    try {
      const a = await fetchAll<Account>("/v1/accounts");
      setAccounts(a);
      const c = await fetchAll<Counterpart>("/v1/counterparts");
      setCounterparts(c);
    } catch {
      /* празни списъци */
    }
  }, []);

  useEffect(() => {
    load();
    loadRefs();
  }, [load, loadRefs]);

  const resetForm = (date: string) => {
    setTab("no_vat");
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
    setActiveEntryMeta(null);
    setReadOnly(false);
    setLoadingEntry(false);
    resetForm(todayIso());
    setModalOpen(true);
  };

  const applyEntry = (data: JournalEntry & { lines?: JournalLine[] }) => {
    setActiveEntryMeta(data);
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
      currency: l.currency ?? "",
      fxQuantity: l.fx_quantity && l.fx_quantity !== "" ? l.fx_quantity : (l.amount ?? ""),
      fxRate: l.fx_rate && l.fx_rate !== "" ? l.fx_rate : "1",
      locationId: l.location_id ?? 0,
      productId: l.product_id ?? 0,
    }));
    setLines(loaded.length > 0 ? loaded : [emptyLine()]);
  };

  const openEdit = async (entry: JournalEntry, e?: MouseEvent) => {
    e?.stopPropagation();
    setEditingId(entry.id);
    setReadOnly(!isManualEntry(entry));
    setFormError("");
    setLoadingEntry(true);
    setModalOpen(true);
    applyEntry(entry);
    try {
      const data = await api.get<JournalEntry & { lines: JournalLine[] }>(
        `/v1/journal/${entry.id}`
      );
      setReadOnly(!isManualEntry(data));
      applyEntry(data);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingEntry(false);
    }
  };

  const setLine = (
    i: number,
    f: "accountId" | "direction" | "amount" | "vatAmount" | "currency" | "fxQuantity" | "fxRate",
    v: string
  ) =>
    setLines((prev) =>
      prev.map((l, idx) => {
        if (idx !== i) return l;
        const next = { ...l, [f]: v };
        // Универсална статия: стойността е производна = количество × курс.
        // При смяна към основната валута (или празна) курсът става 1.
        if (f === "currency") {
          const c = v.trim().toUpperCase();
          if (c === "" || c === baseCurrency) next.fxRate = "1";
        }
        if (f === "fxQuantity" || f === "fxRate" || f === "currency") {
          const q = parseFloat(next.fxQuantity);
          const r = parseFloat(next.fxRate || "1");
          if (!Number.isNaN(q) && !Number.isNaN(r)) next.amount = (q * r).toFixed(2);
        }
        return next;
      })
    );

  // Курс от ЕЦБ (кръстосан през базовата валута) as-of датата на записа.
  const fetchFxRate = async (i: number) => {
    const l = lines[i];
    const cur = l.currency.trim().toUpperCase();
    if (!cur || !entryDate) return;
    try {
      const r = await api.get<{ rate: string; rate_date: string }>(
        `/v1/exchange-rates/cross?from=${encodeURIComponent(cur)}&to=${encodeURIComponent(baseCurrency)}&date=${entryDate}`
      );
      setLines((prev) =>
        prev.map((x, idx) => {
          if (idx !== i) return x;
          const next = { ...x, fxRate: r.rate };
          const q = parseFloat(x.fxQuantity);
          const rr = parseFloat(r.rate);
          if (!Number.isNaN(q) && !Number.isNaN(rr)) next.amount = (q * rr).toFixed(2);
          return next;
        })
      );
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    }
  };

  const accountIsCurrency = (id: string) =>
    accounts.find((a) => String(a.id) === id)?.is_currency === true;

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

  const filteredAccounts = useMemo(() => {
    const s = accountQuery.trim().toLowerCase();
    if (!s) return accounts;
    return accounts.filter((a) => `${a.number} ${a.name}`.toLowerCase().includes(s));
  }, [accounts, accountQuery]);

  const filteredEntries = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return entries;
    return entries.filter((en) =>
      `${en.entry_date} ${en.document_type} ${en.document_id} ${en.document_number ?? ""} ${en.description} ${en.counterpart_name}`
        .toLowerCase()
        .includes(s)
    );
  }, [entries, q]);

  const pageCount = Math.max(1, Math.ceil(filteredEntries.length / PAGE_SIZE));
  const page = Math.min(Math.floor(offset / PAGE_SIZE), pageCount - 1);
  const start = page * PAGE_SIZE;
  const pagedEntries = filteredEntries.slice(start, start + PAGE_SIZE);

  const selectableFiltered = useMemo(
    () => filteredEntries.filter(isManualEntry),
    [filteredEntries]
  );

  const allSelected =
    selectableFiltered.length > 0 &&
    selectableFiltered.every((en) => selected.includes(en.id));

  const toggleOne = (id: number) => {
    setSelected((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]
    );
  };

  const toggleAll = () => {
    setSelected(allSelected ? [] : selectableFiltered.map((en) => en.id));
  };

  const handleDelete = async (en: JournalEntry, e?: MouseEvent) => {
    e?.stopPropagation();
    if (!isManualEntry(en)) return;
    if (!window.confirm(t("common.confirmDelete"))) return;
    setError("");
    setNotice("");
    try {
      await api.del(`/v1/journal/${en.id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleBulkDelete = async () => {
    if (selected.length === 0) {
      setError(t("journal.bulk.none"));
      return;
    }
    if (!window.confirm(t("journal.bulk.confirm_delete"))) {
      return;
    }
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const res = await api.post<BulkResult>("/v1/journal/bulk/delete", {
        ids: selected,
      });
      const ok = res.deleted ?? 0;
      setNotice(
        `${t("journal.bulk.result")}: ${ok}, ${t("journal.bulk.failed")}: ${res.failed}`
      );
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

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
    if (lines.some((l) => !l.accountId)) {
      setFormError(t("journal.pick_account"));
      return;
    }
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
        lines: lines.map((l) => {
          const cur = l.currency.trim().toUpperCase();
          const foreign = cur !== "" && cur !== baseCurrency;
          // За основната валута количеството съвпада със стойността (курс=1).
          const quantity = l.fxQuantity !== "" ? l.fxQuantity : l.amount;
          return {
            account_id: Number(l.accountId),
            direction: l.direction,
            amount: l.amount,
            vat_amount: l.vatAmount || "0",
            currency: foreign ? cur : "",
            fx_quantity: quantity,
            fx_rate: foreign ? l.fxRate || "1" : "1",
            location_id: l.locationId || 0,
            product_id: l.productId || 0,
          };
        }),
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
      {notice && <div className="muted" style={{ marginBottom: 8 }}>{notice}</div>}
      <div className="card" style={{ marginBottom: 12 }}>
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t("common.search")}
        />
      </div>

      {selected.length > 0 && (
        <div className="card" style={{ padding: 12, marginBottom: 12 }}>
          <div className="btn-row wrap">
            <strong>
              {selected.length} {t("journal.bulk.selected")}
            </strong>
            <button
              className="btn btn-danger btn-sm"
              disabled={busy}
              onClick={() => void handleBulkDelete()}
            >
              {t("journal.bulk.delete")}
            </button>
            <button
              className="btn btn-ghost btn-sm"
              disabled={busy}
              onClick={() => setSelected([])}
            >
              {t("journal.bulk.clear")}
            </button>
          </div>
        </div>
      )}

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
                  <th style={{ width: 32 }}>
                    <input
                      type="checkbox"
                      checked={allSelected}
                      disabled={selectableFiltered.length === 0}
                      onChange={toggleAll}
                      title={t("journal.bulk.select_all")}
                    />
                  </th>
                  <th>{t("journal.entry_date")}</th>
                  <th>{t("journal.document")}</th>
                  <th>{t("journal.description")}</th>
                  <th>{t("journal.counterpart")}</th>
                  <th>{t("journal.vat_type")}</th>
                  <th>{t("audit.col_stamp")}</th>
                  <th>{t("common.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {pagedEntries.map((en) => {
                  const manual = isManualEntry(en);
                  return (
                    <tr
                      key={en.id}
                      onClick={() => void openEdit(en)}
                      style={{ cursor: "pointer" }}
                    >
                      <td onClick={(e) => e.stopPropagation()}>
                        {manual && (
                          <input
                            type="checkbox"
                            checked={selected.includes(en.id)}
                            onChange={() => toggleOne(en.id)}
                          />
                        )}
                      </td>
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
                      <td>
                        <AuditStampBadge
                          createdBy={en.created_by}
                          createdAt={en.created_at}
                          updatedBy={en.updated_by}
                          updatedAt={en.updated_at}
                          editCount={en.edit_count}
                        />
                      </td>
                      <td onClick={(e) => e.stopPropagation()}>
                        <div className="icon-actions">
                          <IconButton
                            icon={manual ? "edit" : "view"}
                            title={manual ? t("common.edit") : t("common.view")}
                            onClick={(e) => void openEdit(en, e)}
                          />
                          {manual && (
                            <IconButton
                              icon="delete"
                              title={t("common.delete")}
                              danger
                              onClick={(e) => void handleDelete(en, e)}
                            />
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
        <Pagination
          total={filteredEntries.length}
          limit={PAGE_SIZE}
          offset={start}
          onPageChange={setOffset}
        />
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

            {readOnly ? (
              <div className="muted" style={{ marginBottom: 14 }}>
                {t("journal.manual_only_hint")}
              </div>
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
                  disabled={loadingEntry || readOnly}
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
                    disabled={loadingEntry || readOnly}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("journal.description")} *</label>
                  <input
                    className="input"
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    required
                    disabled={loadingEntry || readOnly}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("journal.counterpart")}</label>
                  <select
                    className="select"
                    value={counterpartId}
                    onChange={(e) => setCounterpartId(e.target.value)}
                    disabled={loadingEntry || readOnly}
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
                      disabled={loadingEntry || readOnly}
                    />
                  </div>
                )}
                <div className="field">
                  <label className="label">{t("journal.document_number")}</label>
                  <input
                    className="input"
                    value={documentNumber}
                    onChange={(e) => setDocumentNumber(e.target.value)}
                    disabled={loadingEntry || readOnly}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("journal.document_date")}</label>
                  <input
                    className="input"
                    type="date"
                    value={documentDate}
                    onChange={(e) => setDocumentDate(e.target.value)}
                    disabled={loadingEntry || readOnly}
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
                  <span>{t("journal.quantity")}</span>
                  {tab !== "no_vat" ? <span>{t("journal.vat_amount")}</span> : null}
                  <span />
                </div>
                {lines.map((l, i) => (
                  <div key={i} className="journal-line-row">
                    <button
                      type="button"
                      className={`btn picker-trigger${l.accountId ? "" : " picker-empty"}`}
                      onClick={() => setAccountModalFor(i)}
                      disabled={loadingEntry || readOnly}
                    >
                      {accountLabel(accounts, l.accountId) ||
                        (l.accountId ? l.accountId : `${t("journal.account")}…`)}
                    </button>
                    <select
                      className="select"
                      value={l.direction}
                      onChange={(e) =>
                        setLine(i, "direction", e.target.value as "debit" | "credit")
                      }
                      disabled={loadingEntry || readOnly}
                    >
                      <option value="debit">{t("journal.debit")}</option>
                      <option value="credit">{t("journal.credit")}</option>
                    </select>
                    <input
                      className="input"
                      inputMode="decimal"
                      placeholder="0.00"
                      title={t("journal.quantity_hint")}
                      value={l.fxQuantity}
                      onChange={(e) => setLine(i, "fxQuantity", e.target.value)}
                      required
                      disabled={loadingEntry || readOnly}
                    />
                    {tab !== "no_vat" && (
                      <input
                        className="input"
                        inputMode="decimal"
                        placeholder={t("journal.vat_amount")}
                        title={t("journal.sign_hint")}
                        value={l.vatAmount}
                        onChange={(e) => setLine(i, "vatAmount", e.target.value)}
                        disabled={loadingEntry || readOnly}
                      />
                    )}
                    <button
                      type="button"
                      className="btn btn-danger btn-sm"
                      onClick={() => removeLine(i)}
                      disabled={loadingEntry || readOnly || lines.length < 2}
                    >
                      ✕
                    </button>
                    <div className="journal-fx-row">
                      <input
                        className="input"
                        placeholder={baseCurrency}
                        title={t("journal.currency")}
                        maxLength={3}
                        value={l.currency}
                        onChange={(e) => setLine(i, "currency", e.target.value.toUpperCase())}
                        disabled={loadingEntry || readOnly || !accountIsCurrency(l.accountId)}
                      />
                      <input
                        className="input"
                        inputMode="decimal"
                        placeholder={t("journal.fx_rate")}
                        title={t("journal.fx_rate")}
                        value={l.fxRate}
                        onChange={(e) => setLine(i, "fxRate", e.target.value)}
                        disabled={loadingEntry || readOnly || !accountIsCurrency(l.accountId)}
                      />
                      <input
                        className="input journal-value-derived"
                        inputMode="decimal"
                        placeholder={t("journal.amount")}
                        title={t("journal.value_derived_hint")}
                        value={l.amount}
                        readOnly
                        disabled={loadingEntry || readOnly}
                      />
                      <button
                        type="button"
                        className="btn btn-sm"
                        onClick={() => void fetchFxRate(i)}
                        disabled={loadingEntry || readOnly || !l.currency || !entryDate}
                        title={t("journal.fx_fetch")}
                      >
                        {t("journal.fx_fetch")}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
              {!readOnly && (
                <button
                  type="button"
                  className="btn btn-sm"
                  onClick={addLine}
                  disabled={loadingEntry}
                >
                  + {t("journal.add_line")}
                </button>
              )}

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
                {!readOnly && (
                  <button
                    className="btn btn-primary"
                    disabled={saving || !balanced || loadingEntry}
                  >
                    {t("common.save")}
                  </button>
                )}
              </div>
            </form>
            {editingId && activeEntryMeta && (
              <AuditHistoryPanel
                history={activeEntryMeta.audit_history}
                createdBy={activeEntryMeta.created_by}
                createdAt={activeEntryMeta.created_at}
                updatedBy={activeEntryMeta.updated_by}
                updatedAt={activeEntryMeta.updated_at}
                editCount={activeEntryMeta.edit_count}
              />
            )}
          </div>
        </div>
      )}

      {accountModalFor !== null && (
        <div
          className="modal-backdrop"
          onClick={() => {
            setAccountModalFor(null);
            setAccountQuery("");
          }}
        >
          <div className="card modal picker-modal" onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">{t("journal.pick_account")}</h2>
            <input
              className="input"
              autoFocus
              value={accountQuery}
              onChange={(e) => setAccountQuery(e.target.value)}
              placeholder={t("common.search")}
            />
            <div className="picker-list">
              {filteredAccounts.map((a) => (
                <button
                  type="button"
                  key={a.id}
                  className="picker-item"
                  onClick={() => {
                    setLine(accountModalFor, "accountId", String(a.id));
                    setAccountModalFor(null);
                    setAccountQuery("");
                  }}
                >
                  <b>{a.number}</b>
                  <span className="muted">{a.name}</span>
                </button>
              ))}
              {filteredAccounts.length === 0 && (
                <div className="muted">{t("common.empty")}</div>
              )}
            </div>
            <div className="form-actions" style={{ marginTop: 12 }}>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  setAccountModalFor(null);
                  setAccountQuery("");
                }}
              >
                {t("common.cancel")}
              </button>
            </div>
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
