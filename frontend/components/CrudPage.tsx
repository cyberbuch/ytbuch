"use client";

// CrudPage — генеричен списък + създаване/редакция/изтриване по конфигурация.
// Използва се за фирми, сметкоплан, контрагенти, стоки. Фактурите и дневникът
// са специализирани (редове/кореспонденции) и са отделни страници.

import { useCallback, useEffect, useMemo, useState, FormEvent } from "react";
import { api, ListResponse } from "../lib/api";
import { useI18n } from "./I18nProvider";
import { IconButton } from "./IconButton";

export interface SelectOption {
  value: string;
  labelKey: string;
}

export interface FieldDef {
  name: string;
  labelKey: string;
  type: "text" | "number" | "select" | "checkbox" | "textarea";
  required?: boolean;
  options?: SelectOption[];
  default?: string;
}

export interface RowAction {
  labelKey: string;
  onClick: (rec: Record<string, unknown>) => void;
}

// VIES извличане: бутон в модала, който пита backend-а за ЕС ДДС номер и
// попълва полетата на формата (име, адрес, суров VIES адрес, държава, ЕИК за BG).
export interface ViesConfig {
  endpoint: string; // напр. "/v1/counterparts/vies-lookup"
  vatField: string; // полето с ДДС номера (източник на заявката)
  map: Record<string, string>; // ключ от отговора → поле във формата
  eikField?: string; // попълва се само за BG номера (vat без кода)
  labelKey: string;
  loadingKey: string;
  invalidKey: string;
  filledKey: string;
}

export interface ViesLookupResponse {
  valid: boolean;
  name: string;
  address: string;
  vies_address: string;
  street_name?: string;
  building_number?: string;
  address_building?: string;
  additional_address_detail?: string;
  address_type?: string;
  city?: string;
  post_code?: string;
  region?: string;
  user_error: string;
  request_date: string;
  vat_number: string;
  country_code: string;
  country?: string;
  parse_ok?: boolean;
  parse_note?: string;
}

export interface AddressSplitConfig {
  labelKey: string;
  loadingKey: string;
  emptyKey: string;
}

export interface CrudConfig {
  endpoint: string;
  titleKey: string;
  fields: FieldDef[];
  columns: string[];
  // Етикет за колона, която не е поле във формата (ключ от i18n).
  columnLabelKey?: Record<string, string>;
  // Текст в клетката. null = стандартното поле.
  columnText?: (column: string, rec: Record<string, unknown>) => string | null;
  rowAction?: RowAction;
  vies?: ViesConfig;
  addressSplit?: AddressSplitConfig;
}

type Record_ = Record<string, unknown>;

function cellText(rec: Record_, field: FieldDef, t: (k: string) => string): string {
  const v = rec[field.name];
  if (v === null || v === undefined) return "";
  if (field.type === "checkbox") return Number(v) === 1 ? "✓" : "";
  if (field.type === "select") {
    const opt = field.options?.find((o) => o.value === String(v));
    return opt ? t(opt.labelKey) : String(v);
  }
  return String(v);
}

