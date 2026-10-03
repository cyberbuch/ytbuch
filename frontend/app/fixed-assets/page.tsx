"use client";

// ДМА — дълготрайни материални активи: активи (CRUD), категории (ЗКПО),
// месечна амортизация (изчисляване + осчетоводяване).

import { useCallback, useEffect, useState, FormEvent, Fragment } from "react";
import { api, ListResponse } from "../../lib/api";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { CrudPage, CrudConfig } from "../../components/CrudPage";
import { IconButton } from "../../components/IconButton";
import {
  CompanyLocation,
  fetchCompanyLocations,
} from "../../lib/locations";

import { Fa, Fac, DepPreview } from "./types";
import { FaDepreciationTab } from "./FaDepreciationTab";
import { FaActionModal, FaAssetModal } from "./FaModals";


const facConfig: CrudConfig = {
  endpoint: "/v1/fixed-asset-categories",
  titleKey: "fa.tab.categories",
  fields: [
    { name: "name", labelKey: "fa.name", type: "text", required: true },
    { name: "cita_category", labelKey: "fa.cita_category", type: "text" },
    { name: "min_depreciation_rate", labelKey: "fa.min_rate", type: "number", default: "0" },
    { name: "max_depreciation_rate", labelKey: "fa.max_rate", type: "number", default: "0" },
    {
      name: "default_method",
      labelKey: "fa.depreciation_method",
      type: "select",
      default: "linear",
      options: [
        { value: "linear", labelKey: "fa.depreciation_method.linear" },
        { value: "declining", labelKey: "fa.depreciation_method.declining" },
      ],
    },
  ],
  columns: ["name", "cita_category", "min_depreciation_rate", "max_depreciation_rate", "default_method"],
};

