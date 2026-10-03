"use client";

// Глобални настройки: приложение, SMTP, S3 архиви.
// SMTP/S3 са Python sidecar; архивът е boilaDB COPY dump, не LSM tar.

import { FormEvent, useCallback, useEffect, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { api, ListResponse } from "../../lib/api";
import { IconButton } from "../../components/IconButton";

type Tab = "app" | "smtp" | "s3" | "auth";

interface Setting {
  key: string;
  value: string;
}

interface Backup {
  key: string;
  filename: string;
  size_human: string;
  last_modified: string;
}

const KEYS = {
  app: ["app.name", "app.url", "app.language"],
  smtp: [
    "smtp.host",
    "smtp.port",
    "smtp.username",
    "smtp.password",
    "smtp.from_email",
    "smtp.from_name",
    "smtp.use_tls",
    "smtp.enabled",
  ],
  s3: [
    "s3.endpoint",
    "s3.bucket",
    "s3.access_key",
    "s3.secret_key",
    "s3.region",
    "s3.prefix",
  ],
  auth: ["auth.registration_enabled"],
};

function emptyMap(keys: string[]): Record<string, string> {
  const o: Record<string, string> = {};
  for (const k of keys) o[k] = "";
  return o;
}

function AdminInner() {
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>("app");
  const [form, setForm] = useState<Record<string, string>>({
    ...emptyMap(KEYS.app),
    ...emptyMap(KEYS.smtp),
    ...emptyMap(KEYS.s3),
    ...emptyMap(KEYS.auth),
    "smtp.port": "587",
    "smtp.use_tls": "1",
    "s3.region": "us-east-1",
    "s3.prefix": "backups/",
    "auth.registration_enabled": "1",
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [busy, setBusy] = useState("");
  const [msg, setMsg] = useState("");
  const [error, setError] = useState("");
  const [testTo, setTestTo] = useState("");
  const [backups, setBackups] = useState<Backup[]>([]);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<ListResponse<Setting>>("/v1/system-settings");
      setForm((prev) => {
        const next = { ...prev };
        for (const s of data.items ?? []) {
          if (s.key) next[s.key] = s.value ?? "";
        }
        return next;
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const set = (key: string, value: string) =>
    setForm((prev) => ({ ...prev, [key]: value }));

  const saveKeys = async (keys: string[]) => {
    setSaving(true);
    setError("");
    setMsg("");
    try {
      const items = keys.map((key) => ({ key, value: form[key] ?? "" }));
      await api.put("/v1/system-settings", { items });
      setMsg(t("admin.saved"));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const smtpTest = async (e: FormEvent) => {
    e.preventDefault();
    setBusy("smtp");
    setError("");
    setMsg("");
    try {
      await saveKeys(KEYS.smtp);
      const r = await api.post<{ message: string }>("/v1/system-settings/smtp-test", {
        to: testTo,
      });
      setMsg(r.message || t("admin.smtp.ok"));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const s3Test = async () => {
    setBusy("s3test");
    setError("");
    setMsg("");
    try {
      await saveKeys(KEYS.s3);
      const r = await api.post<{ message: string }>("/v1/system-settings/s3-test");
      setMsg(r.message || t("admin.s3.ok"));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const s3Backup = async () => {
    setBusy("backup");
    setError("");
    setMsg("");
    try {
      await saveKeys(KEYS.s3);
      const r = await api.post<{ message: string }>("/v1/system-settings/s3-backup");
      setMsg(r.message || t("admin.s3.backup_ok"));
      await loadBackups();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const loadBackups = async () => {
    try {
      const r = await api.get<{ items?: Backup[] }>("/v1/system-settings/s3-backups");
      setBackups(r.items ?? []);
    } catch {
      setBackups([]);
    }
  };

  const delBackup = async (key: string) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    setBusy("del");
    setError("");
    try {
      await api.post("/v1/system-settings/s3-delete", { s3_key: key });
      await loadBackups();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  useEffect(() => {
    if (tab === "s3") void loadBackups();
  }, [tab]);

  const field = (key: string, labelKey: string, type = "text") => (
    <div className="field" key={key}>
      <label className="label">{t(labelKey)}</label>
      <input
        className="input"
        type={type}
        value={form[key] ?? ""}
        onChange={(e) => set(key, e.target.value)}
      />
    </div>
  );

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("admin.title")}</h1>
      </div>
      <p className="muted" style={{ margin: "0 0 12px" }}>
        {t("admin.hint")}
      </p>
      <div className="tabs">
        {(["app", "smtp", "s3", "auth"] as Tab[]).map((k) => (
          <button
            key={k}
            type="button"
            className={`tab${tab === k ? " tab-active" : ""}`}
            onClick={() => setTab(k)}
          >
            {t(`admin.tab.${k}`)}
          </button>
        ))}
      </div>
      {loading && <div className="muted">{t("common.loading")}</div>}
      {error && <div className="error-text">{error}</div>}
      {msg && <div className="muted" style={{ marginBottom: 12 }}>{msg}</div>}

      {tab === "app" && (
        <form
          className="card content"
          onSubmit={(e) => {
            e.preventDefault();
            void saveKeys(KEYS.app);
          }}
        >
          <div className="form-grid">
            {field("app.name", "admin.app.name")}
            {field("app.url", "admin.app.url")}
            {field("app.language", "admin.app.language")}
          </div>
          <div className="form-actions">
            <button className="btn btn-primary" disabled={saving}>
              {t("common.save")}
            </button>
          </div>
        </form>
      )}

      {tab === "smtp" && (
        <form className="card content" onSubmit={smtpTest}>
          <div className="form-grid">
            {field("smtp.host", "admin.smtp.host")}
            {field("smtp.port", "admin.smtp.port")}
            {field("smtp.username", "admin.smtp.username")}
            {field("smtp.password", "admin.smtp.password", "password")}
            {field("smtp.from_email", "admin.smtp.from_email")}
            {field("smtp.from_name", "admin.smtp.from_name")}
          </div>
          <div className="field">
            <label className="label">{t("admin.smtp.use_tls")}</label>
            <select
              className="select"
              value={form["smtp.use_tls"] || "1"}
              onChange={(e) => set("smtp.use_tls", e.target.value)}
            >
              <option value="1">{t("common.yes")}</option>
              <option value="0">{t("common.no")}</option>
            </select>
          </div>
          <div className="field">
            <label className="label">{t("admin.smtp.test_to")}</label>
            <input
              className="input"
              value={testTo}
              onChange={(e) => setTestTo(e.target.value)}
              placeholder="you@example.com"
            />
          </div>
          <div className="form-actions">
            <button
              type="button"
              className="btn"
              disabled={saving}
              onClick={() => void saveKeys(KEYS.smtp)}
            >
              {t("common.save")}
            </button>
            <button className="btn btn-primary" disabled={busy === "smtp" || !testTo}>
              {busy === "smtp" ? t("admin.testing") : t("admin.smtp.test")}
            </button>
          </div>
        </form>
      )}

      {tab === "auth" && (
        <form
          className="card content"
          onSubmit={(e) => {
            e.preventDefault();
            void saveKeys(KEYS.auth);
          }}
        >
          <div className="field">
            <label className="label">{t("admin.auth.registration")}</label>
            <select
              className="select"
              value={form["auth.registration_enabled"] || "1"}
              onChange={(e) => set("auth.registration_enabled", e.target.value)}
            >
              <option value="1">{t("common.yes")}</option>
              <option value="0">{t("common.no")}</option>
            </select>
          </div>
          <p className="muted">{t("admin.auth.hint")}</p>
          <div className="form-actions">
            <button className="btn btn-primary" disabled={saving}>
              {t("common.save")}
            </button>
          </div>
        </form>
      )}

      {tab === "s3" && (
        <div>
          <div className="card content" style={{ marginBottom: 16 }}>
            <div className="form-grid">
              {field("s3.endpoint", "admin.s3.endpoint")}
              {field("s3.bucket", "admin.s3.bucket")}
              {field("s3.access_key", "admin.s3.access_key")}
              {field("s3.secret_key", "admin.s3.secret_key", "password")}
              {field("s3.region", "admin.s3.region")}
              {field("s3.prefix", "admin.s3.prefix")}
            </div>
            <div className="form-actions">
              <button
                type="button"
                className="btn"
                disabled={saving}
                onClick={() => void saveKeys(KEYS.s3)}
              >
                {t("common.save")}
              </button>
              <button
                type="button"
                className="btn"
                disabled={busy !== ""}
                onClick={() => void s3Test()}
              >
                {busy === "s3test" ? t("admin.testing") : t("admin.s3.test")}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={busy !== ""}
                onClick={() => void s3Backup()}
              >
                {busy === "backup" ? t("admin.s3.backing") : t("admin.s3.backup")}
              </button>
            </div>
          </div>
          <div className="card">
            {backups.length === 0 ? (
              <div className="content muted">{t("admin.s3.no_backups")}</div>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("admin.s3.file")}</th>
                    <th>{t("admin.s3.size")}</th>
                    <th>{t("admin.s3.when")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {backups.map((b) => (
                    <tr key={b.key}>
                      <td>{b.filename}</td>
                      <td>{b.size_human}</td>
                      <td>{b.last_modified}</td>
                      <td>
                        <IconButton
                          icon="delete"
                          title={t("common.delete")}
                          danger
                          onClick={() => void delBackup(b.key)}
                        />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default function AdminPage() {
  return (
    <RequireAuth>
      <AdminInner />
    </RequireAuth>
  );
}
