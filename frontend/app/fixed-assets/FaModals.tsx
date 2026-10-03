"use client";

import { FormEvent } from "react";
import { CompanyLocation } from "../../lib/locations";
import { Fa, Fac } from "./types";

export function FaActionModal(props: {
  t: (k: string) => string;
  actionModal: Fa;
  setActionModal: (v: Fa | null) => void;
  actionType: string;
  setActionType: (v: string) => void;
  actionDate: string;
  setActionDate: (v: string) => void;
  actionAmount: string;
  setActionAmount: (v: string) => void;
  actionLocation: string;
  setActionLocation: (v: string) => void;
  disposalType: string;
  setDisposalType: (v: string) => void;
  actionReason: string;
  setActionReason: (v: string) => void;
  actionError: string;
  actionBusy: boolean;
  locations: CompanyLocation[];
  handleActionSave: () => void;
}) {
  const {
    t, actionModal, setActionModal, actionType, setActionType, actionDate, setActionDate,
    actionAmount, setActionAmount, actionLocation, setActionLocation, disposalType, setDisposalType,
    actionReason, setActionReason, actionError, actionBusy, locations, handleActionSave,
  } = props;
  return (
        <div className="modal-backdrop" onClick={() => setActionModal(null)}>
          <div className="card modal" onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">
              {t("fa.action")} — {actionModal.name}
            </h2>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void handleActionSave();
              }}
            >
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("fa.action")}</label>
                  <select
                    className="select"
                    value={actionType}
                    onChange={(e) => setActionType(e.target.value)}
                  >
                    <option value="revalue">{t("fa.action.revalue")}</option>
                    <option value="move">{t("fa.action.move")}</option>
                    <option value="conserve">{t("fa.action.conserve")}</option>
                    <option value="end_conserve">{t("fa.action.end_conserve")}</option>
                    <option value="dispose">{t("fa.action.dispose")}</option>
                  </select>
                </div>
                <div className="field">
                  <label className="label">{t("fa.action.event_date")}</label>
                  <input
                    className="input"
                    value={actionDate}
                    onChange={(e) => setActionDate(e.target.value)}
                    placeholder="2026-09-01"
                  />
                </div>
                {actionType === "revalue" && (
                  <div className="field">
                    <label className="label">{t("fa.action.new_value")} *</label>
                    <input
                      className="input"
                      value={actionAmount}
                      onChange={(e) => setActionAmount(e.target.value)}
                      required
                    />
                  </div>
                )}
                {actionType === "move" && (
                  <div className="field">
                    <label className="label">{t("fa.action.location")} *</label>
                    {locations.length > 0 ? (
                      <select
                        className="select"
                        value={actionLocation}
                        onChange={(e) => setActionLocation(e.target.value)}
                        required
                      >
                        <option value="">—</option>
                        {locations.map((loc) => (
                          <option key={loc.id} value={loc.id}>
                            {loc.name}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        className="input"
                        value={actionLocation}
                        onChange={(e) => setActionLocation(e.target.value)}
                        required
                      />
                    )}
                  </div>
                )}
                {actionType === "dispose" && (
                  <>
                    <div className="field">
                      <label className="label">{t("fa.action.disposal_type")}</label>
                      <select
                        className="select"
                        value={disposalType}
                        onChange={(e) => setDisposalType(e.target.value)}
                      >
                        <option value="write_off">{t("fa.action.disposal_type.write_off")}</option>
                        <option value="sale">{t("fa.action.disposal_type.sale")}</option>
                      </select>
                    </div>
                    <div className="field">
                      <label className="label">{t("fa.action.sale_amount")}</label>
                      <input
                        className="input"
                        value={actionAmount}
                        onChange={(e) => setActionAmount(e.target.value)}
                      />
                    </div>
                  </>
                )}
                <div className="field">
                  <label className="label">{t("fa.action.reason")}</label>
                  <input
                    className="input"
                    value={actionReason}
                    onChange={(e) => setActionReason(e.target.value)}
                  />
                </div>
              </div>
              {actionError && <div className="error-text">{actionError}</div>}
              <div className="form-actions">
                <button type="button" className="btn" onClick={() => setActionModal(null)}>
                  {t("common.cancel")}
                </button>
                <button className="btn btn-primary" disabled={actionBusy}>
                  {t("common.save")}
                </button>
              </div>
            </form>
          </div>
        </div>
  );
}

export function FaAssetModal(props: {
  t: (k: string) => string;
  editing: Fa | null;
  setModalOpen: (v: boolean) => void;
  handleSave: (e: FormEvent) => void;
  form: Record<string, string | boolean>;
  setField: (k: string, v: string | boolean) => void;
  categories: Fac[];
  locations: CompanyLocation[];
  formError: string;
  saving: boolean;
}) {
  const { t, editing, setModalOpen, handleSave, form, setField, categories, locations, formError, saving } = props;
  return (
        <div className="modal-backdrop" onClick={() => setModalOpen(false)}>
          <div className="card modal" onClick={(e) => e.stopPropagation()}>
            <h2 className="modal-title">
              {editing ? t("common.edit") : t("common.create")}
            </h2>
            <form onSubmit={handleSave}>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("fa.name")} *</label>
                  <input
                    className="input"
                    value={String(form.name)}
                    onChange={(e) => setField("name", e.target.value)}
                    required
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.inventory_number")}</label>
                  <input
                    className="input"
                    value={String(form.inventory_number)}
                    onChange={(e) => setField("inventory_number", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.category")}</label>
                  <select
                    className="select"
                    value={String(form.category_id)}
                    onChange={(e) => setField("category_id", e.target.value)}
                  >
                    <option value="">—</option>
                    {categories.map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.name}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label className="label">{t("fa.acquisition_date")}</label>
                  <input
                    className="input"
                    value={String(form.acquisition_date)}
                    onChange={(e) => setField("acquisition_date", e.target.value)}
                    placeholder="2026-01-15"
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.put_into_service_date")}</label>
                  <input
                    className="input"
                    value={String(form.put_into_service_date)}
                    onChange={(e) => setField("put_into_service_date", e.target.value)}
                    placeholder="2026-01-15"
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.cost")}</label>
                  <input
                    className="input"
                    value={String(form.cost)}
                    onChange={(e) => setField("cost", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.salvage_value")}</label>
                  <input
                    className="input"
                    value={String(form.salvage_value)}
                    onChange={(e) => setField("salvage_value", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.useful_life_months")}</label>
                  <input
                    className="input"
                    value={String(form.useful_life_months)}
                    onChange={(e) => setField("useful_life_months", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.depreciation_method")}</label>
                  <select
                    className="select"
                    value={String(form.depreciation_method)}
                    onChange={(e) => setField("depreciation_method", e.target.value)}
                  >
                    <option value="linear">{t("fa.depreciation_method.linear")}</option>
                    <option value="declining">{t("fa.depreciation_method.declining")}</option>
                  </select>
                </div>
                <div className="field">
                  <label className="label">{t("fa.accounting_rate")}</label>
                  <input
                    className="input"
                    value={String(form.accounting_depreciation_rate)}
                    onChange={(e) => setField("accounting_depreciation_rate", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.tax_rate")}</label>
                  <input
                    className="input"
                    value={String(form.tax_depreciation_rate)}
                    onChange={(e) => setField("tax_depreciation_rate", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.status")}</label>
                  <select
                    className="select"
                    value={String(form.status)}
                    onChange={(e) => setField("status", e.target.value)}
                  >
                    <option value="active">{t("fa.status.active")}</option>
                    <option value="sold">{t("fa.status.sold")}</option>
                    <option value="disposed">{t("fa.status.disposed")}</option>
                  </select>
                </div>
                <div className="field">
                  <label className="label">{t("fa.serial_number")}</label>
                  <input
                    className="input"
                    value={String(form.serial_number)}
                    onChange={(e) => setField("serial_number", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.manufacturer")}</label>
                  <input
                    className="input"
                    value={String(form.manufacturer)}
                    onChange={(e) => setField("manufacturer", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.model")}</label>
                  <input
                    className="input"
                    value={String(form.model)}
                    onChange={(e) => setField("model", e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("fa.responsible_person")}</label>
                  <input
                    className="input"
                    value={String(form.responsible_person)}
                    onChange={(e) => setField("responsible_person", e.target.value)}
                  />
                </div>
                {locations.length > 0 && (
                  <div className="field">
                    <label className="label">{t("fa.location")}</label>
                    <select
                      className="select"
                      value={String(form.location_id ?? "")}
                      onChange={(e) => setField("location_id", e.target.value)}
                    >
                      <option value="">—</option>
                      {locations.map((loc) => (
                        <option key={loc.id} value={loc.id}>
                          {loc.name}
                        </option>
                      ))}
                    </select>
                  </div>
                )}
                <div className="field">
                  <label className="label">{t("fa.is_conserved")}</label>
                  <input
                    type="checkbox"
                    checked={Boolean(form.is_conserved)}
                    onChange={(e) => setField("is_conserved", e.target.checked)}
                  />
                </div>
              </div>
              {formError && <div className="error-text">{formError}</div>}
              <div className="form-actions">
                <button type="button" className="btn" onClick={() => setModalOpen(false)}>
                  {t("common.cancel")}
                </button>
                <button className="btn btn-primary" disabled={saving}>
                  {t("common.save")}
                </button>
              </div>
            </form>
          </div>
        </div>
  );
}