function FaInner() {
  const { t } = useI18n();
  const [tab, setTab] = useState<"assets" | "categories" | "depreciation">("assets");

  const [assets, setAssets] = useState<Fa[]>([]);
  const [categories, setCategories] = useState<Fac[]>([]);
  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  // модал за актив
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Fa | null>(null);
  const [form, setForm] = useState<Record<string, string | boolean>>({});
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");

  // амортизация
  const [depPeriod, setDepPeriod] = useState("");
  const [preview, setPreview] = useState<DepPreview | null>(null);
  const [posting, setPosting] = useState(false);

  // жизнен цикъл
  const [actionModal, setActionModal] = useState<Fa | null>(null);
  const [actionType, setActionType] = useState("revalue");
  const [actionDate, setActionDate] = useState("");
  const [actionAmount, setActionAmount] = useState("");
  const [actionLocation, setActionLocation] = useState("");
  const [actionReason, setActionReason] = useState("");
  const [disposalType, setDisposalType] = useState("write_off");
  const [events, setEvents] = useState<Record<string, unknown>[]>([]);
  const [eventsFor, setEventsFor] = useState<number | null>(null);
  const [actionError, setActionError] = useState("");
  const [actionBusy, setActionBusy] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const a = await api.get<ListResponse<Fa>>("/v1/fixed-assets");
      setAssets(a.items ?? []);
      const c = await api.get<ListResponse<Fac>>("/v1/fixed-asset-categories");
      setCategories(c.items ?? []);
      setLocations(await fetchCompanyLocations());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const catName = (id?: number) =>
    id ? categories.find((c) => c.id === id)?.name ?? String(id) : "";

  const openCreate = () => {
    setForm({
      name: "",
      inventory_number: "",
      category_id: "",
      acquisition_date: "",
      put_into_service_date: "",
      cost: "0",
      salvage_value: "0",
      useful_life_months: "0",
      depreciation_method: "linear",
      accounting_depreciation_rate: "0",
      tax_depreciation_rate: "0",
      status: "active",
      serial_number: "",
      manufacturer: "",
      model: "",
      responsible_person: "",
      location_id: "",
      is_conserved: false,
    });
    setEditing(null);
    setFormError("");
    setModalOpen(true);
  };

  const openEdit = (a: Fa) => {
    setForm({
      name: a.name,
      inventory_number: a.inventory_number,
      category_id: a.category_id ? String(a.category_id) : "",
      acquisition_date: a.acquisition_date,
      put_into_service_date: a.put_into_service_date,
      cost: a.cost,
      salvage_value: a.salvage_value,
      useful_life_months: a.useful_life_months,
      depreciation_method: a.depreciation_method,
      accounting_depreciation_rate: a.accounting_depreciation_rate,
      tax_depreciation_rate: a.tax_depreciation_rate,
      status: a.status,
      serial_number: a.serial_number,
      manufacturer: a.manufacturer,
      model: a.model,
      responsible_person: a.responsible_person,
      location_id: a.location_id ? String(a.location_id) : "",
      is_conserved: a.is_conserved === 1,
    });
    setEditing(a);
    setFormError("");
    setModalOpen(true);
  };

  const setField = (name: string, value: string | boolean) =>
    setForm((prev) => ({ ...prev, [name]: value }));

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError("");
    try {
      const payload: Record<string, unknown> = {
        name: form.name,
        inventory_number: form.inventory_number,
        category_id: Number(form.category_id) || 0,
        acquisition_date: form.acquisition_date,
        put_into_service_date: form.put_into_service_date,
        cost: form.cost,
        salvage_value: form.salvage_value,
        useful_life_months: form.useful_life_months,
        depreciation_method: form.depreciation_method,
        accounting_depreciation_rate: form.accounting_depreciation_rate,
        tax_depreciation_rate: form.tax_depreciation_rate,
        status: form.status,
        serial_number: form.serial_number,
        manufacturer: form.manufacturer,
        model: form.model,
        responsible_person: form.responsible_person,
        location_id: Number(form.location_id) || 0,
        is_conserved: form.is_conserved ? 1 : 0,
      };
      if (editing) {
        await api.patch(`/v1/fixed-assets/${editing.id}`, payload);
      } else {
        await api.post("/v1/fixed-assets", payload);
      }
      setModalOpen(false);
      await load();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (a: Fa) => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/fixed-assets/${a.id}`);
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handlePreview = async () => {
    if (!depPeriod) return;
    try {
      const p = await api.post<DepPreview>("/v1/fixed-assets/depreciation/preview", {});
      setPreview(p);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handlePost = async () => {
    if (!depPeriod) return;
    setPosting(true);
    setError("");
    try {
      await api.post("/v1/fixed-assets/depreciation/post", { period: depPeriod });
      await load();
      setPreview(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setPosting(false);
    }
  };

  const openAction = (a: Fa) => {
    setActionModal(a);
    setActionType("revalue");
    setActionDate("");
    setActionAmount("");
    setActionLocation("");
    setActionReason("");
    setDisposalType("write_off");
    setActionError("");
  };

  const handleActionSave = async () => {
    if (!actionModal) return;
    setActionBusy(true);
    setActionError("");
    try {
      const common = {
        event_date: actionDate,
        reason: actionReason,
      };
      if (actionType === "revalue") {
        await api.post(`/v1/fixed-assets/${actionModal.id}/revalue`, {
          ...common,
          new_value: actionAmount,
        });
      } else if (actionType === "move") {
        await api.post(`/v1/fixed-assets/${actionModal.id}/move`, {
          ...common,
          location_id: Number(actionLocation) || 0,
        });
      } else if (actionType === "conserve") {
        await api.post(`/v1/fixed-assets/${actionModal.id}/conserve`, common);
      } else if (actionType === "end_conserve") {
        await api.post(`/v1/fixed-assets/${actionModal.id}/end-conserve`, common);
      } else if (actionType === "dispose") {
        await api.post(`/v1/fixed-assets/${actionModal.id}/dispose`, {
          ...common,
          disposal_type: disposalType,
          sale_amount: actionAmount,
        });
      }
      setActionModal(null);
      await load();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionBusy(false);
    }
  };

  const showEvents = async (a: Fa) => {
    if (eventsFor === a.id) {
      setEventsFor(null);
      return;
    }
    setEventsFor(a.id);
    try {
      const data = await api.get<{ items: Record<string, unknown>[] }>(
        `/v1/fixed-assets/${a.id}/events`
      );
      setEvents(data.items ?? []);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const eventLabel = (et: unknown) => {
    const key = `fa.action.${String(et)}`;
    return t(key);
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("fa.title")}</h1>
        <div style={{ display: "flex", gap: 8 }}>
          {(
            [
              ["assets", "fa.tab.assets"],
              ["categories", "fa.tab.categories"],
              ["depreciation", "fa.tab.depreciation"],
            ] as const
          ).map(([value, key]) => (
            <button
              key={value}
              className={`btn btn-sm ${tab === value ? "btn-primary" : ""}`}
              onClick={() => setTab(value)}
            >
              {t(key)}
            </button>
          ))}
        </div>
      </div>

      {error && <div className="error-text">{error}</div>}

      {tab === "assets" && (
        <div>
          <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: 12 }}>
            <button className="btn btn-primary" onClick={openCreate}>
              {t("common.create")}
            </button>
          </div>
          <div className="card">
            {loading ? (
              <div className="content muted">{t("common.loading")}</div>
            ) : assets.length === 0 ? (
              <div className="content muted">{t("common.empty")}</div>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("fa.inventory_number")}</th>
                    <th>{t("fa.name")}</th>
                    <th>{t("fa.category")}</th>
                    <th>{t("fa.cost")}</th>
                    <th>{t("fa.accumulated")}</th>
                    <th>{t("fa.status")}</th>
                    <th>{t("fa.location")}</th>
                    <th>{t("common.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {assets.map((a) => (
                    <Fragment key={a.id}>
                    <tr>
                      <td>{a.inventory_number}</td>
                      <td>{a.name}</td>
                      <td>{catName(a.category_id)}</td>
                      <td>{a.cost}</td>
                      <td>{a.accumulated_depreciation}</td>
                      <td>{t(`fa.status.${a.status}`)}</td>
                      <td>
                        {locations.find((l) => l.id === a.location_id)?.name ?? ""}
                      </td>
                      <td>
                        <div className="icon-actions">
                          <IconButton
                            icon="action"
                            title={t("fa.action")}
                            onClick={() => openAction(a)}
                          />
                          <IconButton
                            icon="history"
                            title={t("fa.events")}
                            onClick={() => showEvents(a)}
                          />
                          <IconButton
                            icon="edit"
                            title={t("common.edit")}
                            onClick={() => openEdit(a)}
                          />
                          <IconButton
                            icon="delete"
                            title={t("common.delete")}
                            danger
                            onClick={() => handleDelete(a)}
                          />
                        </div>
                      </td>
                    </tr>
                    {eventsFor === a.id && (
                      <tr>
                        <td colSpan={8}>
                          <table className="table">
                            <thead>
                              <tr>
                                <th>{t("fa.action.event_date")}</th>
                                <th>{t("fa.action")}</th>
                                <th>{t("fa.action.location")}</th>
                                <th>{t("journal.amount")}</th>
                                <th>{t("fa.action.reason")}</th>
                              </tr>
                            </thead>
                            <tbody>
                              {events.map((ev) => (
                                <tr key={String(ev.id)}>
                                  <td>{String(ev.event_date)}</td>
                                  <td>{eventLabel(ev.event_type)}</td>
                                  <td>{String(ev.location ?? "")}</td>
                                  <td>{String(ev.amount ?? "")}</td>
                                  <td>{String(ev.reason ?? "")}</td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </td>
                      </tr>
                    )}
                    </Fragment>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      )}

      {tab === "categories" && <CrudPage config={facConfig} />}

      {tab === "depreciation" && (
        <FaDepreciationTab
          t={t}
          depPeriod={depPeriod}
          setDepPeriod={setDepPeriod}
          handlePreview={handlePreview}
          handlePost={handlePost}
          posting={posting}
          preview={preview}
        />
      )}

      {actionModal && (
        <FaActionModal
          t={t}
          actionModal={actionModal}
          setActionModal={setActionModal}
          actionType={actionType}
          setActionType={setActionType}
          actionDate={actionDate}
          setActionDate={setActionDate}
          actionAmount={actionAmount}
          setActionAmount={setActionAmount}
          actionLocation={actionLocation}
          setActionLocation={setActionLocation}
          disposalType={disposalType}
          setDisposalType={setDisposalType}
          actionReason={actionReason}
          setActionReason={setActionReason}
          actionError={actionError}
          actionBusy={actionBusy}
          locations={locations}
          handleActionSave={handleActionSave}
        />
      )}

      {modalOpen && (
        <FaAssetModal
          t={t}
          editing={editing}
          setModalOpen={setModalOpen}
          handleSave={handleSave}
          form={form}
          setField={setField}
          categories={categories}
          locations={locations}
          formError={formError}
          saving={saving}
        />
      )}
    </div>
  );
}

export default function FixedAssetsPage() {
  return (
    <RequireAuth>
      <FaInner />
    </RequireAuth>
  );
}
