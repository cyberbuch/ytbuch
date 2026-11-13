"use client";

// Правила за банкови транзакции: ключови думи → сметка. Ключовите думи се
// търсят в описанието и името на контрагента (кирилица и латиница), без AI;
// при няколко съвпадения печели по-високият приоритет.

import { useCallback, useEffect, useState } from "react";
import { api } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";

export interface RuleAccount {
  id: number;
  number: string;
  name: string;
}

interface Rule {
  id: number;
  name: string;
  keywords: string;
  direction: string;
  priority: number;
  is_active: boolean;
  account_id: number;
  counterpart_id: number;
}

interface AllocateResult {
  matched: number;
  unmatched: number;
  errors: number;
  counterpart_hits: number;
  details: { id: number; status: string; rule?: string; text?: string; error?: string; account?: string; counterpart?: string; amount?: string }[];
}

const emptyDraft = () => ({
  id: 0,
  name: "",
  keywords: "",
  direction: "any",
  priority: 50,
  is_active: true,
  account_id: 0,
});

export function BankRulesPanel({
  accounts,
  onChanged,
}: {
  accounts: RuleAccount[];
  onChanged: () => void;
}) {
  const { t } = useI18n();
  const [rules, setRules] = useState<Rule[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [draft, setDraft] = useState(emptyDraft());
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState("");
  const [result, setResult] = useState<AllocateResult | null>(null);
  const [preview, setPreview] = useState<AllocateResult | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<{ items: Rule[] }>("/v1/bank-matching-rules");
      setRules(data.items ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const accountLabel = (id: number) => {
    const a = accounts.find((x) => x.id === id);
    return a ? `${a.number} ${a.name}` : `#${id}`;
  };

  const save = async () => {
    if (!draft.name.trim() || !draft.keywords.trim() || draft.account_id <= 0) {
      setError(t("brules.required"));
      return;
    }
    setBusy("save");
    setError("");
    try {
      const body = {
        name: draft.name,
        keywords: draft.keywords,
        direction: draft.direction,
        priority: draft.priority,
        is_active: draft.is_active,
        account_id: draft.account_id,
        counterpart_id: 0,
      };
      if (editing) {
        await api.patch(`/v1/bank-matching-rules/${draft.id}`, body);
      } else {
        await api.post("/v1/bank-matching-rules", body);
      }
      setDraft(emptyDraft());
      setEditing(false);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const remove = async (id: number) => {
    setBusy(`del${id}`);
    try {
      await api.del(`/v1/bank-matching-rules/${id}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const seed = async () => {
    setBusy("seed");
    setError("");
    try {
      const r = await api.post<{ created: number }>("/v1/bank-matching-rules/defaults", {});
      setNotice(`${t("brules.seeded")}: ${r.created}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const loadPreview = async () => {
    setBusy("preview");
    setError("");
    setNotice("");
    setResult(null);
    try {
      const r = await api.post<AllocateResult>("/v1/bank-transactions/auto-allocate/preview", {});
      setPreview(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const allocate = async () => {
    setBusy("alloc");
    setError("");
    setNotice("");
    try {
      const r = await api.post<AllocateResult>("/v1/bank-transactions/auto-allocate", {});
      setResult(r);
      setPreview(null);
      setNotice(
        `${t("brules.matched")}: ${r.matched}, ${t("brules.unmatched")}: ${r.unmatched}, ${t("brules.errors")}: ${r.errors}`
      );
      onChanged();
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  return (
    <div className="card card-pad" style={{ marginTop: 16 }}>
      <h3 className="section-title" style={{ marginTop: 0 }}>
        {t("brules.title")}
      </h3>
      <p className="muted" style={{ fontSize: 13 }}>{t("brules.hint")}</p>
      {error && <div className="error-text">{error}</div>}
      {notice && <div className="flash-ok">{notice}</div>}

      <div className="btn-row" style={{ marginBottom: 12 }}>
        <button className="btn btn-sm" type="button" disabled={busy !== ""} onClick={() => void seed()}>
          {t("brules.seed")}
        </button>
        <button className="btn btn-sm" type="button" disabled={busy !== ""} onClick={() => void loadPreview()}>
          {t("brules.preview")}
        </button>
      </div>

      {preview && (
        <div className="card card-pad" style={{ marginBottom: 12 }}>
          <p className="muted" style={{ fontSize: 13 }}>
            {t("brules.preview_hint")}: {preview.matched} / {preview.unmatched} {t("brules.unmatched")}
          </p>
          <table className="table">
            <thead>
              <tr>
                <th>#</th>
                <th>{t("brules.name")}</th>
                <th>{t("brules.account")}</th>
                <th>{t("bank_tx.counterpart")}</th>
                <th>{t("journal.amount")}</th>
              </tr>
            </thead>
            <tbody>
              {preview.details.filter((d) => d.status === "matched").map((d) => (
                <tr key={d.id}>
                  <td>{d.id}</td>
                  <td>{d.rule}</td>
                  <td>{d.account}</td>
                  <td>{d.counterpart}</td>
                  <td>{d.amount}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="form-actions">
            <button className="btn btn-primary" type="button" disabled={busy !== "" || preview.matched === 0} onClick={() => void allocate()}>
              {t("brules.apply")}
            </button>
            <button className="btn" type="button" onClick={() => setPreview(null)}>
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}

      {result && result.details.length > 0 && (
        <ul className="muted" style={{ fontSize: 12, marginBottom: 12 }}>
          {result.details.slice(0, 12).map((d, i) => (
            <li key={i}>
              #{d.id} — {d.status}
              {d.rule ? ` · ${d.rule}` : ""}
              {d.text ? ` · ${d.text}` : ""}
              {d.error ? ` · ${d.error}` : ""}
            </li>
          ))}
        </ul>
      )}

      {loading ? (
        <p className="muted">{t("common.loading")}</p>
      ) : rules.length === 0 ? (
        <p className="muted">{t("brules.empty")}</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>{t("brules.name")}</th>
              <th>{t("brules.keywords")}</th>
              <th>{t("brules.direction")}</th>
              <th>{t("brules.priority")}</th>
              <th>{t("brules.account")}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {rules.map((r) => (
              <tr key={r.id}>
                <td>
                  {r.name}
                  {!r.is_active && <span className="muted"> ({t("brules.inactive")})</span>}
                </td>
                <td style={{ fontSize: 12 }}>{r.keywords}</td>
                <td>{r.direction}</td>
                <td>{r.priority}</td>
                <td>{accountLabel(r.account_id)}</td>
                <td>
                  <div className="btn-row">
                    <button
                      className="btn btn-sm"
                      type="button"
                      onClick={() => {
                        setEditing(true);
                        setDraft({
                          id: r.id,
                          name: r.name,
                          keywords: r.keywords,
                          direction: r.direction,
                          priority: r.priority,
                          is_active: r.is_active,
                          account_id: r.account_id,
                        });
                      }}
                    >
                      {t("common.edit")}
                    </button>
                    <button
                      className="btn btn-sm btn-danger"
                      type="button"
                      disabled={busy !== ""}
                      onClick={() => void remove(r.id)}
                    >
                      {t("common.delete")}
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <div className="form-grid" style={{ marginTop: 12 }}>
        <div className="field">
          <label className="label">{t("brules.name")}</label>
          <input className="input" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
        </div>
        <div className="field">
          <label className="label">{t("brules.direction")}</label>
          <select className="select" value={draft.direction} onChange={(e) => setDraft({ ...draft, direction: e.target.value })}>
            <option value="any">{t("brules.dir_any")}</option>
            <option value="debit">{t("brules.dir_debit")}</option>
            <option value="credit">{t("brules.dir_credit")}</option>
          </select>
        </div>
        <div className="field">
          <label className="label">{t("brules.priority")}</label>
          <input
            className="input"
            type="number"
            value={draft.priority}
            onChange={(e) => setDraft({ ...draft, priority: Number(e.target.value) || 0 })}
          />
        </div>
        <div className="field">
          <label className="label">{t("brules.account")}</label>
          <select
            className="select"
            value={draft.account_id}
            onChange={(e) => setDraft({ ...draft, account_id: Number(e.target.value) || 0 })}
          >
            <option value={0}>{t("brules.pick_account")}</option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.number} {a.name}
              </option>
            ))}
          </select>
        </div>
        <div className="field" style={{ gridColumn: "1 / -1" }}>
          <label className="label">{t("brules.keywords")}</label>
          <input
            className="input"
            value={draft.keywords}
            placeholder="POS,ПОС,теглене,ATM,withdrawal"
            onChange={(e) => setDraft({ ...draft, keywords: e.target.value })}
          />
        </div>
        <div className="field">
          <label className="label">
            <input type="checkbox" checked={draft.is_active} onChange={(e) => setDraft({ ...draft, is_active: e.target.checked })} />{" "}
            {t("brules.active")}
          </label>
        </div>
      </div>
      <div className="form-actions">
        <button className="btn btn-primary" type="button" disabled={busy !== ""} onClick={() => void save()}>
          {editing ? t("common.save") : t("brules.add")}
        </button>
        {editing && (
          <button className="btn" type="button" onClick={() => { setEditing(false); setDraft(emptyDraft()); }}>
            {t("common.cancel")}
          </button>
        )}
      </div>
    </div>
  );
}
