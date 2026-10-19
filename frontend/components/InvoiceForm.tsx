"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { api, fetchAll, ListResponse, getActiveCompany } from "../lib/api";
import {
  Invoice,
  InvoiceLine,
  PAY_METHODS,
  applyDiscountToLines,
  calcLine,
  calcTotals,
  docTypeIsCredit,
  docTypeRequiresOriginal,
  docTypesFor,
  emptyLine,
  normVatRate,
  num,
  round2,
  todayIso,
  toDateInput,
} from "../lib/invoice";
import { useI18n } from "./I18nProvider";
import { UnitOfMeasure } from "../lib/units";
import { VatExemption, filterVatex, vatexLabel } from "../lib/vatExemptions";
import {
  CompanyLocation,
  fetchCompanyLocations,
  mainLocationId,
} from "../lib/locations";
import { InvoiceFormHeader } from "./InvoiceFormHeader";
import { InvoiceFormLines } from "./InvoiceFormLines";
import { InvoiceFormPickers } from "./InvoiceFormPickers";
import {
  InvoiceFormAccount as Account,
  InvoiceFormCounterpart as Counterpart,
  InvoiceFormProduct as Product,
} from "./invoiceFormTypes";

interface Props {
  mode: "create" | "edit";
  invoiceId?: number;
}

