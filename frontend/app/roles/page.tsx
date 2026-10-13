"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { useAuth } from "../../components/AuthProvider";
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
    titleKey: "roles.g.invoices",
    items: [
      { id: "invoices", labelKey: "roles.p.invoices" },
      { id: "counterparts", labelKey: "roles.p.counterparts" },
      { id: "products", labelKey: "roles.p.products" },
      { id: "scans", labelKey: "roles.p.scans" },
    ],
  },
  {
    titleKey: "roles.g.accounting",
    items: [
      { id: "accounts", labelKey: "roles.p.accounts" },
      { id: "journal", labelKey: "roles.p.journal" },
      { id: "periods", labelKey: "roles.p.periods" },
      { id: "vat", labelKey: "roles.p.vat" },
    ],
  },
  {
    titleKey: "roles.g.reports",
    items: [
      { id: "reports", labelKey: "roles.p.reports" },
      { id: "saft", labelKey: "roles.p.saft" },
    ],
  },
  {
    titleKey: "roles.g.ops",
    items: [
      { id: "stock", labelKey: "roles.p.stock" },
      { id: "production", labelKey: "roles.p.production" },
      { id: "assets", labelKey: "roles.p.assets" },
      { id: "banking", labelKey: "roles.p.banking" },
      { id: "dividends", labelKey: "roles.p.dividends" },
    ],
  },
  {
    titleKey: "roles.g.admin",
    items: [
      { id: "companies", labelKey: "roles.p.companies" },
      { id: "users", labelKey: "roles.p.users" },
      { id: "settings", labelKey: "roles.p.settings" },
      { id: "rates", labelKey: "roles.p.rates" },
    ],
  },
];

const ALL_PERM = "*";

const KNOWN_PERMS = new Map<string, string>(
  GROUPS.flatMap((g) => g.items.map((it) => [it.id, it.labelKey] as [string, string]))
);

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
    setPerms((prev) => {
      if (id === ALL_PERM) {
        return prev.includes(ALL_PERM)
          ? prev.filter((p) => p !== ALL_PERM)
          : [ALL_PERM];
      }
      return prev
        .filter((p) => p !== ALL_PERM)
        .includes(id)
        ? prev.filter((p) => p !== id && p !== ALL_PERM)
        : [...prev.filter((p) => p !== ALL_PERM), id];
    });
  };

  const labelOf = (id: string) => {
    const key = KNOWN_PERMS.get(id);
    return key ? t(key) : id;
  };

  const permsOf = (r: Role) => {
    const list = asPerms(r.permissions);
    if (list.includes(ALL_PERM)) return t("roles.p.all");
    return list.map(labelOf).join(", ");
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
                  <td>{permsOf(r) || <span className="muted">—</span>}</td>
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
              <div style={{ marginTop: 12 }}>
                <div className="section-title">{t("roles.perms")}</div>
                <label className="label" style={{ display: "flex", gap: 6 }}>
                  <input
                    type="checkbox"
                    checked={perms.includes(ALL_PERM)}
                    onChange={() => toggle(ALL_PERM)}
                  />
                  {t("roles.p.all")}
                </label>
              </div>
              {!perms.includes(ALL_PERM) && (
                <>
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
                  {perms.some((p) => !KNOWN_PERMS.has(p)) && (
                    <div style={{ marginTop: 12 }}>
                      <div className="section-title">{t("roles.p.other")}</div>
                      <div style={{ display: "flex", flexWrap: "wrap", gap: 10 }}>
                        {perms
                          .filter((p) => !KNOWN_PERMS.has(p))
                          .map((p) => (
                            <label key={p} className="label" style={{ display: "flex", gap: 6 }}>
                              <input
                                type="checkbox"
                                checked
                                onChange={() => toggle(p)}
                              />
                              {p}
                            </label>
                          ))}
                      </div>
                    </div>
                  )}
                </>
              )}
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

function RolesGate() {
  const { t } = useI18n();
  const { isSuperAdmin } = useAuth();
  if (!isSuperAdmin) {
    return (
      <div className="card card-pad">
        <p className="muted" style={{ margin: 0 }}>
          {t("common.super_only")}
        </p>
      </div>
    );
  }
  return <RolesInner />;
}

export default function RolesPage() {
  return (
    <RequireAuth>
      <RolesGate />
    </RequireAuth>
  );
}
