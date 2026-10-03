"use client";

// Настройки на активната фирма — по образец на secret/su-doxis settings.rs.
// Таб «Фирма»: реквизити, управител/счетоводител, ДДС. Таб «SAF-T / Собственици»:
// улица/сграда/регион, обекти (поделения), действителни собственици, предприятия-майки.

import { FormEvent, useCallback, useEffect, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { DocumentSeriesTab, DocSeries } from "../../components/DocumentSeriesTab";
import { FsFormulasTab } from "../../components/FsFormulasTab";
import { SettingsCompanyTab } from "../../components/SettingsCompanyTab";
import { SettingsSaftTab } from "../../components/SettingsSaftTab";
import {
  EMPTY_LOC,
  EMPTY_OWNER,
  EMPTY_PARENT,
  Form,
  Location,
  Owner,
  ParentCo,
  SettingsPack,
} from "../../components/settingsTypes";
import {
  ACTIVE_COMPANY_EVENT,
  api,
  getActiveCompany,
} from "../../lib/api";

function str(v: unknown, fallback = ""): string {
  if (v === null || v === undefined) return fallback;
  return String(v);
}

function parseKeys(raw: unknown): { mistral_api_key: string; zhipu_api_key: string } {
  const empty = { mistral_api_key: "", zhipu_api_key: "" };
  if (typeof raw !== "string" || !raw) return empty;
  try {
    const o = JSON.parse(raw) as Record<string, unknown>;
    return {
      mistral_api_key: str(o.mistral_api_key),
      zhipu_api_key: str(o.zhipu_api_key),
    };
  } catch {
    return empty;
  }
}

function SettingsInner() {
  const { t } = useI18n();
  const [activeId, setActiveId] = useState(0);
  const [tab, setTab] = useState<0 | 1 | 2 | 3>(0);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [splitting, setSplitting] = useState(false);
  const [splitNote, setSplitNote] = useState("");
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");

  const [companyForm, setCompanyForm] = useState<Form>({});
  const [saftForm, setSaftForm] = useState<Form>({});
  const [keys, setKeys] = useState({ mistral_api_key: "", zhipu_api_key: "" });
  const [rawSettings, setRawSettings] = useState("{}");

  const [locations, setLocations] = useState<Location[]>([]);
  const [owners, setOwners] = useState<Owner[]>([]);
  const [parents, setParents] = useState<ParentCo[]>([]);
  const [series, setSeries] = useState<DocSeries[]>([]);

  const [showLoc, setShowLoc] = useState(false);
  const [editLocId, setEditLocId] = useState(0);
  const [locForm, setLocForm] = useState<Form>(EMPTY_LOC);

  const [showOwner, setShowOwner] = useState(false);
  const [editOwnerId, setEditOwnerId] = useState(0);
  const [ownerForm, setOwnerForm] = useState<Form>(EMPTY_OWNER);

  const [showParent, setShowParent] = useState(false);
  const [editParentId, setEditParentId] = useState(0);
  const [parentForm, setParentForm] = useState<Form>(EMPTY_PARENT);

  const setC = (k: string, v: string) => setCompanyForm((p) => ({ ...p, [k]: v }));
  const setS = (k: string, v: string) => setSaftForm((p) => ({ ...p, [k]: v }));
  const setL = (k: string, v: string) => setLocForm((p) => ({ ...p, [k]: v }));
  const setO = (k: string, v: string) => setOwnerForm((p) => ({ ...p, [k]: v }));
  const setP = (k: string, v: string) => setParentForm((p) => ({ ...p, [k]: v }));

  const applyPack = (pack: SettingsPack) => {
    const c = pack.company;
    setCompanyForm({
      name: str(c.name),
      eik: str(c.eik),
      vat_number: str(c.vat_number),
      address: str(c.address),
      city: str(c.city),
      post_code: str(c.post_code),
      country: str(c.country, "BG"),
      phone: str(c.phone),
      email: str(c.email),
      website: str(c.website),
      mol: str(c.mol),
      manager_name: str(c.manager_name),
      manager_eik: str(c.manager_eik),
      manager_egn: str(c.manager_egn),
      accountant_name: str(c.accountant_name),
      accountant_egn: str(c.accountant_egn),
      authorized_person_name: str(c.authorized_person_name),
      authorized_person_egn: str(c.authorized_person_egn),
      tax_authority: str(c.tax_authority),
      nap_office: str(c.nap_office),
      iban: str(c.iban),
      bic: str(c.bic),
      is_vat_registered: Number(c.is_vat_registered) ? "1" : "0",
      vat_period: str(c.vat_period, "monthly"),
      currency: str(c.currency, "EUR"),
      vat_branch_number: str(c.vat_branch_number),
      fiscal_year_start_month: str(c.fiscal_year_start_month, "1"),
    });
    setSaftForm({
      street_name: str(c.street_name),
      building_number: str(c.building_number),
      address_building: str(c.address_building),
      additional_address_detail: str(c.additional_address_detail),
      address_type: str(c.address_type),
      region: str(c.region),
      tax_accounting_basis: str(c.tax_accounting_basis, "A"),
      inventory_valuation_method: str(c.inventory_valuation_method, "WAC"),
      is_part_of_group: str(c.is_part_of_group),
      tax_entity: str(c.tax_entity),
      software_company_name: str(c.software_company_name, "bagabuch"),
      software_id: str(c.software_id, "BAGABUCH"),
      software_version: str(c.software_version, "1.0"),
    });
    const raw = str(c.settings, "{}");
    setRawSettings(raw);
    setKeys(parseKeys(raw));
    setLocations(pack.locations ?? []);
    setOwners(pack.beneficial_owners ?? []);
    setParents(pack.ultimate_parents ?? []);
    setSeries(pack.document_series ?? []);
  };

  const load = useCallback(async () => {
    setLoading(true);
    setErr("");
    try {
      const ac = await getActiveCompany();
      const id = ac && typeof ac.id === "number" ? ac.id : 0;
      setActiveId(id);
      if (id <= 0) {
        setLoading(false);
        return;
      }
      const pack = await api.get<SettingsPack>(`/v1/companies/${id}/settings`);
      applyPack(pack);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
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

  const flash = (ok: string) => {
    setMsg(ok);
    setErr("");
  };

  const saveCompany = async (e: FormEvent) => {
    e.preventDefault();
    if (activeId <= 0) return;
    setSaving(true);
    setErr("");
    try {
      let settingsObj: Record<string, unknown> = {};
      try {
        const parsed = JSON.parse(rawSettings || "{}") as unknown;
        if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
          settingsObj = parsed as Record<string, unknown>;
        }
      } catch {
        settingsObj = {};
      }
      settingsObj.mistral_api_key = keys.mistral_api_key;
      settingsObj.zhipu_api_key = keys.zhipu_api_key;
      await api.patch(`/v1/companies/${activeId}`, {
        name: companyForm.name,
        eik: companyForm.eik,
        vat_number: companyForm.vat_number,
        address: companyForm.address,
        city: companyForm.city,
        post_code: companyForm.post_code,
        country: companyForm.country,
        phone: companyForm.phone,
        email: companyForm.email,
        website: companyForm.website,
        mol: companyForm.mol,
        manager_name: companyForm.manager_name,
        manager_eik: companyForm.manager_eik,
        manager_egn: companyForm.manager_egn,
        accountant_name: companyForm.accountant_name,
        accountant_egn: companyForm.accountant_egn,
        authorized_person_name: companyForm.authorized_person_name,
        authorized_person_egn: companyForm.authorized_person_egn,
        tax_authority: companyForm.tax_authority,
        nap_office: companyForm.nap_office,
        iban: companyForm.iban,
        bic: companyForm.bic,
        is_vat_registered: companyForm.is_vat_registered === "1",
        vat_period: companyForm.vat_period,
        currency: companyForm.currency,
        vat_branch_number: companyForm.vat_branch_number,
        fiscal_year_start_month: Number(companyForm.fiscal_year_start_month) || 1,
        settings: JSON.stringify(settingsObj),
      });
      flash(t("settings.saved.company"));
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const splitAddress = async () => {
    const address = (companyForm.address ?? "").trim();
    if (!address) {
      setErr(t("settings.split_address.empty"));
      setSplitNote("");
      return;
    }
    setSplitting(true);
    setErr("");
    setSplitNote("");
    try {
      const data = await api.post<{
        street_name: string;
        building_number: string;
        address_building: string;
        additional_address_detail: string;
        city: string;
        post_code: string;
        region: string;
        country: string;
        address_type: string;
        parse_ok: boolean;
        parse_note: string;
      }>("/v1/addresses/parse", {
        address,
        country: companyForm.country || "BG",
        city: companyForm.city,
        post_code: companyForm.post_code,
      });
      if (!data.parse_ok) {
        setErr(data.parse_note || t("settings.split_address.empty"));
        return;
      }
      const fill = (prev: Form, key: string, value: string) =>
        value ? { ...prev, [key]: value } : prev;
      setSaftForm((prev) => {
        let next = fill(prev, "street_name", data.street_name);
        next = fill(next, "building_number", data.building_number);
        next = fill(next, "address_building", data.address_building);
        next = fill(next, "additional_address_detail", data.additional_address_detail);
        next = fill(next, "region", data.region);
        next = fill(next, "address_type", data.address_type);
        return next;
      });
      setCompanyForm((prev) => {
        let next = prev;
        if (!prev.city && data.city) next = { ...next, city: data.city };
        if (!prev.post_code && data.post_code) next = { ...next, post_code: data.post_code };
        if (!prev.country && data.country) next = { ...next, country: data.country };
        return next;
      });
      setSplitNote(data.parse_note ? `${data.parse_note} ${t("settings.split_address.save")}` : t("settings.split_address.save"));
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSplitting(false);
    }
  };

  const saveSaft = async (e: FormEvent) => {
    e.preventDefault();
    if (activeId <= 0) return;
    setSaving(true);
    setErr("");
    try {
      await api.patch(`/v1/companies/${activeId}`, {
        street_name: saftForm.street_name,
        building_number: saftForm.building_number,
        address_building: saftForm.address_building,
        additional_address_detail: saftForm.additional_address_detail,
        address_type: saftForm.address_type,
        region: saftForm.region,
        tax_accounting_basis: saftForm.tax_accounting_basis,
        inventory_valuation_method: saftForm.inventory_valuation_method,
        is_part_of_group: saftForm.is_part_of_group,
        tax_entity: saftForm.tax_entity,
        software_company_name: saftForm.software_company_name,
        software_id: saftForm.software_id,
        software_version: saftForm.software_version,
      });
      flash(t("settings.saved.saft"));
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const reloadRelated = async () => {
    const pack = await api.get<SettingsPack>(`/v1/companies/${activeId}/settings`);
    setLocations(pack.locations ?? []);
    setOwners(pack.beneficial_owners ?? []);
    setParents(pack.ultimate_parents ?? []);
    setSeries(pack.document_series ?? []);
  };

  const saveLoc = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setErr("");
    try {
      const body = {
        name: locForm.name,
        location_type: locForm.location_type,
        street_name: locForm.street_name,
        building_number: locForm.building_number,
        city: locForm.city,
        post_code: locForm.post_code,
        region: locForm.region,
        country: locForm.country,
        is_main: locForm.is_main === "1",
      };
      if (editLocId > 0) {
        await api.patch(`/v1/company-locations/${editLocId}`, body);
      } else {
        await api.post(`/v1/companies/${activeId}/locations`, body);
      }
      setShowLoc(false);
      setEditLocId(0);
      await reloadRelated();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const saveOwner = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setErr("");
    try {
      const body = {
        first_name_bg: ownerForm.first_name_bg,
        last_name_bg: ownerForm.last_name_bg,
        egn: ownerForm.egn,
        first_name_latin: ownerForm.first_name_latin,
        last_name_latin: ownerForm.last_name_latin,
        country: ownerForm.country,
        ownership_percentage: ownerForm.ownership_percentage,
      };
      if (editOwnerId > 0) {
        await api.patch(`/v1/beneficial-owners/${editOwnerId}`, body);
      } else {
        await api.post(`/v1/companies/${activeId}/beneficial-owners`, body);
      }
      setShowOwner(false);
      setEditOwnerId(0);
      await reloadRelated();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const saveParent = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setErr("");
    try {
      const body = {
        name_bg: parentForm.name_bg,
        uic: parentForm.uic,
        name_latin: parentForm.name_latin,
        country: parentForm.country,
      };
      if (editParentId > 0) {
        await api.patch(`/v1/ultimate-parents/${editParentId}`, body);
      } else {
        await api.post(`/v1/companies/${activeId}/ultimate-parents`, body);
      }
      setShowParent(false);
      setEditParentId(0);
      await reloadRelated();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const delLoc = async (id: number) => {
    if (!confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/company-locations/${id}`);
      await reloadRelated();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  const delOwner = async (id: number) => {
    if (!confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/beneficial-owners/${id}`);
      await reloadRelated();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  const delParent = async (id: number) => {
    if (!confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/ultimate-parents/${id}`);
      await reloadRelated();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  if (loading) {
    return <div className="muted">{t("common.loading")}</div>;
  }
  if (activeId <= 0) {
    return (
      <div className="card card-pad">
        <p className="muted" style={{ margin: 0 }}>
          {t("settings.no_company")}
        </p>
      </div>
    );
  }

  return (
    <>
      <h1 className="page-title">{t("settings.title")}</h1>
      {msg && (
        <div className="flash-ok">
          {msg}
          <button className="btn-ghost btn-sm" onClick={() => setMsg("")} type="button">
            ×
          </button>
        </div>
      )}
      {err && (
        <div className="flash-err">
          {err}
          <button className="btn-ghost btn-sm" onClick={() => setErr("")} type="button">
            ×
          </button>
        </div>
      )}
      <div className="tabs">
        <button
          type="button"
          className={`tab${tab === 0 ? " tab-active" : ""}`}
          onClick={() => setTab(0)}
        >
          {t("settings.tab.company")}
        </button>
        <button
          type="button"
          className={`tab${tab === 1 ? " tab-active" : ""}`}
          onClick={() => setTab(1)}
        >
          {t("settings.tab.saft")}
        </button>
        <button
          type="button"
          className={`tab${tab === 2 ? " tab-active" : ""}`}
          onClick={() => setTab(2)}
        >
          {t("settings.tab.series")}
        </button>
        <button
          type="button"
          className={`tab${tab === 3 ? " tab-active" : ""}`}
          onClick={() => setTab(3)}
        >
          {t("settings.tab.fs")}
        </button>
      </div>

      {tab === 0 && (
        <SettingsCompanyTab
          t={t}
          companyForm={companyForm}
          setC={setC}
          keys={keys}
          setKeys={setKeys}
          saving={saving}
          onSubmit={saveCompany}
        />
      )}

      {tab === 1 && (
        <SettingsSaftTab
          t={t}
          saftForm={saftForm}
          setS={setS}
          saving={saving}
          splitting={splitting}
          splitNote={splitNote}
          onSplitAddress={splitAddress}
          onSaveSaft={saveSaft}
          locations={locations}
          showLoc={showLoc}
          setShowLoc={setShowLoc}
          editLocId={editLocId}
          setEditLocId={setEditLocId}
          locForm={locForm}
          setLocForm={setLocForm}
          setL={setL}
          emptyLoc={EMPTY_LOC}
          saveLoc={saveLoc}
          delLoc={delLoc}
          owners={owners}
          showOwner={showOwner}
          setShowOwner={setShowOwner}
          editOwnerId={editOwnerId}
          setEditOwnerId={setEditOwnerId}
          ownerForm={ownerForm}
          setOwnerForm={setOwnerForm}
          setO={setO}
          emptyOwner={EMPTY_OWNER}
          saveOwner={saveOwner}
          delOwner={delOwner}
          parents={parents}
          showParent={showParent}
          setShowParent={setShowParent}
          editParentId={editParentId}
          setEditParentId={setEditParentId}
          parentForm={parentForm}
          setParentForm={setParentForm}
          setP={setP}
          emptyParent={EMPTY_PARENT}
          saveParent={saveParent}
          delParent={delParent}
        />
      )}

      {tab === 2 && (
        <div className="card card-pad" style={{ maxWidth: 760 }}>
          <DocumentSeriesTab
            companyId={activeId}
            series={series}
            onChange={reloadRelated}
            onError={setErr}
          />
        </div>
      )}

      {tab === 3 && <FsFormulasTab />}
    </>
  );
}

export default function SettingsPage() {
  return (
    <RequireAuth>
      <SettingsInner />
    </RequireAuth>
  );
}
