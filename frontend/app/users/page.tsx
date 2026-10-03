"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { api, ListResponse } from "../../lib/api";
import { IconButton } from "../../components/IconButton";

interface Role {
  id: number;
  name: string;
}

interface User {
  id: number;
  email: string;
  name: string;
  is_active: number;
  is_super_admin: number;
  role_id: number;
}

const empty = {
  email: "",
  name: "",
  password: "",
  is_active: true,
  is_super_admin: false,
  role_id: 0,
};

function UsersInner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<User[]>([]);
  const [roles, setRoles] = useState<Role[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [modal, setModal] = useState(false);
  const [editing, setEditing] = useState<User | null>(null);
  const [form, setForm] = useState(empty);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");
  const [q, setQ] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [u, r] = await Promise.all([
        api.get<ListResponse<User>>("/v1/users"),
        api.get<ListResponse<Role>>("/v1/roles"),
      ]);
      setRows(u.items ?? []);
      setRoles(r.items ?? []);
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
    setForm(empty);
    setEditing(null);
    setFormError("");
    setModal(true);
  };

  const openEdit = (u: User) => {
    setForm({
      email: u.email,
      name: u.name,
      password: "",
      is_active: Number(u.is_active) === 1,
      is_super_admin: Number(u.is_super_admin) === 1,
      role_id: u.role_id || 0,
    });
    setEditing(u);
    setFormError("");
    setModal(true);
  };

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError("");
    try {
      const payload: Record<string, unknown> = {
        email: form.email,
        name: form.name,
        is_active: form.is_active ? 1 : 0,
        is_super_admin: form.is_super_admin ? 1 : 0,
        role_id: form.role_id,
      };
      if (form.password) payload.password = form.password;
      if (editing) {
        await api.patch(`/v1/users/${editing.id}`, payload);
      } else {
        await api.post("/v1/users", payload);
      }
      setModal(false);
      await load();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const del = async (u: User) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/users/${u.id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const roleName = (id: number) => roles.find((r) => r.id === id)?.name ?? "";

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((u) =>
      `${u.email} ${u.name} ${roleName(u.role_id)}`.toLowerCase().includes(s)
    );
  }, [rows, q, roles]);

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("users.title")}</h1>
        <button className="btn btn-primary" type="button" onClick={openCreate}>
          {t("common.create")}
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
        ) : filtered.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("users.name")}</th>
                <th>{t("users.email")}</th>
                <th>{t("users.role")}</th>
                <th>{t("users.active")}</th>
                <th>{t("users.super")}</th>
                <th>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((u) => (
                <tr key={u.id}>
                  <td>{u.name}</td>
                  <td>{u.email}</td>
                  <td>{roleName(u.role_id)}</td>
                  <td>{Number(u.is_active) === 1 ? "✓" : ""}</td>
                  <td>{Number(u.is_super_admin) === 1 ? "✓" : ""}</td>
                  <td>
                    <div className="icon-actions">
                      <IconButton icon="edit" title={t("common.edit")} onClick={() => openEdit(u)} />
                      <IconButton
                        icon="delete"
                        title={t("common.delete")}
                        danger
                        onClick={() => void del(u)}
                      />
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {modal && (
        <div className="modal-backdrop" onClick={() => setModal(false)}>
          <div className="card modal" onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">
              {editing ? t("common.edit") : t("common.create")}
            </h2>
            <form onSubmit={save}>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("users.email")} *</label>
                  <input
                    className="input"
                    required
                    value={form.email}
                    onChange={(e) => setForm({ ...form, email: e.target.value })}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("users.name")} *</label>
                  <input
                    className="input"
                    required
                    value={form.name}
                    onChange={(e) => setForm({ ...form, name: e.target.value })}
                  />
                </div>
                <div className="field">
                  <label className="label">
                    {t("users.password")}
                    {editing ? "" : " *"}
                  </label>
                  <input
                    className="input"
                    type="password"
                    required={!editing}
                    value={form.password}
                    onChange={(e) => setForm({ ...form, password: e.target.value })}
                    placeholder={editing ? t("users.password_keep") : ""}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("users.role")}</label>
                  <select
                    className="select"
                    value={form.role_id}
                    onChange={(e) =>
                      setForm({ ...form, role_id: Number(e.target.value) })
                    }
                  >
                    <option value={0}>{t("users.role_none")}</option>
                    {roles.map((r) => (
                      <option key={r.id} value={r.id}>
                        {r.name}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              <label className="label" style={{ display: "flex", gap: 8, marginTop: 8 }}>
                <input
                  type="checkbox"
                  checked={form.is_active}
                  onChange={(e) => setForm({ ...form, is_active: e.target.checked })}
                />
                {t("users.active")}
              </label>
              <label className="label" style={{ display: "flex", gap: 8 }}>
                <input
                  type="checkbox"
                  checked={form.is_super_admin}
                  onChange={(e) =>
                    setForm({ ...form, is_super_admin: e.target.checked })
                  }
                />
                {t("users.super")}
              </label>
              {formError && <div className="error-text">{formError}</div>}
              <div className="form-actions">
                <button type="button" className="btn" onClick={() => setModal(false)}>
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

export default function UsersPage() {
  return (
    <RequireAuth>
      <UsersInner />
    </RequireAuth>
  );
}
