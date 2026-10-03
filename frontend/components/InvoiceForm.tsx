"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ListResponse, getActiveCompany } from "../lib/api";
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

  const [counterpartId, setCounterpartId] = useState("");
  const [counterparts, setCounterparts] = useState<Counterpart[]>([]);
  const [loadedParty, setLoadedParty] = useState<Counterpart | null>(null);
  const [cpQuery, setCpQuery] = useState("");
  const [cpOpen, setCpOpen] = useState(false);

  const [products, setProducts] = useState<Product[]>([]);
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
      const [cp, pr, inv, un, ex, co] = await Promise.all([
        api.get<ListResponse<Counterpart>>("/v1/counterparts"),
        api.get<ListResponse<Product>>("/v1/products"),
        api.get<ListResponse<Invoice>>("/v1/invoices"),
        api.get<ListResponse<UnitOfMeasure>>("/v1/units"),
        api.get<ListResponse<VatExemption>>("/v1/vat-exemptions"),
        getActiveCompany(),
      ]);
      setCounterparts(cp.items ?? []);
      setProducts(pr.items ?? []);
      setInvoices(inv.items ?? []);
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
      if (dir !== "out") {
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

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError("");
    try {
      if (!counterpartId) {
        setFormError(t("invoices.pick_counterpart"));
        setSaving(false);
        return;
      }
      if (direction === "in" && !number.trim()) {
        setFormError(t("invoices.number_in_hint"));
        setSaving(false);
        return;
      }
      if (docTypeRequiresOriginal(documentType) && !originalInvoiceId) {
        setFormError(t("invoices.need_original"));
        setSaving(false);
        return;
      }
      if (hasZeroVat && !vatExemption) {
        setFormError(t("invoices.vat_exemption_pick"));
        setSaving(false);
        return;
      }
      const computed = pricedLines.map((l) =>
        calcLine(l, pricesIncludeVat, isCredit)
      );
      const payloadLines = applyDiscountToLines(computed, discountPercent).map(
        (l) => ({
          product_id: l.product_id,
          description: l.description,
          quantity: l.quantity,
          unit: l.unit,
          unit_price: round2(num(l.unit_price)),
          vat_rate: l.vat_rate,
          net_amount: l.net_amount,
          vat_amount: l.vat_amount,
          total_amount: l.total_amount,
        })
      );
      const payload = {
        direction,
        document_type: documentType,
        number,
        issue_date: issueDate,
        tax_event_date: taxEventDate || issueDate,
        due_date: dueDate,
        accounting_month: issueDate.slice(0, 7),
        counterpart_id: Number(counterpartId),
        currency,
        currency_rate: currency === "EUR" ? "1" : currencyRate,
        payment_method: paymentMethod,
        notes,
        discount_percent: discountPercent || "0",
        discount_amount: totals.discount,
        vat_exemption_reason: vatExemption,
        original_invoice_id: originalInvoiceId ? Number(originalInvoiceId) : 0,
        location_id: Number(locationId) || 0,
        lines: payloadLines,
      };
      if (mode === "edit" && invoiceId) {
        await api.patch(`/v1/invoices/${invoiceId}`, payload);
        router.push(`/invoices/${invoiceId}`);
      } else {
        const created = await api.post<Invoice>("/v1/invoices", payload);
        router.push(`/invoices/${created.id}`);
      }
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
          {hasZeroVat && (
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
      <div className="form-actions">
        <button type="button" className="btn" onClick={() => router.push("/invoices")}>
          {t("common.cancel")}
        </button>
        <button className="btn btn-primary" disabled={saving}>
          {t("common.save")}
        </button>
      </div>

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
      />
    </form>
  );
}
