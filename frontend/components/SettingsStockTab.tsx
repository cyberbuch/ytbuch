"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { useI18n } from "./I18nProvider";
import { ACTIVE_COMPANY_EVENT, api, fetchAll, ListResponse } from "../lib/api";

interface Account {
  id: number;
  number: string;
  name: string;
}

interface Rule {
  id: number;
  kind: string;
  inventory_account_id: number;
  offset_account_id: number;
  clearing_account_id: number;
}

const KINDS = ["goods", "production", "material"] as const;

function label(accounts: Account[], id: number): string {
  const a = accounts.find((x) => x.id === id);
  return a ? `${a.number} ${a.name}` : "";
}

export function SettingsStockTab() {
  const { t } = useI18n();
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [rules, setRules] = useState<Rule[]>([]);
  const [kind, setKind] = useState<(typeof KINDS)[number]>("goods");
  const [inventory, setInventory] = useState(0);
  const [offset, setOffset] = useState(0);
  const [clearing, setClearing] = useState(0);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setError("");
    try {
      const [acc, pack] = await Promise.all([
        fetchAll<Account>("/v1/accounts"),
        api.get<ListResponse<Rule>>("/v1/stock/rules"),
      ]);
      setAccounts(acc);
      setRules(pack.items ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

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
      await api.post("/v1/stock/rules", {
        kind,
        inventory_account_id: inventory,
        offset_account_id: offset,
        clearing_account_id: kind === "production" ? clearing : 0,
      });
      setInventory(0);
      setOffset(0);
      setClearing(0);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  async function remove(id: number) {
    setError("");
    try {
      await api.del(`/v1/stock/rules/${id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <div className="card card-pad" style={{ maxWidth: 880 }}>
      <h2 style={{ marginTop: 0 }}>{t("stock.rules.title")}</h2>
      <p className="muted">{t("stock.rules.hint")}</p>
      {error && <div className="error-text">{error}</div>}
      <form onSubmit={add}>
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("stock.rules.kind")}</label>
            <select className="select" value={kind} onChange={(e) => setKind(e.target.value as (typeof KINDS)[number])}>
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {t(`stock.rules.kind.${k}`)}
                </option>
              ))}
            </select>
          </div>
          <AccountField label={t("stock.rules.inventory")} accounts={accounts} value={inventory} onChange={setInventory} pick={t("stock.rules.pick")} />
          <AccountField label={t("stock.rules.offset")} accounts={accounts} value={offset} onChange={setOffset} pick={t("stock.rules.pick")} />
          {kind === "production" && (
            <AccountField label={t("stock.rules.clearing")} accounts={accounts} value={clearing} onChange={setClearing} pick={t("stock.rules.pick")} />
          )}
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" disabled={saving || inventory <= 0 || offset <= 0 || (kind === "production" && clearing <= 0)}>
            {t("stock.rules.add")}
          </button>
        </div>
      </form>
      {rules.length === 0 ? (
        <div className="muted">{t("stock.rules.empty")}</div>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>{t("stock.rules.kind")}</th>
              <th>{t("stock.rules.inventory")}</th>
              <th>{t("stock.rules.offset")}</th>
              <th>{t("stock.rules.clearing")}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {rules.map((r) => (
              <tr key={r.id}>
                <td>{t(`stock.rules.kind.${r.kind}`)}</td>
                <td>{label(accounts, r.inventory_account_id)}</td>
                <td>{label(accounts, r.offset_account_id)}</td>
                <td>{r.clearing_account_id > 0 ? label(accounts, r.clearing_account_id) : ""}</td>
                <td>
                  <button type="button" className="btn" onClick={() => remove(r.id)}>
                    {t("common.delete")}
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

function AccountField({
  label: text,
  accounts,
  value,
  onChange,
  pick,
}: {
  label: string;
  accounts: Account[];
  value: number;
  onChange: (id: number) => void;
  pick: string;
}) {
  return (
    <div className="field">
      <label className="label">{text}</label>
      <select className="select" value={String(value)} onChange={(e) => onChange(Number(e.target.value) || 0)}>
        <option value="0">{pick}</option>
        {accounts.map((a) => (
          <option key={a.id} value={a.id}>
            {a.number} {a.name}
          </option>
        ))}
      </select>
    </div>
  );
}
