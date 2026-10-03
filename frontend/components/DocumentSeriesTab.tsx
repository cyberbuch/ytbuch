"use client";

import { FormEvent, useMemo, useState } from "react";
import { api } from "../lib/api";
import { DOC_TYPES_OUT, DOC_TYPE_PROFORMA } from "../lib/invoice";
import { useI18n } from "./I18nProvider";
import { IconButton } from "./IconButton";

export interface DocSeries {
  id: number;
  name: string;
  document_types: string;
  pad_width: number;
  start_number: number;
  next_number?: string;
}

const SERIES_TYPES = [DOC_TYPE_PROFORMA, ...DOC_TYPES_OUT, "internal"];

const DEFAULTS: { nameKey: string; types: string[] }[] = [
  { nameKey: "settings.series.default.invoices", types: ["01", "02", "03"] },
  { nameKey: "settings.series.default.proforma", types: ["proforma"] },
  { nameKey: "settings.series.default.cash_acc", types: ["11", "12", "13"] },
  { nameKey: "settings.series.default.protocols", types: ["09", "29", "50", "91", "93", "94", "95"] },
  { nameKey: "settings.series.default.reports", types: ["81", "82", "83", "84", "85"] },
  { nameKey: "settings.series.default.goods", types: ["04"] },
  { nameKey: "settings.series.default.internal", types: ["internal"] },
];

function parseTypes(csv: string): string[] {
  return csv
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
}

export function DocumentSeriesTab({
  companyId,
  series,
  onChange,
  onError,
}: {
  companyId: number;
  series: DocSeries[];
  onChange: () => Promise<void>;
  onError: (msg: string) => void;
}) {
  const { t } = useI18n();
  const [show, setShow] = useState(false);
  const [editId, setEditId] = useState(0);
  const [name, setName] = useState("");
  const [start, setStart] = useState("1");
  const [pad, setPad] = useState("10");
  const [types, setTypes] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  const usedElsewhere = useMemo(() => {
    const map = new Map<string, string>();
    for (const s of series) {
      if (s.id === editId) continue;
      for (const ty of parseTypes(s.document_types)) {
        map.set(ty, s.name);
      }
    }
    return map;
  }, [series, editId]);

  const openNew = () => {
    setEditId(0);
    setName("");
    setStart("1");
    setPad("10");
    setTypes([]);
    setShow(true);
  };

  const openEdit = (s: DocSeries) => {
    setEditId(s.id);
    setName(s.name);
    setStart(String(s.start_number || 1));
    setPad(String(s.pad_width || 10));
    setTypes(parseTypes(s.document_types));
    setShow(true);
  };

  const toggle = (ty: string) => {
    setTypes((prev) =>
      prev.includes(ty) ? prev.filter((x) => x !== ty) : [...prev, ty]
    );
  };

  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setSaving(true);
    try {
      const body = {
        name: name.trim(),
        document_types: types,
        start_number: Number(start) || 1,
        pad_width: Number(pad) || 10,
      };
      if (editId > 0) {
        await api.patch(`/v1/document-series/${editId}`, body);
      } else {
        await api.post(`/v1/companies/${companyId}/document-series`, body);
      }
      setShow(false);
      await onChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const del = async (id: number) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/document-series/${id}`);
      await onChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  };

  const seedDefaults = async () => {
    setSaving(true);
    try {
      for (const d of DEFAULTS) {
        await api.post(`/v1/companies/${companyId}/document-series`, {
          name: t(d.nameKey),
          document_types: d.types,
          start_number: 1,
          pad_width: 10,
        });
      }
      await onChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const typeLabel = (ty: string) => t(`invoices.document_type.${ty}`);

  return (
    <div>
      <p className="muted" style={{ marginTop: 0 }}>
        {t("settings.series.hint")}
      </p>
      <div className="page-head" style={{ marginBottom: 12 }}>
        <h3 className="section-title" style={{ margin: 0 }}>
          {t("settings.series.title")}
        </h3>
        <div className="btn-row">
          {series.length === 0 && (
            <button type="button" className="btn btn-sm" onClick={seedDefaults} disabled={saving}>
              {t("settings.series.defaults")}
            </button>
          )}
          <button type="button" className="btn btn-sm btn-primary" onClick={openNew}>
            {t("settings.series.add")}
          </button>
        </div>
      </div>

      {show && (
        <form className="inline-form" onSubmit={save} style={{ marginBottom: 16 }}>
          <h4 className="section-title" style={{ fontSize: 13 }}>
            {editId > 0 ? t("common.edit") : t("common.create")}
          </h4>
          <div className="form-grid">
            <div className="field" style={{ gridColumn: "1 / -1" }}>
              <label className="label">{t("settings.series.name")} *</label>
              <input className="input" value={name} onChange={(e) => setName(e.target.value)} required />
            </div>
            <div className="field">
              <label className="label">{t("settings.series.start")}</label>
              <input className="input" value={start} onChange={(e) => setStart(e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("settings.series.pad")}</label>
              <input className="input" value={pad} onChange={(e) => setPad(e.target.value)} />
            </div>
          </div>
          <div className="field" style={{ marginTop: 10 }}>
            <label className="label">{t("settings.series.types")}</label>
            <div className="series-types">
              {SERIES_TYPES.map((ty) => {
                const other = usedElsewhere.get(ty);
                return (
                  <label key={ty} className="check-inline" title={other ? t("settings.series.used_in") + " " + other : ""}>
                    <input
                      type="checkbox"
                      checked={types.includes(ty)}
                      onChange={() => toggle(ty)}
                    />
                    <span>
                      {typeLabel(ty)}
                      {other ? <span className="muted"> ({other})</span> : null}
                    </span>
                  </label>
                );
              })}
            </div>
          </div>
          <div className="form-actions">
            <button type="button" className="btn" onClick={() => setShow(false)}>
              {t("common.cancel")}
            </button>
            <button className="btn btn-primary" type="submit" disabled={saving}>
              {t("common.save")}
            </button>
          </div>
        </form>
      )}

      {series.length === 0 && !show && (
        <p className="muted">{t("settings.series.empty")}</p>
      )}
      {series.map((s) => (
        <div className="list-item" key={s.id}>
          <div>
            <div style={{ fontWeight: 600 }}>{s.name}</div>
            <div className="muted" style={{ fontSize: 12 }}>
              {t("settings.series.next")}: {s.next_number || "—"}
              {" · "}
              {parseTypes(s.document_types).map(typeLabel).join(", ") || t("settings.series.no_types")}
            </div>
          </div>
          <div className="icon-actions">
            <IconButton icon="edit" title={t("common.edit")} onClick={() => openEdit(s)} />
            <IconButton icon="delete" title={t("common.delete")} danger onClick={() => del(s.id)} />
          </div>
        </div>
      ))}
    </div>
  );
}
