"use client";

// SAF-T кореспонденции на сметки — Дт/Кт шаблони към тип движение
// (запаси / ДМА / пари). Стандартните настройки са по българския сметкоплан.

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { ACTIVE_COMPANY_EVENT, api } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { IconButton } from "../../components/IconButton";

type Kind = "stock" | "asset" | "cash";

interface MovementType {
  code: string;
  name: string;
  name_en: string;
}

interface Mapping {
  id: number;
  kind: Kind;
  type_code: string;
  type_name: string;
  debit_account: string;
  credit_account: string;
  debit_analytical: string;
  credit_analytical: string;
  description: string;
  is_active: number;
}

interface MappingList {
  kind: Kind;
  items: Mapping[];
  count: number;
  types: MovementType[];
}

const emptyForm = {
  type_code: "",
  debit_account: "",
  credit_account: "",
  debit_analytical: "",
  credit_analytical: "",
  description: "",
};

function codeNum(code: string): number {
  const n = Number(code);
  return Number.isFinite(n) ? n : 9999;
}

function SmapInner() {
  const { t } = useI18n();
  const [kind, setKind] = useState<Kind>("stock");
  const [items, setItems] = useState<Mapping[]>([]);
  const [types, setTypes] = useState<MovementType[]>([]);
  const [q, setQ] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [ok, setOk] = useState("");
  const [modal, setModal] = useState(false);
  const [editing, setEditing] = useState<Mapping | null>(null);
  const [form, setForm] = useState(emptyForm);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<MappingList>(
        `/v1/saft/movement-mappings?kind=${kind}`
      );
      setItems(data.items ?? []);
      setTypes(data.types ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setItems([]);
      setTypes([]);
    } finally {
      setLoading(false);
    }
  }, [kind]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const onChange = () => {
      void load();
    };
    window.addEventListener(ACTIVE_COMPANY_EVENT, onChange);
    return () => window.removeEventListener(ACTIVE_COMPANY_EVENT, onChange);
  }, [load]);

  useEffect(() => {
    if (!ok) return;
    const tmr = window.setTimeout(() => setOk(""), 2500);
    return () => window.clearTimeout(tmr);
  }, [ok]);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return items;
    return items.filter((m) => {
      const blob = [
        m.type_code,
        m.type_name,
        m.debit_account,
        m.credit_account,
        m.debit_analytical,
        m.credit_analytical,
        m.description,
      ]
        .join(" ")
        .toLowerCase();
      return blob.includes(s);
    });
  }, [items, q]);

  const groups = useMemo(() => {
    const map = new Map<string, Mapping[]>();
    for (const m of filtered) {
      const list = map.get(m.type_code) ?? [];
      list.push(m);
      map.set(m.type_code, list);
    }
    const codes = Array.from(map.keys()).sort((a, b) => codeNum(a) - codeNum(b));
    return codes.map((code) => {
      const list = map.get(code) ?? [];
      const fromType = types.find((x) => x.code === code);
      const name = fromType?.name || list[0]?.type_name || code;
      return { code, name, items: list };
    });
  }, [filtered, types]);

  const typeLabel =
    kind === "asset" ? t("smap.type.asset") : kind === "cash" ? t("smap.type.cash") : t("smap.type.stock");

  const openCreate = () => {
    setEditing(null);
    setForm({ ...emptyForm, type_code: types[0]?.code ?? "" });
    setModal(true);
  };

  const openEdit = (m: Mapping) => {
    setEditing(m);
    setForm({
      type_code: m.type_code,
      debit_account: m.debit_account,
      credit_account: m.credit_account,
      debit_analytical: m.debit_analytical,
      credit_analytical: m.credit_analytical,
      description: m.description,
    });
    setModal(true);
  };

  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (!form.type_code) {
      setError(t("smap.need_type"));
      return;
    }
    if (!form.debit_account.trim() || !form.credit_account.trim()) {
      setError(t("smap.need_accounts"));
      return;
    }
    setSaving(true);
    setError("");
    try {
      const payload = {
        kind,
        type_code: form.type_code,
        debit_account: form.debit_account.trim(),
        credit_account: form.credit_account.trim(),
        debit_analytical: form.debit_analytical.trim(),
        credit_analytical: form.credit_analytical.trim(),
        description: form.description.trim(),
      };
      if (editing) {
        await api.patch(`/v1/saft/movement-mappings/${editing.id}`, payload);
      } else {
        await api.post("/v1/saft/movement-mappings", payload);
      }
      setModal(false);
      setOk(t("smap.saved"));
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (id: number) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    setError("");
    try {
      await api.del(`/v1/saft/movement-mappings/${id}`);
      setOk(t("smap.deleted"));
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const loadDefaults = async () => {
    if (items.length > 0) return;
    setSaving(true);
    setError("");
    try {
      await api.post("/v1/saft/movement-mappings/defaults", { kind });
      setOk(t("smap.defaults_ok"));
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const acc = (code: string, analytical: string) => (
    <span>
      <span style={{ fontFamily: "ui-monospace, monospace" }}>{code}</span>
      {analytical ? (
        <span className="muted" style={{ marginLeft: 6, fontSize: 12 }}>
          ({analytical})
        </span>
      ) : null}
    </span>
  );

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("smap.title")}</h1>
        <div className="icon-actions">
          <button className="btn btn-primary" onClick={openCreate}>
            {t("smap.add")}
          </button>
          <button
            className="btn"
            onClick={() => void loadDefaults()}
            disabled={saving || items.length > 0}
          >
            {t("smap.defaults")}
          </button>
        </div>
      </div>
      <p className="muted" style={{ margin: "0 0 12px" }}>
        {t("smap.hint")}{" "}
        <Link href="/saft">{t("smap.link.export")}</Link>
      </p>
      <div className="tabs">
        {(["stock", "asset", "cash"] as Kind[]).map((k) => (
          <button
            key={k}
            type="button"
            className={`tab${kind === k ? " tab-active" : ""}`}
            onClick={() => setKind(k)}
          >
            {t(`smap.tab.${k}`)}
          </button>
        ))}
      </div>
      <div className="toolbar" style={{ marginBottom: 12 }}>
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t("common.search")}
          style={{ maxWidth: 360 }}
        />
      </div>
      {error && <div className="error-text">{error}</div>}
      {ok && (
        <div className="muted" style={{ color: "var(--success)", marginBottom: 12 }}>
          {ok}
        </div>
      )}
      {loading ? (
        <div className="card content muted">{t("common.loading")}</div>
      ) : groups.length === 0 ? (
        <div className="card content">
          <p className="muted">{t("smap.empty")}</p>
          <p className="muted">{t("smap.empty_hint")}</p>
        </div>
      ) : (
        groups.map((g) => (
          <div className="card" key={g.code} style={{ marginBottom: 16 }}>
            <div
              style={{
                background: "var(--primary)",
                color: "#fff",
                padding: "10px 16px",
                fontWeight: 600,
                borderRadius: "var(--radius) var(--radius) 0 0",
              }}
            >
              {g.code} — {g.name}
            </div>
            <div className="table-wrap">
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("smap.debit")}</th>
                    <th>{t("smap.credit")}</th>
                    <th>{t("smap.description")}</th>
                    <th>{t("common.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {g.items.map((m) => (
                    <tr key={m.id}>
                      <td>{acc(m.debit_account, m.debit_analytical)}</td>
                      <td>{acc(m.credit_account, m.credit_analytical)}</td>
                      <td className="muted">{m.description}</td>
                      <td>
                        <div className="icon-actions">
                          <IconButton
                            icon="edit"
                            title={t("common.edit")}
                            onClick={() => openEdit(m)}
                          />
                          <IconButton
                            icon="delete"
                            title={t("common.delete")}
                            danger
                            onClick={() => void remove(m.id)}
                          />
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        ))
      )}

      {modal && (
        <div className="modal-backdrop" onClick={() => setModal(false)}>
          <div className="card modal" onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">
              {editing ? t("common.edit") : t("smap.new")}
            </h2>
            <form onSubmit={(e) => void save(e)}>
              <div className="field">
                <label className="label">{typeLabel}</label>
                <select
                  className="select"
                  value={form.type_code}
                  onChange={(e) => setForm({ ...form, type_code: e.target.value })}
                >
                  <option value="">{t("smap.pick_type")}</option>
                  {types.map((tp) => (
                    <option key={tp.code} value={tp.code}>
                      {tp.code} — {tp.name}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("smap.debit")}</label>
                  <input
                    className="input"
                    value={form.debit_account}
                    onChange={(e) => setForm({ ...form, debit_account: e.target.value })}
                    placeholder="302, 302*, 30*"
                  />
                </div>
                <div className="field">
                  <label className="label">{t("smap.credit")}</label>
                  <input
                    className="input"
                    value={form.credit_account}
                    onChange={(e) => setForm({ ...form, credit_account: e.target.value })}
                    placeholder="401, 401*, 40*"
                  />
                </div>
                <div className="field">
                  <label className="label">{t("smap.debit_an")}</label>
                  <input
                    className="input"
                    value={form.debit_analytical}
                    onChange={(e) => setForm({ ...form, debit_analytical: e.target.value })}
                    placeholder="3021"
                  />
                </div>
                <div className="field">
                  <label className="label">{t("smap.credit_an")}</label>
                  <input
                    className="input"
                    value={form.credit_analytical}
                    onChange={(e) => setForm({ ...form, credit_analytical: e.target.value })}
                    placeholder="4011"
                  />
                </div>
              </div>
              <div className="field">
                <label className="label">{t("smap.description")}</label>
                <input
                  className="input"
                  value={form.description}
                  onChange={(e) => setForm({ ...form, description: e.target.value })}
                />
              </div>
              <div className="form-actions">
                <button type="button" className="btn" onClick={() => setModal(false)}>
                  {t("common.cancel")}
                </button>
                <button type="submit" className="btn btn-primary" disabled={saving}>
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

export default function SaftMovementMappingsPage() {
  return (
    <RequireAuth>
      <SmapInner />
    </RequireAuth>
  );
}
