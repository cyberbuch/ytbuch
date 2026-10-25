"use client";

// Пълната форма за артикул — една и съща в „Стоки и услуги“, във фактурата
// и при сканиране. Полетата и поведението (КН8, вид запас, сметки,
// автоматично изписване) не трябва да се разминават между трите места.

import { useEffect, useState } from "react";
import { api, ListResponse } from "../lib/api";
import { UnitOfMeasure } from "../lib/units";
import { useI18n } from "./I18nProvider";
import { UnitPicker } from "./UnitPicker";
import { Cn8Item, Cn8Picker } from "./Cn8Picker";

export interface ProductFormAccount {
  id: number;
  number: string;
  name: string;
}

interface TypeNom {
  code: string;
  name: string;
}

interface Cn8Suggestion {
  cn8_code: string;
  cn8_name: string;
  unit: string;
  product_type: string;
  is_service: boolean;
  source: string;
  candidates: Cn8Item[];
}

export const emptyProductForm = {
  name: "",
  code: "",
  description: "",
  unit: "C62",
  price: "0",
  cost_price: "0",
  vat_rate: "20",
  is_service: false,
  is_inventory: false,
  inventory_account_id: 0,
  revenue_account_id: 0,
  stock_quantity: "0",
  is_active: true,
  product_type: "",
  cn8_code: "",
  cn8_name: "",
  expense_account_id: 0,
  auto_write_off: false,
};

export type ProductFormValues = typeof emptyProductForm;

export function productAccountLabel(a: ProductFormAccount): string {
  return `${a.number} ${a.name}`;
}

export function productPayload(form: ProductFormValues) {
  return {
    name: form.name.trim(),
    code: form.code.trim(),
    description: form.description,
    unit: form.unit || "C62",
    price: form.price || "0",
    cost_price: form.cost_price || "0",
    vat_rate: form.vat_rate || "20",
    is_service: form.is_service,
    is_inventory: form.is_service ? false : form.is_inventory,
    inventory_account_id: form.inventory_account_id,
    revenue_account_id: form.revenue_account_id,
    stock_quantity: form.stock_quantity || "0",
    is_active: form.is_active,
    product_type: form.product_type,
    cn8_code: form.cn8_code,
    cn8_name: form.cn8_name,
    expense_account_id: form.expense_account_id,
    auto_write_off: form.auto_write_off,
  };
}

function accountId(accounts: ProductFormAccount[], number: string): number {
  return accounts.find((a) => a.number === number)?.id ?? 0;
}

function withProductType(
  form: ProductFormValues,
  type: string,
  accounts: ProductFormAccount[]
): ProductFormValues {
  const next = { ...form, product_type: type };
  if (type === "10") {
    next.is_service = false;
    next.is_inventory = true;
    if (form.product_type !== "10" || !next.inventory_account_id) {
      next.inventory_account_id = accountId(accounts, "302") || next.inventory_account_id;
    }
    if (form.product_type !== "10" || !next.revenue_account_id) {
      next.revenue_account_id = accountId(accounts, "706") || next.revenue_account_id;
    }
    if (form.product_type !== "10" || !next.expense_account_id) {
      next.expense_account_id = accountId(accounts, "601") || next.expense_account_id;
    }
  } else {
    next.auto_write_off = false;
    next.expense_account_id = 0;
    if (type === "30" && form.product_type === "10") {
      next.inventory_account_id = accountId(accounts, "304") || next.inventory_account_id;
      next.revenue_account_id = accountId(accounts, "702") || next.revenue_account_id;
    }
  }
  return next;
}

