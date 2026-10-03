"use client";

// Сметкоплан: CRUD + търсачка към SAF-T номенклатурата на НАП +
// бутон „Зареди SAF-T сметкоплан“ (като secret/su-doxis).

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { SaftAccountPicker, SaftNomAccount } from "../../components/SaftAccountPicker";
import { useI18n } from "../../components/I18nProvider";
import { api, ListResponse } from "../../lib/api";
import { IconButton } from "../../components/IconButton";

interface Account {
  id: number;
  number: string;
  name: string;
  saft_account_type: string;
  analytic_type: string;
}

const emptyForm = {
  number: "",
  name: "",
  saft_account_type: "",
  analytic_type: "none",
};

function AccountsInner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<Account[]>([]);
  const [noms, setNoms] = useState<SaftNomAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [msg, setMsg] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Account | null>(null);
  const [form, setForm] = useState(emptyForm);
  const [saving, setSaving] = useState(false);
  const [seeding, setSeeding] = useState(false);
  const [formError, setFormError] = useState("");
  const [q, setQ] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [a, n] = await Promise.all([
        api.get<ListResponse<Account>>("/v1/accounts"),
        api.get<ListResponse<SaftNomAccount>>(
          "/v1/saft/nomenclatures?kind=accounts"
        ),
      ]);
      setRows(a.items ?? []);
      setNoms(n.items ?? []);
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

  const openEdit = (acc: Account) => {
    setForm({
      number: acc.number,
      name: acc.name,
      saft_account_type: acc.saft_account_type ?? "",
      analytic_type: acc.analytic_type || "none",
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
        number: form.number,
        name: form.name,
        saft_account_type: form.saft_account_type,
        analytic_type: form.analytic_type,
      };
      if (editing) {
        await api.patch(`/v1/accounts/${editing.id}`, payload);
      } else {
        await api.post("/v1/accounts", payload);
      }
      setModalOpen(false);
      await load();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (acc: Account) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/accounts/${acc.id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSeed = async () => {
    setSeeding(true);
    setError("");
    setMsg("");
    try {
      const r = await api.post<{ created: number; skipped: number; total: number }>(
        "/v1/accounts/seed-saft"
      );
      if (r.created > 0) {
        setMsg(t("accounts.seed.ok").replace("{n}", String(r.created)));
      } else {
        setMsg(t("accounts.seed.none"));
      }
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSeeding(false);
    }
  };

  const pickSaft = (code: string, acc: SaftNomAccount | null) => {
    setForm((prev) => ({
      ...prev,
      saft_account_type: code,
      number:
        !editing && prev.number === "" && acc ? acc.code : prev.number,
      name: !editing && prev.name === "" && acc ? acc.name : prev.name,
    }));
  };

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((a) =>
      `${a.number} ${a.name} ${a.saft_account_type} ${a.analytic_type}`
        .toLowerCase()
        .includes(s)
    );
  }, [rows, q]);

  const analyticLabel = (v: string) => {
    if (v === "counterpart") return t("accounts.analytic.counterpart");
    if (v === "product") return t("accounts.analytic.product");
    return t("accounts.analytic.none");
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("accounts.title")}</h1>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="btn"
            onClick={handleSeed}
            disabled={seeding}
            type="button"
          >
            {seeding ? t("accounts.seed.busy") : t("accounts.seed")}
          </button>
          <button className="btn btn-primary" onClick={openCreate} type="button">
            {t("common.create")}
          </button>
        </div>
      </div>
      {error && <div className="error-text">{error}</div>}
      {msg && <div className="muted" style={{ marginBottom: 12 }}>{msg}</div>}
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
            {rows.length === 0 ? t("accounts.empty") : t("common.empty")}
          </div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("accounts.number")}</th>
                <th>{t("accounts.name")}</th>
                <th>{t("accounts.saft_account_type")}</th>
                <th>{t("accounts.analytic_type")}</th>
                <th>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((acc) => (
                <tr key={acc.id}>
                  <td style={{ fontFamily: "ui-monospace, monospace" }}>
                    {acc.number}
                  </td>
                  <td>{acc.name}</td>
                  <td style={{ fontFamily: "ui-monospace, monospace" }}>
                    {acc.saft_account_type}
                  </td>
                  <td>{analyticLabel(acc.analytic_type)}</td>
                  <td>
                    <div className="icon-actions">
                      <IconButton
                        icon="edit"
                        title={t("common.edit")}
                        onClick={() => openEdit(acc)}
                      />
                      <IconButton
                        icon="delete"
                        title={t("common.delete")}
                        danger
                        onClick={() => handleDelete(acc)}
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
            style={{ maxWidth: 640 }}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">
              {editing ? t("common.edit") : t("common.create")}
            </h2>
            <form onSubmit={handleSave}>
              <div className="field" style={{ gridColumn: "1 / -1" }}>
                <label className="label">{t("accounts.saft_account_type")}</label>
                <SaftAccountPicker
                  value={form.saft_account_type}
                  accounts={noms}
                  onChange={pickSaft}
                />
              </div>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("accounts.number")} *</label>
                  <input
                    className="input"
                    required
                    value={form.number}
                    onChange={(e) => setForm({ ...form, number: e.target.value })}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("accounts.name")} *</label>
                  <input
                    className="input"
                    required
                    value={form.name}
                    onChange={(e) => setForm({ ...form, name: e.target.value })}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("accounts.analytic_type")}</label>
                  <select
                    className="select"
                    value={form.analytic_type}
                    onChange={(e) =>
                      setForm({ ...form, analytic_type: e.target.value })
                    }
                  >
                    <option value="none">{t("accounts.analytic.none")}</option>
                    <option value="counterpart">
                      {t("accounts.analytic.counterpart")}
                    </option>
                    <option value="product">{t("accounts.analytic.product")}</option>
                  </select>
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

export default function AccountsPage() {
  return (
    <RequireAuth>
      <AccountsInner />
    </RequireAuth>
  );
}
