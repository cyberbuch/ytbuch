"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { useI18n } from "./I18nProvider";
import { ACTIVE_COMPANY_EVENT, api } from "../lib/api";

interface Role {
  id: number;
  name: string;
  description: string;
}

interface Member {
  user_id: number;
  email: string;
  name: string;
  is_active: number;
  role_id: number;
  role: string;
  created_at: string;
}

interface MembersPack {
  items: Member[];
  roles: Role[];
  total?: number;
}

export function SettingsUsersTab({ companyId }: { companyId: number }) {
  const { t } = useI18n();
  const [members, setMembers] = useState<Member[]>([]);
  const [roles, setRoles] = useState<Role[]>([]);
  const [email, setEmail] = useState("");
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [roleId, setRoleId] = useState(0);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setError("");
    try {
      const pack = await api.get<MembersPack>(`/v1/companies/${companyId}/users`, {
        limit: 1000,
      });
      setMembers(pack.items ?? []);
      setRoles(pack.roles ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [companyId]);

  useEffect(() => {
    load();
    const onChange = () => load();
    window.addEventListener(ACTIVE_COMPANY_EVENT, onChange);
    return () => window.removeEventListener(ACTIVE_COMPANY_EVENT, onChange);
  }, [load]);

  async function add(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError("");
    try {
      const body: Record<string, unknown> = { email, role_id: roleId };
      if (name) body.name = name;
      if (password) body.password = password;
      await api.post(`/v1/companies/${companyId}/users`, body);
      setEmail("");
      setName("");
      setPassword("");
      setRoleId(0);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  async function changeRole(userId: number, nextRoleId: number) {
    setError("");
    try {
      await api.patch(`/v1/companies/${companyId}/users/${userId}`, {
        role_id: nextRoleId,
      });
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function remove(userId: number) {
    if (!confirm(t("common.confirmDelete"))) return;
    setError("");
    try {
      await api.del(`/v1/companies/${companyId}/users/${userId}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <div className="card card-pad" style={{ maxWidth: 880 }}>
      <h2 style={{ marginTop: 0 }}>{t("settings.users.title")}</h2>
      <p className="muted">{t("settings.users.hint")}</p>
      {error && <div className="error-text">{error}</div>}
      <form onSubmit={add}>
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("users.email")} *</label>
            <input
              className="input"
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </div>
          <div className="field">
            <label className="label">{t("users.name")}</label>
            <input
              className="input"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="field">
            <label className="label">{t("users.password")}</label>
            <input
              className="input"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>
          <div className="field">
            <label className="label">{t("users.role")} *</label>
            <select
              className="select"
              value={String(roleId)}
              onChange={(e) => setRoleId(Number(e.target.value) || 0)}
            >
              <option value="0">{t("users.role_none")}</option>
              {roles.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.name}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="form-actions">
          <button
            className="btn btn-primary"
            disabled={saving || !email || roleId <= 0}
          >
            {t("settings.users.add")}
          </button>
        </div>
      </form>
      {members.length === 0 ? (
        <div className="muted">{t("settings.users.empty")}</div>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>{t("users.name")}</th>
              <th>{t("users.email")}</th>
              <th>{t("users.role")}</th>
              <th>{t("common.actions")}</th>
            </tr>
          </thead>
          <tbody>
            {members.map((m) => (
              <tr key={m.user_id}>
                <td>{m.name}</td>
                <td>{m.email}</td>
                <td>
                  <select
                    className="select"
                    value={String(m.role_id)}
                    onChange={(e) =>
                      changeRole(m.user_id, Number(e.target.value) || 0)
                    }
                  >
                    {roles.map((r) => (
                      <option key={r.id} value={r.id}>
                        {r.name}
                      </option>
                    ))}
                  </select>
                </td>
                <td>
                  <button
                    type="button"
                    className="btn"
                    onClick={() => remove(m.user_id)}
                  >
                    {t("settings.users.remove")}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