export function ProductForm({
  accounts,
  units,
  initial,
  stockValue,
  onSubmit,
  onCancel,
  cancelLabel,
}: {
  accounts: ProductFormAccount[];
  units: UnitOfMeasure[];
  initial?: ProductFormValues;
  stockValue?: string;
  onSubmit: (form: ProductFormValues) => Promise<void>;
  onCancel: () => void;
  cancelLabel?: string;
}) {
  const { t } = useI18n();
  const [form, setForm] = useState<ProductFormValues>(initial ?? emptyProductForm);
  const [productTypes, setProductTypes] = useState<TypeNom[]>([]);
  const [cn8Candidates, setCn8Candidates] = useState<Cn8Item[]>([]);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiNote, setAiNote] = useState("");
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  useEffect(() => {
    let active = true;
    api
      .get<ListResponse<TypeNom>>("/v1/saft/nomenclatures", { kind: "product_types" })
      .then((data) => {
        if (active) setProductTypes(data.items ?? []);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const suggestCn8 = async () => {
    setAiBusy(true);
    setFormError("");
    setAiNote("");
    try {
      const result = await api.post<Cn8Suggestion>("/v1/products/cn8-suggest", {
        name: form.name,
        description: form.description,
        unit: form.unit,
        product_type: form.product_type,
        is_service: form.is_service,
      });
      let next: ProductFormValues = {
        ...form,
        cn8_code: result.cn8_code,
        cn8_name: result.cn8_name,
        unit: result.unit || form.unit,
        is_service: result.is_service,
        is_inventory: result.is_service
          ? false
          : form.is_inventory || Boolean(result.product_type),
      };
      if (result.is_service) {
        next = {
          ...next,
          is_service: true,
          is_inventory: false,
          product_type: "",
          expense_account_id: 0,
          auto_write_off: false,
        };
      } else {
        next = withProductType(next, result.product_type || form.product_type, accounts);
      }
      setForm(next);
      setCn8Candidates(result.candidates ?? []);
      setAiNote(
        `${result.cn8_code} — ${t(result.source === "ai" ? "products.cn8_ai_source" : "products.cn8_catalog_source")}`
      );
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setAiBusy(false);
    }
  };

  // Без <form>: picker-ите са вътре във формата на фактурата, а вложените
  // форми са невалиден HTML и пращат нативен submit (презареждане) вместо
  // React onSubmit. Записът е с бутон type="button".
  const save = async () => {
    if (!form.name.trim()) {
      setFormError(t("scan.need_product_name"));
      return;
    }
    setSaving(true);
    setFormError("");
    try {
      await onSubmit(form);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const accountOptions = (value: number, onChange: (id: number) => void) => (
    <select
      className="select"
      value={String(value)}
      onChange={(e) => onChange(Number(e.target.value) || 0)}
    >
      <option value="0">{t("products.account_required")}</option>
      {accounts.map((a) => (
        <option key={a.id} value={a.id}>
          {productAccountLabel(a)}
        </option>
      ))}
    </select>
  );

  return (
    <div>
      <div className="form-grid">
        <div className="field">
          <label className="label">{t("products.name")} *</label>
          <input
            className="input"
            required
            autoFocus
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
          />
        </div>
        <div className="field">
          <label className="label">{t("products.code")}</label>
          <input
            className="input"
            value={form.code}
            onChange={(e) => setForm({ ...form, code: e.target.value })}
            placeholder={t("products.code_hint")}
          />
        </div>
        <div className="field" style={{ gridColumn: "1 / -1" }}>
          <label className="label">{t("products.description")}</label>
          <textarea
            className="input"
            rows={2}
            value={form.description}
            onChange={(e) => setForm({ ...form, description: e.target.value })}
          />
        </div>
        <div className="field">
          <label className="label">{t("products.product_type")}</label>
          <select
            className="select"
            value={form.product_type}
            disabled={form.is_service}
            onChange={(e) => setForm(withProductType(form, e.target.value, accounts))}
          >
            <option value="">{t("products.product_type_pick")}</option>
            {productTypes.map((type) => (
              <option key={type.code} value={type.code}>
                {type.code} — {type.name}
              </option>
            ))}
          </select>
        </div>
        <div className="field" style={{ gridColumn: "1 / -1" }}>
          <button
            type="button"
            className="btn"
            disabled={aiBusy || !form.name.trim()}
            onClick={suggestCn8}
          >
            {aiBusy ? t("products.cn8_ai_busy") : t("products.cn8_ai_btn")}
          </button>
          {aiNote && <span className="muted" style={{ marginLeft: 10 }}>{aiNote}</span>}
        </div>
        <div className="field" style={{ gridColumn: "1 / -1" }}>
          <label className="label">{t("products.cn8_code")}</label>
          <Cn8Picker
            code={form.cn8_code}
            name={form.cn8_name}
            candidates={cn8Candidates}
            onSelect={(cn8_code, cn8_name) => setForm({ ...form, cn8_code, cn8_name })}
          />
        </div>
        <div className="field" style={{ gridColumn: "1 / -1" }}>
          <label className="label">{t("products.unit")}</label>
          <UnitPicker
            value={form.unit}
            onChange={(unit) => setForm({ ...form, unit })}
            units={units}
          />
        </div>
        <div className="field">
          <label className="label">{t("products.price")}</label>
          <input
            className="input"
            value={form.price}
            onChange={(e) => setForm({ ...form, price: e.target.value })}
          />
        </div>
        <div className="field">
          <label className="label">{t("products.cost_price")}</label>
          <input
            className="input"
            value={form.cost_price}
            onChange={(e) => setForm({ ...form, cost_price: e.target.value })}
          />
        </div>
        <div className="field">
          <label className="label">{t("products.vat_rate")}</label>
          <input
            className="input"
            value={form.vat_rate}
            onChange={(e) => setForm({ ...form, vat_rate: e.target.value })}
          />
        </div>
        <div className="field">
          <label className="label">{t("products.revenue_account")}</label>
          {accountOptions(form.revenue_account_id, (id) =>
            setForm({ ...form, revenue_account_id: id })
          )}
        </div>
        <div className="field">
          <label className="label">{t("products.purchase_account")}</label>
          {accountOptions(form.inventory_account_id, (id) =>
            setForm({ ...form, inventory_account_id: id })
          )}
        </div>
        {form.product_type === "10" && (
          <>
            <div className="field">
              <label className="label">{t("products.expense_account")}</label>
              {accountOptions(form.expense_account_id, (id) =>
                setForm({ ...form, expense_account_id: id })
              )}
            </div>
            <div className="field" style={{ gridColumn: "1 / -1" }}>
              <label className="check-inline">
                <input
                  type="checkbox"
                  checked={form.auto_write_off}
                  onChange={(e) => setForm({ ...form, auto_write_off: e.target.checked })}
                />
                {t("products.auto_write_off")}
              </label>
              <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>
                {form.auto_write_off
                  ? t("products.auto_write_off_hint")
                  : t("products.manual_write_off_hint")}
              </div>
            </div>
          </>
        )}
        <div className="field">
          <label className="check-inline">
            <input
              type="checkbox"
              checked={form.is_service}
              onChange={(e) =>
                setForm({
                  ...form,
                  is_service: e.target.checked,
                  is_inventory: e.target.checked ? false : form.is_inventory,
                  product_type: e.target.checked ? "" : form.product_type,
                  expense_account_id: e.target.checked ? 0 : form.expense_account_id,
                  auto_write_off: e.target.checked ? false : form.auto_write_off,
                  cn8_code: e.target.checked && !form.cn8_code ? "00000000" : form.cn8_code,
                  cn8_name: e.target.checked && !form.cn8_name ? "Услуги" : form.cn8_name,
                })
              }
            />
            {t("products.is_service")}
          </label>
        </div>
        <div className="field">
          <label className="check-inline">
            <input
              type="checkbox"
              checked={form.is_inventory}
              disabled={form.is_service}
              onChange={(e) =>
                setForm({
                  ...form,
                  is_inventory: e.target.checked,
                  auto_write_off: e.target.checked ? form.auto_write_off : false,
                })
              }
            />
            {t("products.is_inventory")}
          </label>
        </div>
        <div className="field">
          <label className="check-inline">
            <input
              type="checkbox"
              checked={form.is_active}
              onChange={(e) => setForm({ ...form, is_active: e.target.checked })}
            />
            {t("products.is_active")}
          </label>
        </div>
        {form.is_inventory && stockValue !== undefined && (
          <div className="field">
            <label className="label">{t("products.stock_quantity")}</label>
            <input className="input" value={form.stock_quantity} disabled readOnly />
            <div className="muted" style={{ fontSize: 12 }}>
              {t("products.stock_hint")} {stockValue}
            </div>
          </div>
        )}
      </div>
      {formError && <div className="error-text">{formError}</div>}
      <div className="form-actions">
        <button type="button" className="btn" disabled={saving} onClick={onCancel}>
          {cancelLabel ?? t("common.cancel")}
        </button>
        <button type="button" className="btn btn-primary" disabled={saving} onClick={save}>
          {t("common.save")}
        </button>
      </div>
    </div>
  );
}