export function InvoiceForm({ mode, invoiceId }: Props) {
  const { t, lang } = useI18n();
  const router = useRouter();
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState("");
  const [loading, setLoading] = useState(mode === "edit");

  const [direction, setDirection] = useState("out");
  const [documentType, setDocumentType] = useState<string>("01");
  const [number, setNumber] = useState("");
  const [issueDate, setIssueDate] = useState(todayIso());
  const [taxEventDate, setTaxEventDate] = useState(todayIso());
  const [dueDate, setDueDate] = useState("");
  const [currency, setCurrency] = useState("EUR");
  const [currencyRate, setCurrencyRate] = useState("1");
  const [paymentMethod, setPaymentMethod] = useState("банков превод");
  const [notes, setNotes] = useState("");
  const [discountPercent, setDiscountPercent] = useState("0");
  const [vatExemption, setVatExemption] = useState("");
  const [originalInvoiceId, setOriginalInvoiceId] = useState("");
  const [pricesIncludeVat, setPricesIncludeVat] = useState(false);
  const [excludeVat, setExcludeVat] = useState(false);
  const [vatArticle, setVatArticle] = useState("чл. 117");
  const [customsStatistical, setCustomsStatistical] = useState("");
  const [customsDuty, setCustomsDuty] = useState("");
  const [customsVat, setCustomsVat] = useState("");
  const [protoModal, setProtoModal] = useState(false);
  const [protoArticle, setProtoArticle] = useState("чл. 117");
  const [protoRate, setProtoRate] = useState("20");
  const [protoDate, setProtoDate] = useState(todayIso());
  const [protoNumber, setProtoNumber] = useState("");
  const [customsModal, setCustomsModal] = useState(false);
  const [mrnNumber, setMrnNumber] = useState("");
  const [mrnDate, setMrnDate] = useState(todayIso());
  const [mrnStat, setMrnStat] = useState("");
  const [mrnDuty, setMrnDuty] = useState("0.00");
  const [mrnVat, setMrnVat] = useState("");

  const [counterpartId, setCounterpartId] = useState("");
  const [counterparts, setCounterparts] = useState<Counterpart[]>([]);
  const [loadedParty, setLoadedParty] = useState<Counterpart | null>(null);
  const [cpQuery, setCpQuery] = useState("");
  const [cpOpen, setCpOpen] = useState(false);

  const [products, setProducts] = useState<Product[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [units, setUnits] = useState<UnitOfMeasure[]>([]);
  const [exemptions, setExemptions] = useState<VatExemption[]>([]);
  const [exQ, setExQ] = useState("");
  const [vatRegistered, setVatRegistered] = useState(true);
  const [invoices, setInvoices] = useState<Invoice[]>([]);
  const [itemOpenFor, setItemOpenFor] = useState<number | null>(null);
  const [itemQuery, setItemQuery] = useState("");

  const [lines, setLines] = useState<InvoiceLine[]>([emptyLine()]);
  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [locationId, setLocationId] = useState("0");

  const loadLookups = useCallback(async () => {
    try {
      const [cp, pr, acc, inv, un, ex, co] = await Promise.all([
        fetchAll<Counterpart>("/v1/counterparts"),
        fetchAll<Product>("/v1/products"),
        fetchAll<Account>("/v1/accounts"),
        fetchAll<Invoice>("/v1/invoices"),
        api.get<ListResponse<UnitOfMeasure>>("/v1/units"),
        api.get<ListResponse<VatExemption>>("/v1/vat-exemptions"),
        getActiveCompany(),
      ]);
      setCounterparts(cp);
      setProducts(pr);
      setAccounts(acc);
      setInvoices(inv);
      setUnits(un.items ?? []);
      setExemptions(ex.items ?? []);
      setVatRegistered(Number((co as { is_vat_registered?: number }).is_vat_registered ?? 1) === 1);
      const locs = await fetchCompanyLocations();
      setLocations(locs);
      if (mode === "create" && locs.length > 0) {
        setLocationId(String(mainLocationId(locs)));
      }
    } catch {
      /* lookup failure is non-fatal */
    }
  }, [mode]);

  const loadNumber = useCallback(
    async (dt: string, dir: string) => {
      if (dir !== "out" && dt !== "09") {
        setNumber("");
        return;
      }
      try {
        const data = await api.get<{ number: string }>(
          `/v1/invoices/next-number?document_type=${encodeURIComponent(dt)}&direction=${encodeURIComponent(dir)}`
        );
        setNumber(data.number);
      } catch {
        /* keep current */
      }
    },
    []
  );

  useEffect(() => {
    loadLookups();
  }, [loadLookups]);

  useEffect(() => {
    if (!docTypesFor(direction).includes(documentType)) {
      setDocumentType("01");
    }
  }, [direction, documentType]);

  useEffect(() => {
    if (mode === "create") loadNumber(documentType, direction);
  }, [mode, documentType, direction, loadNumber]);

  useEffect(() => {
    if (mode !== "edit") return;
    if (!invoiceId || !Number.isFinite(invoiceId) || invoiceId <= 0) {
      setFormError(t("common.error"));
      setLoading(false);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const inv = await api.get<Invoice>(`/v1/invoices/${invoiceId}`);
        if (cancelled) return;
        setDirection(inv.direction || "out");
        setDocumentType(inv.document_type || "01");
        setNumber(inv.number || "");
        setIssueDate(toDateInput(inv.issue_date) || todayIso());
        setTaxEventDate(toDateInput(inv.tax_event_date || inv.issue_date) || todayIso());
        setDueDate(toDateInput(inv.due_date || ""));
        setCurrency(inv.currency || "EUR");
        setCurrencyRate(inv.currency_rate || "1");
        setPaymentMethod(inv.payment_method || "");
        setLocationId(String(inv.location_id || 0));
        setNotes(inv.notes || "");
        setDiscountPercent(inv.discount_percent || "0");
        setVatExemption(inv.vat_exemption_reason || "");
        setOriginalInvoiceId(
          inv.original_invoice_id ? String(inv.original_invoice_id) : ""
        );
        setExcludeVat(Number(inv.exclude_vat || 0) === 1);
        setVatArticle(inv.vat_article || "чл. 117");
        setCustomsStatistical(inv.customs_statistical || "");
        setCustomsDuty(inv.customs_duty || "");
        setCustomsVat(inv.customs_vat || "");
        setCounterpartId(inv.counterpart_id ? String(inv.counterpart_id) : "");
        const party = inv.counterpart;
        if (party && (party.id || inv.counterpart_id)) {
          setLoadedParty({
            id: party.id || inv.counterpart_id,
            name: party.name || inv.counterpart_name || "",
            eik: party.eik || "",
            vat_number: party.vat_number || "",
            address: party.address || "",
            city: party.city || "",
          });
        } else if (inv.counterpart_id) {
          setLoadedParty({
            id: inv.counterpart_id,
            name: inv.counterpart_name || "",
            eik: "",
            vat_number: "",
            address: "",
            city: "",
          });
        }
        const ls = (inv.lines ?? []).map((l) => ({
          ...emptyLine(),
          ...l,
          code: l.code || "",
          description: l.description || "",
          quantity: l.quantity || "1",
          unit: l.unit || "C62",
          unit_price: l.unit_price || "0",
          vat_rate: normVatRate(l.vat_rate),
        }));
        setLines(ls.length ? ls : [emptyLine()]);
      } catch (e) {
        if (!cancelled) setFormError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [mode, invoiceId, t]);

  useEffect(() => {
    if (products.length === 0) return;
    setLines((prev) => {
      let changed = false;
      const next = prev.map((l) => {
        if (l.code || !l.product_id) return l;
        const p = products.find((x) => x.id === l.product_id);
        if (!p?.code) return l;
        changed = true;
        return { ...l, code: p.code };
      });
      return changed ? next : prev;
    });
  }, [products]);

  useEffect(() => {
    if (!loadedParty) return;
    setCounterparts((prev) => {
      if (prev.some((c) => c.id === loadedParty.id)) return prev;
      return [loadedParty, ...prev];
    });
  }, [loadedParty]);

  const selectedCp =
    counterparts.find((c) => String(c.id) === counterpartId) ??
    (loadedParty && String(loadedParty.id) === counterpartId ? loadedParty : undefined);

  const isCredit = docTypeIsCredit(documentType);
  const pricedLines = useMemo(
    () => lines.map((l) => calcLine(l, pricesIncludeVat, isCredit)),
    [lines, pricesIncludeVat, isCredit]
  );
  const totals = useMemo(
    () => calcTotals(pricedLines, discountPercent),
    [pricedLines, discountPercent]
  );
  const hasZeroVat = pricedLines.some((l) => num(l.vat_rate) === 0);

  useEffect(() => {
    if (hasZeroVat && !vatRegistered && !vatExemption) {
      setVatExemption("VATEX-EU-SM");
    }
  }, [hasZeroVat, vatRegistered, vatExemption]);

  const filteredCp = counterparts.filter((c) => {
    const q = cpQuery.toLowerCase();
    if (!q) return true;
    return (
      c.name.toLowerCase().includes(q) ||
      (c.eik || "").includes(q) ||
      (c.vat_number || "").toLowerCase().includes(q)
    );
  });

  const filteredProducts = products.filter((p) => {
    const q = itemQuery.toLowerCase();
    if (!q) return true;
    return (
      p.name.toLowerCase().includes(q) ||
      (p.code || "").toLowerCase().includes(q)
    );
  });

  const setLine = (i: number, patch: Partial<InvoiceLine>) =>
    setLines((prev) => prev.map((l, idx) => (idx === i ? { ...l, ...patch } : l)));

  const pickProduct = (i: number, p: Product) => {
    setLine(i, {
      product_id: p.id,
      code: p.code || "",
      description: p.name,
      unit: p.unit || "C62",
      unit_price: p.price || "0",
      vat_rate: p.vat_rate || "20",
    });
    setItemOpenFor(null);
    setItemQuery("");
  };

  const saveUnderlyingInvoice = async (): Promise<number | null> => {
    setFormError("");
    if (!counterpartId && documentType !== "07") {
      setFormError(t("invoices.pick_counterpart"));
      return null;
    }
    if (direction === "in" && documentType !== "09" && !number.trim()) {
      setFormError(t("invoices.number_in_hint"));
      return null;
    }
    if (docTypeRequiresOriginal(documentType) && !originalInvoiceId) {
      setFormError(t("invoices.need_original"));
      return null;
    }
    if (hasZeroVat && !excludeVat && documentType !== "07" && documentType !== "09" && !vatExemption) {
      setFormError(t("invoices.vat_exemption_pick"));
      return null;
    }
    const computed = pricedLines.map((l) => calcLine(l, pricesIncludeVat, isCredit));
    const payloadLines = applyDiscountToLines(computed, discountPercent).map((l) => ({
      product_id: l.product_id,
      description: l.description,
      quantity: l.quantity,
      unit: l.unit,
      unit_price: round2(num(l.unit_price)),
      vat_rate: l.vat_rate,
      net_amount: l.net_amount,
      vat_amount: l.vat_amount,
      total_amount: l.total_amount,
    }));
    const payload = {
      direction,
      document_type: documentType,
      number,
      issue_date: issueDate,
      tax_event_date: taxEventDate || issueDate,
      due_date: dueDate,
      accounting_month: issueDate.slice(0, 7),
      counterpart_id: counterpartId ? Number(counterpartId) : 0,
      currency,
      currency_rate: currency === "EUR" ? "1" : currencyRate,
      payment_method: paymentMethod,
      notes,
      discount_percent: discountPercent || "0",
      discount_amount: totals.discount,
      vat_exemption_reason: vatExemption,
      original_invoice_id: originalInvoiceId ? Number(originalInvoiceId) : 0,
      location_id: Number(locationId) || 0,
      exclude_vat:
        direction === "in" && documentType !== "07" && documentType !== "09" && excludeVat
          ? 1
          : 0,
      vat_article: documentType === "09" ? vatArticle : "",
      customs_statistical: documentType === "07" ? customsStatistical : "",
      customs_duty: documentType === "07" ? customsDuty : "",
      customs_vat: documentType === "07" ? customsVat : "",
      lines: payloadLines,
    };
    if (mode === "edit" && invoiceId) {
      await api.patch(`/v1/invoices/${invoiceId}`, payload);
      return invoiceId;
    }
    const created = await api.post<Invoice>("/v1/invoices", payload);
    return created.id;
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      const id = await saveUnderlyingInvoice();
      if (id) router.push(`/invoices/${id}`);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleIssueProtocol = async () => {
    setSaving(true);
    try {
      const id = await saveUnderlyingInvoice();
      if (!id) return;
      const proto = await api.post<Invoice>(`/v1/invoices/${id}/issue-protocol`, {
        vat_article: protoArticle,
        vat_rate: protoRate,
        issue_date: protoDate || issueDate,
        number: protoNumber,
      });
      setProtoModal(false);
      router.push(`/invoices/${proto.id}`);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleIssueCustoms = async () => {
    setSaving(true);
    try {
      const id = await saveUnderlyingInvoice();
      if (!id) return;
      const cust = await api.post<Invoice>(`/v1/invoices/${id}/issue-customs`, {
        number: mrnNumber,
        issue_date: mrnDate || issueDate,
        customs_statistical: mrnStat || totals.taxable,
        customs_duty: mrnDuty || "0.00",
        customs_vat: mrnVat,
      });
      setCustomsModal(false);
      router.push(`/invoices/${cust.id}`);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return <div className="muted">{t("common.loading")}</div>;
  }

  return (
    <form className="invoice-form" onSubmit={handleSave}>
      <InvoiceFormHeader
        t={t}
        direction={direction}
        setDirection={setDirection}
        documentType={documentType}
        setDocumentType={setDocumentType}
        isCredit={isCredit}
        locations={locations}
        locationId={locationId}
        setLocationId={setLocationId}
        number={number}
        setNumber={setNumber}
        issueDate={issueDate}
        setIssueDate={setIssueDate}
        taxEventDate={taxEventDate}
        setTaxEventDate={setTaxEventDate}
        dueDate={dueDate}
        setDueDate={setDueDate}
        currency={currency}
        setCurrency={setCurrency}
        currencyRate={currencyRate}
        setCurrencyRate={setCurrencyRate}
        originalInvoiceId={originalInvoiceId}
        setOriginalInvoiceId={setOriginalInvoiceId}
        invoices={invoices}
        selectedCp={selectedCp}
        setCpOpen={setCpOpen}
        setFormError={setFormError}
        excludeVat={excludeVat}
        setExcludeVat={setExcludeVat}
        vatArticle={vatArticle}
        setVatArticle={setVatArticle}
        customsStatistical={customsStatistical}
        setCustomsStatistical={setCustomsStatistical}
        customsDuty={customsDuty}
        setCustomsDuty={setCustomsDuty}
        customsVat={customsVat}
        setCustomsVat={setCustomsVat}
      />
      <InvoiceFormLines
        t={t}
        pricedLines={pricedLines}
        pricesIncludeVat={pricesIncludeVat}
        setPricesIncludeVat={setPricesIncludeVat}
        setLine={setLine}
        setLines={setLines}
        setItemOpenFor={setItemOpenFor}
        setItemQuery={setItemQuery}
        units={units}
      />
      <section className="card invoice-section">
        <h2 className="invoice-section-title">{t("invoices.totals_section")}</h2>
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("invoices.payment_method")}</label>
            <select
              className="select"
              value={paymentMethod}
              onChange={(e) => setPaymentMethod(e.target.value)}
            >
              {PAY_METHODS.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
              {paymentMethod && !PAY_METHODS.includes(paymentMethod) && (
                <option value={paymentMethod}>{paymentMethod}</option>
              )}
            </select>
          </div>
          <div className="field">
            <label className="label">{t("invoices.discount_percent")}</label>
            <input
              className="input"
              value={discountPercent}
              onChange={(e) => setDiscountPercent(e.target.value)}
            />
          </div>
          <div className="field" style={{ gridColumn: "1 / -1" }}>
            <label className="label">{t("invoices.notes")}</label>
            <textarea
              className="textarea"
              rows={2}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
          </div>
          {hasZeroVat && !excludeVat && documentType !== "07" && documentType !== "09" && (
            <div className="field" style={{ gridColumn: "1 / -1" }}>
              <label className="label">{t("invoices.vat_exemption")} *</label>
              <input
                className="input"
                value={exQ}
                onChange={(e) => setExQ(e.target.value)}
                placeholder={t("invoices.vat_exemption_search")}
              />
              <select
                className="select"
                value={vatExemption}
                onChange={(e) => setVatExemption(e.target.value)}
                required
                style={{ marginTop: 6 }}
                size={6}
              >
                <option value="">{t("invoices.vat_exemption_pick")}</option>
                {filterVatex(exemptions, exQ).map((e) => (
                  <option key={e.code} value={e.code}>
                    {vatexLabel(e, lang)}
                  </option>
                ))}
                {vatExemption &&
                  !exemptions.some((e) => e.code === vatExemption) && (
                    <option value={vatExemption}>{vatExemption}</option>
                  )}
              </select>
            </div>
          )}
        </div>
        <div className="totals-box">
          <div>
            <span>{t("invoices.subtotal")}</span>
            <b>
              {totals.subtotal} {currency}
            </b>
          </div>
          <div>
            <span>{t("invoices.discount")}</span>
            <b>
              {totals.discount} {currency}
            </b>
          </div>
          <div>
            <span>{t("invoices.taxable")}</span>
            <b>
              {totals.taxable} {currency}
            </b>
          </div>
          <div>
            <span>{t("invoices.vat")}</span>
            <b>
              {totals.vat} {currency}
            </b>
          </div>
          <div className="totals-grand">
            <span>{t("invoices.amount_due")}</span>
            <b>
              {totals.total} {currency}
            </b>
          </div>
        </div>
      </section>

      {formError && <div className="error-text">{formError}</div>}
      <div className="form-actions" style={{ flexWrap: "wrap", gap: 8 }}>
        <button type="button" className="btn" onClick={() => router.push("/invoices")}>
          {t("common.cancel")}
        </button>
        {direction === "in" && documentType !== "07" && documentType !== "09" && excludeVat && (
          <>
            <button
              type="button"
              className="btn"
              disabled={saving}
              onClick={async () => {
                setProtoDate(issueDate);
                try {
                  const r = await api.get<{ number: string }>(
                    "/v1/invoices/next-number?document_type=09&direction=in"
                  );
                  setProtoNumber(r.number);
                } catch {
                  setProtoNumber("");
                }
                setProtoModal(true);
              }}
            >
              {t("invoices.issue_protocol")}
            </button>
            <button
              type="button"
              className="btn"
              disabled={saving}
              onClick={() => {
                setMrnDate(issueDate);
                const st = totals.taxable;
                setMrnStat(st);
                setMrnDuty("0.00");
                setMrnVat(round2(num(st) * 0.2));
                setCustomsModal(true);
              }}
            >
              {t("invoices.issue_customs")}
            </button>
          </>
        )}
        <button className="btn btn-primary" disabled={saving}>
          {t("common.save")}
        </button>
      </div>

      {protoModal && (
        <div className="modal-backdrop" onClick={() => setProtoModal(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 520 }}>
            <div className="modal-head">
              <h3>{t("invoices.protocol_modal_title")}</h3>
              <button type="button" className="btn-ghost" onClick={() => setProtoModal(false)}>
                ✕
              </button>
            </div>
            <div className="content">
              <p className="muted" style={{ marginBottom: 12 }}>
                {t("invoices.protocol_hint")}
              </p>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("invoices.number")}</label>
                  <input
                    className="input"
                    value={protoNumber}
                    onChange={(e) => setProtoNumber(e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("invoices.issue_date")}</label>
                  <input
                    className="input"
                    type="date"
                    value={protoDate}
                    onChange={(e) => setProtoDate(e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("invoices.vat_article")}</label>
                  <select
                    className="select"
                    value={protoArticle}
                    onChange={(e) => setProtoArticle(e.target.value)}
                  >
                    <option value="чл. 117">чл. 117 от ЗДДС (ВОП / чл. 82, ал. 2)</option>
                    <option value="чл. 163а">чл. 163а от ЗДДС (обратно начисляване)</option>
                    <option value="чл. 151в">чл. 151в от ЗДДС</option>
                  </select>
                </div>
                <div className="field">
                  <label className="label">{t("invoices.vat_rate_protocol")}</label>
                  <select
                    className="select"
                    value={protoRate}
                    onChange={(e) => setProtoRate(e.target.value)}
                  >
                    <option value="20">20%</option>
                    <option value="9">9%</option>
                  </select>
                </div>
              </div>
              <div className="form-actions" style={{ marginTop: 16 }}>
                <button type="button" className="btn" onClick={() => setProtoModal(false)}>
                  {t("common.cancel")}
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={saving}
                  onClick={() => void handleIssueProtocol()}
                >
                  {t("invoices.issue_protocol")}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {customsModal && (
        <div className="modal-backdrop" onClick={() => setCustomsModal(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 560 }}>
            <div className="modal-head">
              <h3>{t("invoices.customs_modal_title")}</h3>
              <button type="button" className="btn-ghost" onClick={() => setCustomsModal(false)}>
                ✕
              </button>
            </div>
            <div className="content">
              <p className="muted" style={{ marginBottom: 12 }}>
                {t("invoices.customs_hint")}
              </p>
              <div className="form-grid">
                <div className="field">
                  <label className="label">{t("invoices.customs_mrn")} *</label>
                  <input
                    className="input"
                    value={mrnNumber}
                    onChange={(e) => setMrnNumber(e.target.value)}
                    placeholder="26BG005800..."
                  />
                </div>
                <div className="field">
                  <label className="label">{t("invoices.issue_date")}</label>
                  <input
                    className="input"
                    type="date"
                    value={mrnDate}
                    onChange={(e) => setMrnDate(e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("invoices.customs_statistical")}</label>
                  <input
                    className="input"
                    value={mrnStat}
                    onChange={(e) => {
                      const st = e.target.value;
                      setMrnStat(st);
                      setMrnVat(round2((num(st) + num(mrnDuty)) * 0.2));
                    }}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("invoices.customs_duty")}</label>
                  <input
                    className="input"
                    value={mrnDuty}
                    onChange={(e) => {
                      const du = e.target.value;
                      setMrnDuty(du);
                      setMrnVat(round2((num(mrnStat) + num(du)) * 0.2));
                    }}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("invoices.customs_vat")}</label>
                  <input
                    className="input"
                    value={mrnVat}
                    onChange={(e) => setMrnVat(e.target.value)}
                  />
                </div>
                <div className="field">
                  <label className="label">{t("invoices.counterpart")}</label>
                  <input className="input" value={t("invoices.customs_agency")} disabled />
                </div>
              </div>
              <div className="form-actions" style={{ marginTop: 16 }}>
                <button type="button" className="btn" onClick={() => setCustomsModal(false)}>
                  {t("common.cancel")}
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={saving || !mrnNumber.trim()}
                  onClick={() => void handleIssueCustoms()}
                >
                  {t("invoices.issue_customs")}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      <InvoiceFormPickers
        t={t}
        cpOpen={cpOpen}
        setCpOpen={setCpOpen}
        cpQuery={cpQuery}
        setCpQuery={setCpQuery}
        filteredCp={filteredCp}
        setCounterpartId={setCounterpartId}
        itemOpenFor={itemOpenFor}
        setItemOpenFor={setItemOpenFor}
        itemQuery={itemQuery}
        setItemQuery={setItemQuery}
        filteredProducts={filteredProducts}
        pickProduct={pickProduct}
        units={units}
        accounts={accounts}
        onProductCreated={(lineIdx, created) => {
          setProducts((prev) => [...prev, created]);
          pickProduct(lineIdx, created);
        }}
        initialDescription={
          itemOpenFor !== null ? lines[itemOpenFor]?.description : ""
        }
      />
    </form>
  );
}
