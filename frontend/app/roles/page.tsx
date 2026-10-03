"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { api, ListResponse } from "../../lib/api";
import { IconButton } from "../../components/IconButton";

interface Role {
  id: number;
  name: string;
  description: string;
  permissions: string[];
}

const GROUPS: { titleKey: string; items: { id: string; labelKey: string }[] }[] = [
  {
    titleKey: "roles.g.admin",
    items: [
      { id: "admin:read", labelKey: "roles.p.read" },
      { id: "admin:create", labelKey: "roles.p.create" },
      { id: "admin:update", labelKey: "roles.p.update" },
      { id: "admin:delete", labelKey: "roles.p.delete" },
    ],
  },
  {
    titleKey: "roles.g.accounting",
    items: [
      { id: "accounting:read", labelKey: "roles.p.read" },
      { id: "accounting:write", labelKey: "roles.p.write" },
      { id: "accounting:post", labelKey: "roles.p.post" },
    ],
  },
  {
    titleKey: "roles.g.invoices",
    items: [
      { id: "invoice:read", labelKey: "roles.p.read" },
      { id: "invoice:create", labelKey: "roles.p.create" },
      { id: "invoice:update", labelKey: "roles.p.update" },
      { id: "invoice:delete", labelKey: "roles.p.delete" },
    ],
  },
  {
    titleKey: "roles.g.documents",
    items: [
      { id: "document:read", labelKey: "roles.p.read" },
      { id: "document:upload", labelKey: "roles.p.upload" },
    ],
  },
  {
    titleKey: "roles.g.reports",
    items: [
      { id: "report:read", labelKey: "roles.p.read" },
      { id: "report:export", labelKey: "roles.p.export" },
    ],
  },
  {
    titleKey: "roles.g.vat",
    items: [
      { id: "vat:read", labelKey: "roles.p.read" },
      { id: "vat:create", labelKey: "roles.p.create" },
      { id: "vat:submit", labelKey: "roles.p.submit" },
    ],
  },
  {
    titleKey: "roles.g.settings",
    items: [
      { id: "settings:read", labelKey: "roles.p.read" },
      { id: "settings:update", labelKey: "roles.p.update" },
    ],
  },
  {
    titleKey: "roles.g.users",
    items: [
      { id: "user:read", labelKey: "roles.p.read" },
      { id: "user:create", labelKey: "roles.p.create" },
      { id: "user:update", labelKey: "roles.p.update" },
      { id: "user:delete", labelKey: "roles.p.delete" },
    ],
  },
  {
    titleKey: "roles.g.companies",
    items: [
      { id: "company:read", labelKey: "roles.p.read" },
      { id: "company:create", labelKey: "roles.p.create" },
      { id: "company:update", labelKey: "roles.p.update" },
      { id: "company:delete", labelKey: "roles.p.delete" },
    ],
  },
  {
    titleKey: "roles.g.roles",
    items: [
      { id: "role:read", labelKey: "roles.p.read" },
      { id: "role:create", labelKey: "roles.p.create" },
      { id: "role:update", labelKey: "roles.p.update" },
      { id: "role:delete", labelKey: "roles.p.delete" },
    ],
  },
];

function asPerms(v: unknown): string[] {
  if (Array.isArray(v)) return v.map(String);
  if (typeof v === "string") {
    try {
      const p = JSON.parse(v);
      return Array.isArray(p) ? p.map(String) : [];
    } catch {
      return [];
    }
  }
  return [];
}

function RolesInner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<Role[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [modal, setModal] = useState(false);
  const [editing, setEditing] = useState<Role | null>(null);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [perms, setPerms] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const r = await api.get<ListResponse<Role>>("/v1/roles");
      setRows(
        (r.items ?? []).map((x) => ({ ...x, permissions: asPerms(x.permissions) }))
      );
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
    setEditing(null);
    setName("");
    setDescription("");
    setPerms([]);
    setFormError("");
    setModal(true);
  };

  const openEdit = (r: Role) => {
    setEditing(r);
    setName(r.name);
    setDescription(r.description ?? "");
    setPerms(asPerms(r.permissions));
    setFormError("");
    setModal(true);
  };

  const toggle = (id: string) => {
    setPerms((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id]
    );
  };

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError("");
    try {
      const payload = { name, description, permissions: perms };
      if (editing) await api.patch(`/v1/roles/${editing.id}`, payload);
      else await api.post("/v1/roles", payload);
      setModal(false);
      await load();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const del = async (r: Role) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/roles/${r.id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("roles.title")}</h1>
        <button className="btn btn-primary" type="button" onClick={openCreate}>
          {t("common.create")}
        </button>
      </div>
      {error && <div className="error-text">{error}</div>}
      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : rows.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("roles.name")}</th>
                <th>{t("roles.description")}</th>
                <th>{t("roles.perms")}</th>
                <th>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.id}>
                  <td>{r.name}</td>
                  <td>{r.description}</td>
                  <td>{asPerms(r.permissions).length}</td>
                  <td>
                    <div className="icon-actions">
                      <IconButton icon="edit" title={t("common.edit")} onClick={() => openEdit(r)} />
                      <IconButton
                        icon="delete"
                        title={t("common.delete")}
                        danger
                        onClick={() => void del(r)}
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
          <div
            className="card modal"
            style={{ maxWidth: 720 }}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="modal-title">
              {editing ? t("common.edit") : t("common.create")}
            </h2>
            <form onSubmit={save}>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("roles.name")} *</label>
                  <input
                    className="input"
                    required
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("roles.description")}</label>
                  <input
                    className="input"
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                  />
                </div>
              </div>
              {GROUPS.map((g) => (
                <div key={g.titleKey} style={{ marginTop: 12 }}>
                  <div className="section-title">{t(g.titleKey)}</div>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 10 }}>
                    {g.items.map((it) => (
                      <label key={it.id} className="label" style={{ display: "flex", gap: 6 }}>
                        <input
                          type="checkbox"
                          checked={perms.includes(it.id)}
                          onChange={() => toggle(it.id)}
                        />
                        {t(it.labelKey)}
                      </label>
                    ))}
                  </div>
                </div>
              ))}
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

export default function RolesPage() {
  return (
    <RequireAuth>
      <RolesInner />
    </RequireAuth>
  );
}