export function CrudPage({ config }: { config: CrudConfig }) {
  const { t } = useI18n();
  const [rows, setRows] = useState<Record_[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Record_ | null>(null);
  const [form, setForm] = useState<Record<string, string | boolean>>({});
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");
  const [viesLoading, setViesLoading] = useState(false);
  const [splitLoading, setSplitLoading] = useState(false);
  const [viesError, setViesError] = useState("");
  const [viesFilled, setViesFilled] = useState(false);
  const [viesNote, setViesNote] = useState("");
  const [q, setQ] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<ListResponse<Record_>>(config.endpoint);
      setRows(data.items ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [config.endpoint]);

  useEffect(() => {
    // Зареждане при монтиране (данните зависят от автентикацията, четат се от
    // клиента; сетСтейт е асинхронен след await, не синхронен каскаден рендер).
    load();
  }, [load]);

  const openCreate = () => {
    const initial: Record<string, string | boolean> = {};
    for (const f of config.fields) {
      if (f.type === "checkbox") initial[f.name] = false;
      else initial[f.name] = f.default ?? "";
    }
    setForm(initial);
    setEditing(null);
    setFormError("");
    setViesError("");
    setViesFilled(false);
    setViesNote("");
    setModalOpen(true);
  };

  const openEdit = (rec: Record_) => {
    const initial: Record<string, string | boolean> = {};
    for (const f of config.fields) {
      const v = rec[f.name];
      if (f.type === "checkbox") initial[f.name] = Number(v) === 1;
      else initial[f.name] = v === null || v === undefined ? "" : String(v);
    }
    setForm(initial);
    setEditing(rec);
    setFormError("");
    setViesError("");
    setViesFilled(false);
    setViesNote("");
    setModalOpen(true);
  };

  // VIES: чете ДДС номера от формата, пита backend-а и попълва полетата.
  const handleViesFetch = async () => {
    const v = config.vies;
    if (!v) return;
    const vat = String(form[v.vatField] ?? "").trim();
    setViesFilled(false);
    setViesNote("");
    if (!vat) {
      setViesError(t(v.invalidKey));
      return;
    }
    setViesLoading(true);
    setViesError("");
    try {
      const data = await api.get<ViesLookupResponse>(
        `${v.endpoint}?vat=${encodeURIComponent(vat)}`
      );
      if (!data.valid) {
        setViesError(t(v.invalidKey));
        return;
      }
      setForm((prev) => {
        const next = { ...prev, [v.vatField]: data.vat_number };
        for (const [respKey, field] of Object.entries(v.map)) {
          const val = (data as unknown as Record_)[respKey];
          if (typeof val === "string" && val !== "" && val !== "---") {
            next[field] = val;
          }
        }
        if (v.eikField && data.country_code === "BG") {
          next[v.eikField] = data.vat_number.substring(2);
        }
        return next;
      });
      setViesFilled(true);
      setViesNote(data.parse_note ?? "");
    } catch (err) {
      setViesError(err instanceof Error ? err.message : String(err));
    } finally {
      setViesLoading(false);
    }
  };

  const handleSplitAddress = async () => {
    const cfg = config.addressSplit;
    if (!cfg) return;
    const address = String(form.address ?? "").trim();
    setViesFilled(false);
    setViesNote("");
    if (!address) {
      setViesError(t(cfg.emptyKey));
      return;
    }
    setSplitLoading(true);
    setViesError("");
    try {
      const data = await api.post<ViesLookupResponse>("/v1/addresses/parse", {
        address,
        country: String(form.country ?? "BG"),
        city: String(form.city ?? ""),
        post_code: String(form.post_code ?? ""),
      });
      if (!data.parse_ok) {
        setViesError(data.parse_note || t(cfg.emptyKey));
        return;
      }
      setForm((prev) => {
        const next = { ...prev };
        const fields: Array<[string, string | undefined]> = [
          ["street_name", data.street_name],
          ["building_number", data.building_number],
          ["address_building", data.address_building],
          ["additional_address_detail", data.additional_address_detail],
          ["address_type", data.address_type],
          ["region", data.region],
          ["city", data.city],
          ["post_code", data.post_code],
          ["country", data.country || data.country_code],
        ];
        for (const [field, val] of fields) {
          if (val) next[field] = val;
        }
        return next;
      });
      setViesNote(data.parse_note ?? "");
    } catch (err) {
      setViesError(err instanceof Error ? err.message : String(err));
    } finally {
      setSplitLoading(false);
    }
  };

  const buildPayload = (): Record_ => {
    const payload: Record_ = {};
    for (const f of config.fields) {
      const v = form[f.name];
      if (f.type === "checkbox") {
        payload[f.name] = Boolean(v);
      } else if (f.type === "number") {
        const raw = String(v ?? "").trim();
        if (raw === "") {
          if (f.default !== undefined && f.default !== "") {
            payload[f.name] = Number(f.default);
          }
        } else {
          payload[f.name] = Number(raw);
        }
      } else {
        payload[f.name] = v ?? "";
      }
    }
    return payload;
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError("");
    try {
      const payload = buildPayload();
      if (editing) {
        await api.patch(`${config.endpoint}/${editing.id}`, payload);
      } else {
        await api.post(config.endpoint, payload);
      }
      setModalOpen(false);
      await load();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (rec: Record_) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`${config.endpoint}/${rec.id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const setField = (name: string, value: string | boolean) =>
    setForm((prev) => ({ ...prev, [name]: value }));

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((rec) =>
      Object.values(rec)
        .map((v) => (v == null ? "" : String(v)))
        .join(" ")
        .toLowerCase()
        .includes(s)
    );
  }, [rows, q]);

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t(config.titleKey)}</h1>
        <button className="btn btn-primary" onClick={openCreate}>
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
                {config.columns.map((c) => {
                  const labelKey = config.columnLabelKey?.[c];
                  const f = config.fields.find((x) => x.name === c);
                  const label = labelKey ? t(labelKey) : f ? t(f.labelKey) : c;
                  return <th key={c}>{label}</th>;
                })}
                <th>{t("common.actions")}</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((rec) => (
                <tr key={String(rec.id)}>
                  {config.columns.map((c) => {
                    const custom = config.columnText?.(c, rec);
                    const f = config.fields.find((x) => x.name === c);
                    const text =
                      custom != null
                        ? custom
                        : f
                          ? cellText(rec, f, t)
                          : String(rec[c] ?? "");
                    return <td key={c}>{text}</td>;
                  })}
                  <td>
                    <div className="icon-actions">
                      {config.rowAction && (
                        <button
                          type="button"
                          className="btn btn-success btn-activate"
                          onClick={() => config.rowAction?.onClick(rec)}
                        >
                          {t(config.rowAction.labelKey)}
                        </button>
                      )}
                      <IconButton
                        icon="edit"
                        title={t("common.edit")}
                        onClick={() => openEdit(rec)}
                      />
                      <IconButton
                        icon="delete"
                        title={t("common.delete")}
                        danger
                        onClick={() => handleDelete(rec)}
                      />
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {modalOpen && (
        <div className="modal-backdrop" onClick={() => setModalOpen(false)}>
          <div className="card modal" onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">
              {editing ? t("common.edit") : t("common.create")}
            </h2>
            <form onSubmit={handleSave}>
              {(config.vies || config.addressSplit) && (
                <div className="form-actions" style={{ justifyContent: "flex-start" }}>
                  {config.vies && (
                  <button
                    type="button"
                    className="btn"
                    onClick={handleViesFetch}
                    disabled={viesLoading || splitLoading}
                  >
                    {viesLoading
                      ? t(config.vies.loadingKey)
                      : t(config.vies.labelKey)}
                  </button>
                  )}
                  {config.addressSplit && (
                  <button
                    type="button"
                    className="btn"
                    onClick={handleSplitAddress}
                    disabled={splitLoading || viesLoading}
                  >
                    {splitLoading
                      ? t(config.addressSplit.loadingKey)
                      : t(config.addressSplit.labelKey)}
                  </button>
                  )}
                  {viesFilled && !viesError && config.vies && (
                    <span className="muted">{t(config.vies.filledKey)}</span>
                  )}
                  {viesNote && !viesError && (
                    <span className="muted">{viesNote}</span>
                  )}
                  {viesError && <span className="error-text">{viesError}</span>}
                </div>
              )}
              <div className="form-grid">
                {config.fields.map((f) => (
                  <div className="field" key={f.name}>
                    <label className="label">
                      {t(f.labelKey)}
                      {f.required ? " *" : ""}
                    </label>
                    {f.type === "select" ? (
                      <select
                        className="select"
                        value={String(form[f.name] ?? "")}
                        onChange={(e) => setField(f.name, e.target.value)}
                      >
                        {(f.options ?? []).map((o) => (
                          <option key={o.value} value={o.value}>
                            {t(o.labelKey)}
                          </option>
                        ))}
                      </select>
                    ) : f.type === "checkbox" ? (
                      <input
                        type="checkbox"
                        checked={Boolean(form[f.name])}
                        onChange={(e) => setField(f.name, e.target.checked)}
                      />
                    ) : f.type === "textarea" ? (
                      <textarea
                        className="input"
                        rows={3}
                        value={String(form[f.name] ?? "")}
                        onChange={(e) => setField(f.name, e.target.value)}
                        required={f.required}
                      />
                    ) : (
                      <input
                        className="input"
                        type={f.type === "number" ? "text" : "text"}
                        value={String(form[f.name] ?? "")}
                        onChange={(e) => setField(f.name, e.target.value)}
                        required={f.required}
                      />
                    )}
                  </div>
                ))}
              </div>
              {formError && <div className="error-text">{formError}</div>}
              <div className="form-actions">
                <button
                  type="button"
                  className="btn"
                  onClick={() => setModalOpen(false)}
                >
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
