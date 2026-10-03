"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ListResponse } from "../lib/api";
import { useI18n } from "./I18nProvider";
import {
  CompanyLocation,
  StockAtLocation,
  fetchCompanyLocations,
  fetchStockAtLocation,
} from "../lib/locations";
import {
  InternalDoc,
  InternalDocLine,
  emptyIdocLine,
  todayIso,
} from "../lib/internalDoc";

interface FaRow {
  id: number;
  name: string;
  inventory_number: string;
  location_id?: number;
  cost: string;
}

interface Props {
  mode: "create" | "edit";
  docId?: number;
}

export function InternalDocForm({ mode, docId }: Props) {
  const { t } = useI18n();
  const router = useRouter();
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");
  const [loading, setLoading] = useState(true);

  const [number, setNumber] = useState("");
  const [docDate, setDocDate] = useState(todayIso());
  const [fromId, setFromId] = useState("0");
  const [toId, setToId] = useState("0");
  const [notes, setNotes] = useState("");
  const [handedBy, setHandedBy] = useState("");
  const [receivedBy, setReceivedBy] = useState("");
  const [lines, setLines] = useState<InternalDocLine[]>([emptyIdocLine()]);

  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [locsReady, setLocsReady] = useState(false);
  const [stock, setStock] = useState<StockAtLocation[]>([]);
  const [assets, setAssets] = useState<FaRow[]>([]);

  const loadLookups = useCallback(async () => {
    const locs = await fetchCompanyLocations();
    setLocations(locs);
    setLocsReady(true);
    try {
      const fa = await api.get<ListResponse<FaRow>>("/v1/fixed-assets");
      setAssets(fa.items ?? []);
    } catch {
      setAssets([]);
    }
    if (mode === "create") {
      try {
        const n = await api.get<{ number: string }>("/v1/internal-docs/next-number");
        setNumber(n.number);
      } catch {
        /* keep empty */
      }
      setLoading(false);
    }
  }, [mode]);

  useEffect(() => {
    loadLookups();
  }, [loadLookups]);

  useEffect(() => {
    const id = Number(fromId);
    if (!id) {
      setStock([]);
      return;
    }
    fetchStockAtLocation(id)
      .then(setStock)
      .catch(() => setStock([]));
  }, [fromId]);

  useEffect(() => {
    if (mode !== "edit" || !docId) return;
    let cancelled = false;
    (async () => {
      try {
        const doc = await api.get<InternalDoc>(`/v1/internal-docs/${docId}`);
        if (cancelled) return;
        setNumber(doc.number);
        setDocDate(doc.doc_date);
        setFromId(String(doc.from_location_id || 0));
        setToId(String(doc.to_location_id || 0));
        setNotes(doc.notes || "");
        setHandedBy(doc.handed_by || "");
        setReceivedBy(doc.received_by || "");
        setLines(doc.lines && doc.lines.length ? doc.lines : [emptyIdocLine()]);
      } catch (e) {
        if (!cancelled) setFormError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [mode, docId]);

  const assetsAtFrom = assets.filter(
    (a) => !fromId || String(a.location_id || 0) === fromId || !a.location_id
  );

  const setLine = (i: number, patch: Partial<InternalDocLine>) => {
    setLines((prev) => prev.map((l, idx) => (idx === i ? { ...l, ...patch } : l)));
  };

  const pickProduct = (i: number, pid: number) => {
    const p = stock.find((s) => s.product_id === pid);
    if (!p) {
      setLine(i, { product_id: pid, asset_id: 0 });
      return;
    }
    const qty = "1";
    const cost = p.unit_cost || "0";
    setLine(i, {
      product_id: pid,
      asset_id: 0,
      description: p.name,
      quantity: qty,
      unit_cost: cost,
      amount: cost,
    });
  };

  const pickAsset = (i: number, aid: number) => {
    const a = assets.find((x) => x.id === aid);
    if (!a) {
      setLine(i, { asset_id: aid, product_id: 0 });
      return;
    }
    setLine(i, {
      asset_id: aid,
      product_id: 0,
      description: `${a.inventory_number} ${a.name}`.trim(),
      quantity: "1",
      unit_cost: a.cost || "0",
      amount: a.cost || "0",
    });
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError("");
    try {
      if (Number(fromId) === Number(toId) || !Number(fromId) || !Number(toId)) {
        setFormError(t("idoc.need_locations"));
        setSaving(false);
        return;
      }
      const payload = {
        number,
        doc_date: docDate,
        from_location_id: Number(fromId),
        to_location_id: Number(toId),
        notes,
        handed_by: handedBy,
        received_by: receivedBy,
        lines: lines.map((l) => ({
          line_kind: l.line_kind,
          product_id: l.line_kind === "product" ? l.product_id || 0 : 0,
          asset_id: l.line_kind === "fixed_asset" ? l.asset_id || 0 : 0,
          quantity: l.quantity,
          unit_cost: l.unit_cost,
          amount: l.amount,
          description: l.description,
        })),
      };
      if (mode === "edit" && docId) {
        await api.patch(`/v1/internal-docs/${docId}`, payload);
        router.push(`/internal-docs/${docId}`);
      } else {
        const created = await api.post<InternalDoc>("/v1/internal-docs", payload);
        router.push(`/internal-docs/${created.id}`);
      }
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  if (loading || !locsReady) return <p className="muted">{t("common.loading")}</p>;

  if (locations.length < 2) {
    return <p className="muted">{t("idoc.need_locations")}</p>;
  }

  return (
    <form className="invoice-form" onSubmit={handleSave}>
      <section className="card invoice-section">
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("idoc.number")}</label>
            <input className="input" value={number} onChange={(e) => setNumber(e.target.value)} />
          </div>
          <div className="field">
            <label className="label">{t("idoc.date")} *</label>
            <input
              className="input"
              type="date"
              value={docDate}
              onChange={(e) => setDocDate(e.target.value)}
              required
            />
          </div>
          <div className="field">
            <label className="label">{t("idoc.from")} *</label>
            <select
              className="select"
              value={fromId}
              onChange={(e) => setFromId(e.target.value)}
              required
            >
              <option value="0">{t("idoc.pick_from")}</option>
              {locations.map((loc) => (
                <option key={loc.id} value={loc.id}>
                  {loc.name}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label className="label">{t("idoc.to")} *</label>
            <select
              className="select"
              value={toId}
              onChange={(e) => setToId(e.target.value)}
              required
            >
              <option value="0">—</option>
              {locations
                .filter((loc) => String(loc.id) !== fromId)
                .map((loc) => (
                  <option key={loc.id} value={loc.id}>
                    {loc.name}
                  </option>
                ))}
            </select>
          </div>
          <div className="field">
            <label className="label">{t("idoc.handed_by")}</label>
            <input className="input" value={handedBy} onChange={(e) => setHandedBy(e.target.value)} />
          </div>
          <div className="field">
            <label className="label">{t("idoc.received_by")}</label>
            <input
              className="input"
              value={receivedBy}
              onChange={(e) => setReceivedBy(e.target.value)}
            />
          </div>
        </div>
        <div className="field" style={{ marginTop: 12 }}>
          <label className="label">{t("idoc.notes")}</label>
          <textarea className="input" value={notes} onChange={(e) => setNotes(e.target.value)} />
        </div>
      </section>

      <section className="card invoice-section">
        <div className="page-head">
          <h2 className="invoice-section-title">{t("idoc.lines")}</h2>
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => setLines((prev) => [...prev, emptyIdocLine()])}
          >
            {t("idoc.add_line")}
          </button>
        </div>
        <table className="table">
          <thead>
            <tr>
              <th>{t("idoc.line_kind")}</th>
              <th>{t("products.name")}</th>
              <th>{t("idoc.qty")}</th>
              <th>{t("idoc.cost")}</th>
              <th>{t("idoc.amount")}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {lines.map((line, i) => (
              <tr key={i}>
                <td>
                  <select
                    className="select"
                    value={line.line_kind}
                    onChange={(e) =>
                      setLine(i, {
                        line_kind: e.target.value as InternalDocLine["line_kind"],
                        product_id: 0,
                        asset_id: 0,
                        description: "",
                      })
                    }
                  >
                    <option value="product">{t("idoc.line_kind.product")}</option>
                    <option value="fixed_asset">{t("idoc.line_kind.fixed_asset")}</option>
                  </select>
                </td>
                <td>
                  {line.line_kind === "product" ? (
                    <select
                      className="select"
                      value={line.product_id || ""}
                      onChange={(e) => pickProduct(i, Number(e.target.value))}
                    >
                      <option value="">—</option>
                      {stock.map((s) => (
                        <option key={s.product_id} value={s.product_id}>
                          {s.name} ({s.quantity})
                        </option>
                      ))}
                    </select>
                  ) : (
                    <select
                      className="select"
                      value={line.asset_id || ""}
                      onChange={(e) => pickAsset(i, Number(e.target.value))}
                    >
                      <option value="">—</option>
                      {assetsAtFrom.map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.inventory_number} {a.name}
                        </option>
                      ))}
                    </select>
                  )}
                </td>
                <td>
                  <input
                    className="input"
                    value={line.quantity}
                    onChange={(e) => {
                      const quantity = e.target.value;
                      const amount = String(
                        (Number(quantity) || 0) * (Number(line.unit_cost) || 0)
                      );
                      setLine(i, { quantity, amount });
                    }}
                    disabled={line.line_kind === "fixed_asset"}
                  />
                </td>
                <td>{line.unit_cost}</td>
                <td>{line.amount}</td>
                <td>
                  <button
                    type="button"
                    className="btn btn-sm btn-danger"
                    onClick={() => setLines((prev) => prev.filter((_, idx) => idx !== i))}
                    disabled={lines.length < 2}
                  >
                    {t("common.delete")}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {Number(fromId) > 0 && stock.length === 0 && (
          <p className="muted">{t("idoc.none_from")}</p>
        )}
      </section>

      {formError && <div className="error-text">{formError}</div>}
      <div className="form-actions">
        <button type="button" className="btn" onClick={() => router.push("/internal-docs")}>
          {t("common.cancel")}
        </button>
        <button className="btn btn-primary" disabled={saving}>
          {saving ? t("common.loading") : t("common.save")}
        </button>
      </div>
    </form>
  );
}
