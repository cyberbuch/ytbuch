"use client";

// Банкови сметки: CRUD + избор на счетоводна (ГЛ) и буферна сметка.
// Порт на secret/su-doxis bank_accounts.rs върху bagabuch REST API.

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { api, ListResponse } from "../../lib/api";
import { IconButton } from "../../components/IconButton";

interface BankAccount {
  id: number;
  iban: string;
  bic: string;
  bank_name: string;
  currency: string;
  name: string;
  account_number: string;
  gl_account_id: number;
  buffer_account_id: number;
  balance: string;
  gl_account_code?: string;
  buffer_account_code?: string;
}

interface Account {
  id: number;
  number: string;
  name: string;
}

const emptyForm = {
  name: "",
  bank_name: "",
  account_number: "",
  iban: "",
  bic: "",
  currency: "BGN",
  balance: "0.00",
  gl_account_id: 0,
  buffer_account_id: 0,
};

function BankAccountsInner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<BankAccount[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<BankAccount | null>(null);
  const [form, setForm] = useState(emptyForm);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");
  const [q, setQ] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [b, a] = await Promise.all([
        api.get<ListResponse<BankAccount>>("/v1/bank-accounts"),
        api.get<ListResponse<Account>>("/v1/accounts"),
      ]);
      setRows(b.items ?? []);
      setAccounts(a.items ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const openCreate = () => {
    setForm(emptyForm);
    setEditing(null);
    setFormError("");
    setModalOpen(true);
  };

  const openEdit = (acc: BankAccount) => {
    setForm({
      name: acc.name,
      bank_name: acc.bank_name ?? "",
      account_number: acc.account_number ?? "",
      iban: acc.iban,
      bic: acc.bic ?? "",
      currency: acc.currency || "BGN",
      balance: acc.balance || "0.00",
      gl_account_id: acc.gl_account_id || 0,
      buffer_account_id: acc.buffer_account_id || 0,
    });
    setEditing(acc);
    setFormError("");
    setModalOpen(true);
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError("");
    try {
      const payload = {
        name: form.name,
        bank_name: form.bank_name,
        account_number: form.account_number,
        iban: form.iban,
        bic: form.bic,
        currency: form.currency,
        balance: form.balance,
        gl_account_id: form.gl_account_id,
        buffer_account_id: form.buffer_account_id,
      };
      if (editing) {
        await api.patch(`/v1/bank-accounts/${editing.id}`, payload);
      } else {
        await api.post("/v1/bank-accounts", payload);
      }
      setModalOpen(false);
      await load();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (acc: BankAccount) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/bank-accounts/${acc.id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((b) =>
      `${b.name} ${b.iban} ${b.bank_name} ${b.bic}`
        .toLowerCase()
        .includes(s)
    );
  }, [rows, q]);

  const accountCode = (id: number) => {
    if (!id) return "-";
    const a = accounts.find((x) => x.id === id);
    return a?.number || "-";
  };

  const accountOptions = (value: number, onChange: (v: number) => void) => (
    <select
      className="select"
      value={value || 0}
      onChange={(e) => onChange(Number(e.target.value))}
    >
      <option value={0}>{t("bank_accounts.pick_account")}</option>
      {accounts.map((a) => (
        <option key={a.id} value={a.id}>
          {a.number} — {a.name}
        </option>
      ))}
    </select>
  );

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("bank_accounts.title")}</h1>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-primary" onClick={openCreate} type="button">
            {t("common.create")}
          </button>
        </div>
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
        ) : filtered.length === 0 ? (
          <div className="content muted">
            {rows.length === 0 ? t("bank_accounts.empty") : t("common.empty")}
          </div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("bank_accounts.name")}</th>
                <th>{t("bank_accounts.bank_name")}</th>
                <th>{t("bank_accounts.iban")}</th>
                <th>{t("bank_accounts.gl_account")}</th>
                <th>{t("bank_accounts.buffer_account")}</th>
                <th style={{ textAlign: "right" }}>
                  {t("bank_accounts.balance")}
                </th>
                <th>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((b) => (
                <tr key={b.id}>
                  <td style={{ fontWeight: 500 }}>{b.name}</td>
                  <td>{b.bank_name}</td>
                  <td style={{ fontFamily: "ui-monospace, monospace" }}>
                    {b.iban}
                  </td>
                  <td style={{ fontFamily: "ui-monospace, monospace" }}>
                    {accountCode(b.gl_account_id)}
                  </td>
                  <td style={{ fontFamily: "ui-monospace, monospace" }}>
                    {accountCode(b.buffer_account_id)}
                  </td>
                  <td style={{ textAlign: "right", fontWeight: 500 }}>
                    {b.balance} {b.currency}
                  </td>
                  <td>
                    <div className="icon-actions">
                      <IconButton
                        icon="edit"
                        title={t("common.edit")}
                        onClick={() => openEdit(b)}
                      />
                      <IconButton
                        icon="delete"
                        title={t("common.delete")}
                        danger
                        onClick={() => handleDelete(b)}
                      />
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {modalOpen && (
        <div className="modal-backdrop" onClick={() => setModalOpen(false)}>
          <div
            className="card modal"
            style={{ maxWidth: 680 }}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">
              {editing ? t("common.edit") : t("common.create")}
            </h2>
            <form onSubmit={handleSave}>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("bank_accounts.name")} *</label>
                  <input
                    className="input"
                    required
                    value={form.name}
                    onChange={(e) => setForm({ ...form, name: e.target.value })}
                    placeholder={t("bank_accounts.name_placeholder")}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("bank_accounts.bank_name")}</label>
                  <input
                    className="input"
                    value={form.bank_name}
                    onChange={(e) =>
                      setForm({ ...form, bank_name: e.target.value })
                    }
                    placeholder={t("bank_accounts.bank_name_placeholder")}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("bank_accounts.iban")} *</label>
                  <input
                    className="input"
                    required
                    value={form.iban}
                    onChange={(e) => setForm({ ...form, iban: e.target.value })}
                    placeholder={t("bank_accounts.iban_placeholder")}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("bank_accounts.bic")}</label>
                  <input
                    className="input"
                    value={form.bic}
                    onChange={(e) => setForm({ ...form, bic: e.target.value })}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("bank_accounts.currency")}</label>
                  <input
                    className="input"
                    value={form.currency}
                    onChange={(e) =>
                      setForm({ ...form, currency: e.target.value })
                    }
                  />
                </div>
                <div className="field">
                  <label className="label">{t("bank_accounts.balance")}</label>
                  <input
                    className="input"
                    type="number"
                    step="0.01"
                    value={form.balance}
                    onChange={(e) =>
                      setForm({ ...form, balance: e.target.value })
                    }
                  />
                </div>
                <div className="field">
                  <label className="label">{t("bank_accounts.gl_account")}</label>
                  {accountOptions(form.gl_account_id, (v) =>
                    setForm({ ...form, gl_account_id: v })
                  )}
                </div>
                <div className="field">
                  <label className="label">
                    {t("bank_accounts.buffer_account")}
                  </label>
                  {accountOptions(form.buffer_account_id, (v) =>
                    setForm({ ...form, buffer_account_id: v })
                  )}
                </div>
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
                <button className="btn btn-primary" disabled={saving}>
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

export default function BankAccountsPage() {
  return (
    <RequireAuth>
      <BankAccountsInner />
    </RequireAuth>
  );
}
