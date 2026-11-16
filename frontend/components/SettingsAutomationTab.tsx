"use client";

// Таб «Автоматизация»: default сметки на фирмата (settings.default_accounts).
// Постът и плащанията ги ползват за клиент/доставчик, ДДС, каса, банка и
// буферни сметки за карта; приход/разход/запаси остават от артикула.

import { FormEvent, useCallback, useEffect, useState } from "react";
import { AccountPicker } from "./AccountPicker";
import { useI18n } from "./I18nProvider";
import { ACTIVE_COMPANY_EVENT, api, fetchAll, getActiveCompany } from "../lib/api";

interface Account {
  id: number;
  number: string;
  name: string;
}

const ROLES = [
  "customers",
  "suppliers",
  "vat_purchase",
  "vat_sale",
  "cash",
  "bank",
  "card_purchase",
  "card_sale",
  "revenues",
  "expenses",
] as const;

const FALLBACK: Record<string, string> = {
  customers: "411",
  suppliers: "401",
  vat_purchase: "4531",
  vat_sale: "4532",
  cash: "501",
  bank: "503",
  revenues: "702",
  expenses: "602",
};

function parseDefaults(raw: unknown): Record<string, string> {
  if (typeof raw !== "string" || !raw) return {};
  try {
    const o = JSON.parse(raw) as Record<string, unknown>;
    const m = o.default_accounts;
    if (m && typeof m === "object" && !Array.isArray(m)) {
      const out: Record<string, string> = {};
      for (const [k, v] of Object.entries(m as Record<string, unknown>)) {
        if (typeof v === "string" && v.trim()) out[k] = v.trim();
      }
      return out;
    }
  } catch {
    // невалиден JSON — празни настройки
  }
  return {};
}

export function SettingsAutomationTab() {
  const { t } = useI18n();
  const [companyId, setCompanyId] = useState(0);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [values, setValues] = useState<Record<string, string>>({});
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
      setValues(parseDefaults(raw));
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
      const clean: Record<string, string> = {};
      for (const [k, v] of Object.entries(values)) {
        if (v.trim()) clean[k] = v.trim();
      }
      if (Object.keys(clean).length > 0) base.default_accounts = clean;
      else delete base.default_accounts;
      await api.patch(`/v1/companies/${companyId}`, { settings: JSON.stringify(base) });
      setRawSettings(JSON.stringify(base));
      setValues(clean);
      setMsg(t("auto.saved"));
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
      <h2 style={{ marginTop: 0 }}>{t("auto.title")}</h2>
      <p className="muted">{t("auto.hint")}</p>
      {error && <div className="error-text">{error}</div>}
      {msg && <div className="flash-ok">{msg}</div>}
      <form onSubmit={save}>
        <div className="form-grid">
          {ROLES.map((role) => (
            <div className="field" key={role}>
              <label className="label">
                {t(`auto.role.${role}`)}
                {FALLBACK[role] ? <span className="muted"> ({FALLBACK[role]})</span> : null}
              </label>
              <AccountPicker
                value={values[role] ?? ""}
                accounts={accounts}
                onChange={(v) => setValues((p) => ({ ...p, [role]: v }))}
                placeholder={FALLBACK[role] ?? ""}
              />
            </div>
          ))}
        </div>
        <div className="form-actions">
          <button type="submit" className="btn btn-primary" disabled={saving}>
            {t("common.save")}
          </button>
        </div>
      </form>
    </div>
  );
}
