"use client";

// Таб «Приключване»: карта closing_map в настройките на фирмата —
// коя сметка/подсметка (префикс на номера) към коя сметка се приключва
// в края на месеца (напр. 60 -> 611, 602 -> 612).

import { FormEvent, useCallback, useEffect, useState } from "react";
import { AccountPicker } from "./AccountPicker";
import { useI18n } from "./I18nProvider";
import {
  ACTIVE_COMPANY_EVENT,
  api,
  fetchAll,
  getActiveCompany,
} from "../lib/api";

interface Account {
  id: number;
  number: string;
  name: string;
}

interface RuleRow {
  source: string;
  target: string;
}

function parseRules(raw: unknown): RuleRow[] {
  if (typeof raw !== "string" || !raw) return [];
  try {
    const o = JSON.parse(raw) as Record<string, unknown>;
    const m = o.closing_map;
    if (Array.isArray(m)) {
      return m
        .map((x) => {
          const r = x as Record<string, unknown>;
          return {
            source: String(r.source ?? "").trim(),
            target: String(r.target ?? "").trim(),
          };
        })
        .filter((r) => r.source && r.target);
    }
    if (m && typeof m === "object") {
      return Object.entries(m as Record<string, unknown>)
        .filter(([, v]) => typeof v === "string" && v)
        .map(([source, target]) => ({ source: source.trim(), target: String(target).trim() }));
    }
  } catch {
    // невалиден JSON — празна карта
  }
  return [];
}

function accountLabel(accounts: Account[], number: string): string {
  const a = accounts.find((x) => x.number === number);
  return a ? `${a.number} ${a.name}` : number;
}

export function SettingsClosingTab() {
  const { t } = useI18n();
  const [companyId, setCompanyId] = useState(0);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [rows, setRows] = useState<RuleRow[]>([]);
  const [rawSettings, setRawSettings] = useState("{}");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [msg, setMsg] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const ac = await getActiveCompany();
      const id = ac && typeof ac.id === "number" ? ac.id : 0;
      setCompanyId(id);
      if (id <= 0) {
        setLoading(false);
        return;
      }
      const [acc, company] = await Promise.all([
        fetchAll<Account>("/v1/accounts"),
        api.get<{ settings?: string }>(`/v1/companies/${id}`),
      ]);
      setAccounts(acc);
      const raw = typeof company.settings === "string" ? company.settings : "{}";
      setRawSettings(raw);
      setRows(parseRules(raw));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
    const onChange = () => load();
    window.addEventListener(ACTIVE_COMPANY_EVENT, onChange);
    return () => window.removeEventListener(ACTIVE_COMPANY_EVENT, onChange);
  }, [load]);

  const setRow = (i: number, patch: Partial<RuleRow>) =>
    setRows((prev) => prev.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));

  const removeRow = (i: number) => setRows((prev) => prev.filter((_, idx) => idx !== i));

  const addRow = () => setRows((prev) => [...prev, { source: "", target: "" }]);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (companyId <= 0) return;
    setSaving(true);
    setError("");
    setMsg("");
    try {
      let base: Record<string, unknown> = {};
      try {
        const parsed = JSON.parse(rawSettings || "{}") as unknown;
        if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
          base = parsed as Record<string, unknown>;
        }
      } catch {
        base = {};
      }
      const valid = rows.filter((r) => r.source.trim() && r.target.trim());
      if (valid.length > 0) {
        base.closing_map = valid.map((r) => ({
          source: r.source.trim(),
          target: r.target.trim(),
        }));
      } else {
        delete base.closing_map;
      }
      await api.patch(`/v1/companies/${companyId}`, {
        settings: JSON.stringify(base),
      });
      setRawSettings(JSON.stringify(base));
      setRows(valid);
      setMsg(t("closing.saved"));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <div className="muted">{t("common.loading")}</div>;
  if (companyId <= 0) return <div className="muted">{t("settings.no_company")}</div>;

  return (
    <div className="card card-pad" style={{ maxWidth: 880 }}>
      <h2 style={{ marginTop: 0 }}>{t("closing.title")}</h2>
      <p className="muted">{t("closing.hint")}</p>
      {error && <div className="error-text">{error}</div>}
      {msg && <div className="flash-ok">{msg}</div>}
      <form onSubmit={save}>
        {rows.length === 0 ? (
          <div className="muted">{t("closing.empty")}</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("closing.source")}</th>
                <th>{t("closing.target")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={i}>
                  <td>
                    <AccountPicker
                      value={r.source}
                      accounts={accounts}
                      onChange={(v) => setRow(i, { source: v })}
                      placeholder={t("closing.source_ph")}
                    />
                  </td>
                  <td>
                    <AccountPicker
                      value={r.target}
                      accounts={accounts}
                      onChange={(v) => setRow(i, { target: v })}
                      placeholder={t("closing.search_ph")}
                    />
                  </td>
                  <td>
                    <button type="button" className="btn" onClick={() => removeRow(i)}>
                      {t("common.delete")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <div className="form-actions">
          <button type="button" className="btn" onClick={addRow}>
            {t("closing.add")}
          </button>
          <button type="submit" className="btn btn-primary" disabled={saving}>
            {t("common.save")}
          </button>
        </div>
        {rows.length > 0 && (
          <p className="muted" style={{ fontSize: 12 }}>
            {t("closing.preview_label")}:{" "}
            {rows
              .map((r) => `${r.source} → ${accountLabel(accounts, r.target)}`)
              .join("; ")}
          </p>
        )}
      </form>
    </div>
  );
}
