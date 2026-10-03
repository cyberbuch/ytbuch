"use client";

import { useRouter } from "next/navigation";
import { ChangeEvent, DragEvent, useCallback, useEffect, useMemo, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { IconButton } from "../../components/IconButton";
import { api, ListResponse } from "../../lib/api";
import {
  Invoice,
  InvoiceLine,
  calcLine,
  calcTotals,
  docTypeIsCredit,
  docTypeRequiresOriginal,
  emptyLine,
  num,
  round2,
  todayIso,
} from "../../lib/invoice";
import { VatExemption } from "../../lib/vatExemptions";
import { ScanDraft } from "./ScanDraft";
import {
  Counterpart,
  Product,
  QueuedFile,
  ScanExtract,
  ScanLine,
  ViesLookup,
} from "./types";

const MAX_BYTES = 8 * 1024 * 1024;

function monthNow(): string {
  return todayIso().slice(0, 7);
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const s = String(reader.result ?? "");
      const comma = s.indexOf(",");
      resolve(comma >= 0 ? s.slice(comma + 1) : s);
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

function lineFromScan(s: ScanLine): InvoiceLine {
  const base: InvoiceLine = {
    ...emptyLine(),
    description: s.description || "",
    quantity: s.quantity || "1",
    unit: s.unit || "C62",
    unit_price: s.unit_price || "0",
    vat_rate: s.vat_rate || "20",
    product_id: s.product_id,
    seller_code: s.seller_code || "",
    keep_amounts: Boolean(s.net_amount && s.vat_amount && s.total_amount),
  };
  if (s.net_amount && s.vat_amount && s.total_amount) {
    return {
      ...base,
      net_amount: s.net_amount,
      vat_amount: s.vat_amount,
      total_amount: s.total_amount,
    };
  }
  return calcLine(base, false);
}

function isXmlFile(file: File): boolean {
  return file.name.toLowerCase().endsWith(".xml");
}

function ScanInner() {
  const { t } = useI18n();
  const router = useRouter();
  const [direction, setDirection] = useState<"in" | "out">("in");
  const [period, setPeriod] = useState(monthNow);
  const [queue, setQueue] = useState<QueuedFile[]>([]);
  const [dragging, setDragging] = useState(false);
  const [stage, setStage] = useState<0 | 1>(0);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [activeName, setActiveName] = useState("");

  const [number, setNumber] = useState("");
  const [issueDate, setIssueDate] = useState(todayIso());
  const [dueDate, setDueDate] = useState("");
  const [documentType, setDocumentType] = useState("01");
  const [currency, setCurrency] = useState("EUR");
  const [lines, setLines] = useState<InvoiceLine[]>([]);

  const [counterparts, setCounterparts] = useState<Counterpart[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [counterpartId, setCounterpartId] = useState("");
  const [ocrName, setOcrName] = useState("");
  const [ocrEik, setOcrEik] = useState("");
  const [ocrVat, setOcrVat] = useState("");
  const [vies, setVies] = useState<ViesLookup | null>(null);
  const [itemOpenFor, setItemOpenFor] = useState<number | null>(null);
  const [itemQuery, setItemQuery] = useState("");
  const [addingProduct, setAddingProduct] = useState(false);
  const [newName, setNewName] = useState("");
  const [newCode, setNewCode] = useState("");
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState("");
  const [fromXml, setFromXml] = useState(false);
  const [paymentMethod, setPaymentMethod] = useState("банков превод");
  const [paymentNote, setPaymentNote] = useState("");
  const [currencyRate, setCurrencyRate] = useState("1");
  const [taxEvent, setTaxEvent] = useState("");
  const [vatExemption, setVatExemption] = useState("");
  const [exemptions, setExemptions] = useState<VatExemption[]>([]);
  const [exLoaded, setExLoaded] = useState(false);
  const [originalInvoiceId, setOriginalInvoiceId] = useState("");
  const [origNumber, setOrigNumber] = useState("");
  const [invoiceChoices, setInvoiceChoices] = useState<Invoice[]>([]);
  const [invLoaded, setInvLoaded] = useState(false);

  const loadLookups = useCallback(async () => {
    try {
      const [cp, pr] = await Promise.all([
        api.get<ListResponse<Counterpart>>("/v1/counterparts"),
        api.get<ListResponse<Product>>("/v1/products"),
      ]);
      setCounterparts(cp.items ?? []);
      setProducts(pr.items ?? []);
    } catch {
      /* non-fatal */
    }
  }, []);

  useEffect(() => {
    loadLookups();
  }, [loadLookups]);

  useEffect(() => {
    if (stage !== 1 || exLoaded) return;
    api
      .get<ListResponse<VatExemption>>("/v1/vat-exemptions")
      .then((r) => setExemptions(r.items ?? []))
      .catch(() => setExemptions([]))
      .finally(() => setExLoaded(true));
  }, [stage, exLoaded]);

  useEffect(() => {
    if (stage !== 1 || !docTypeRequiresOriginal(documentType) || invLoaded) return;
    api
      .get<ListResponse<Invoice>>("/v1/invoices")
      .then((r) => setInvoiceChoices(r.items ?? []))
      .catch(() => setInvoiceChoices([]))
      .finally(() => setInvLoaded(true));
  }, [stage, documentType, invLoaded]);

  const addFiles = (list: FileList | File[]) => {
    const next: QueuedFile[] = [];
    for (const file of Array.from(list)) {
      const name = file.name.toLowerCase();
      const ok =
        name.endsWith(".pdf") ||
        name.endsWith(".xml") ||
        name.endsWith(".jpg") ||
        name.endsWith(".jpeg") ||
        name.endsWith(".png") ||
        file.type === "application/pdf" ||
        file.type === "text/xml" ||
        file.type === "application/xml" ||
        file.type.startsWith("image/");
      if (!ok) {
        setError(t("scan.need_file"));
        continue;
      }
      if (file.size > MAX_BYTES) {
        setError(t("scan.too_big"));
        continue;
      }
      next.push({ id: `${file.name}-${file.size}-${file.lastModified}`, file });
    }
    if (next.length) {
      setQueue((prev) => {
        const seen = new Set(prev.map((q) => q.id));
        return [...prev, ...next.filter((n) => !seen.has(n.id))];
      });
      setError("");
    }
  };

  const onInput = (e: ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) addFiles(e.target.files);
    e.target.value = "";
  };

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragging(false);
    if (e.dataTransfer.files) addFiles(e.dataTransfer.files);
  };

  const productById = useMemo(() => {
    const m = new Map<number, Product>();
    for (const p of products) m.set(p.id, p);
    return m;
  }, [products]);

  const priced = useMemo(
    () => lines.map((l) => calcLine(l, false, docTypeIsCredit(documentType))),
    [lines, documentType]
  );
  const totals = useMemo(() => calcTotals(priced, "0"), [priced]);

  const filteredProducts = useMemo(() => {
    const q = itemQuery.trim().toLowerCase();
    if (!q) return products;
    return products.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        (p.code || "").toLowerCase().includes(q)
    );
  }, [products, itemQuery]);

  const setLine = (i: number, patch: Partial<InvoiceLine>) =>
    setLines((prev) =>
      prev.map((l, idx) => {
        if (idx !== i) return l;
        const next = { ...l, ...patch };
        if ("quantity" in patch || "unit_price" in patch || "vat_rate" in patch) {
          next.keep_amounts = false;
        }
        return next;
      })
    );

  const openProductPicker = (i: number) => {
    const scanned = lines[i]?.description || "";
    setItemOpenFor(i);
    setItemQuery(scanned);
    setAddingProduct(false);
    setNewName(scanned);
    setNewCode("");
    setCreateError("");
  };

  const closeProductPicker = () => {
    setItemOpenFor(null);
    setItemQuery("");
    setAddingProduct(false);
    setCreateError("");
  };

  const pickProduct = (i: number, p: Product) => {
    setLine(i, {
      product_id: p.id,
      code: p.code || "",
    });
    closeProductPicker();
  };

  const createProduct = async () => {
    if (itemOpenFor === null) return;
    const name = newName.trim();
    if (!name) {
      setCreateError(t("scan.need_product_name"));
      return;
    }
    const line = lines[itemOpenFor];
    setCreating(true);
    setCreateError("");
    try {
      const created = await api.post<Product>("/v1/products", {
        name,
        code: newCode.trim(),
        description: line.description || name,
        unit: line.unit || "C62",
        price: line.unit_price || "0",
        vat_rate: line.vat_rate || "20",
        is_active: true,
      });
      setProducts((prev) => [...prev, created]);
      pickProduct(itemOpenFor, created);
    } catch (e) {
      setCreateError(e instanceof Error ? e.message : String(e));
    } finally {
      setCreating(false);
    }
  };

  const clearProduct = (i: number) => {
    setLine(i, { product_id: undefined, code: "" });
  };

  const runExtract = async (item: QueuedFile) => {
    const xml = isXmlFile(item.file);
    setBusy(xml ? t("scan.running_ubl") : t("scan.running"));
    setError("");
    setActiveName(item.file.name);
    setFromXml(xml);
    try {
      const content_base64 = await fileToBase64(item.file);
      const data = await api.post<ScanExtract>(xml ? "/v1/scans/ubl" : "/v1/scans/extract", {
        filename: item.file.name,
        mime: item.file.type || (xml ? "application/xml" : ""),
        content_base64,
        direction,
      });
      if (data.direction === "in" || data.direction === "out") setDirection(data.direction);
      setNumber(data.invoice_number || "");
      setIssueDate(data.issue_date || todayIso());
      setDueDate(data.due_date || "");
      setTaxEvent(data.tax_event_date || data.issue_date || "");
      setDocumentType(data.document_type || "01");
      setCurrency(data.currency || "EUR");
      setCurrencyRate(data.currency_rate || "1");
      setPaymentMethod(data.payment_method || "банков превод");
      setPaymentNote(data.payment_note || "");
      setVatExemption(data.vat_exemption_reason || "");
      setOrigNumber(data.original_invoice_number || "");
      setOriginalInvoiceId(data.original_invoice_id ? String(data.original_invoice_id) : "");
      setOcrName(data.counterpart_name || "");
      setOcrEik(data.counterpart_eik || "");
      setOcrVat(data.counterpart_vat_number || "");
      setCounterpartId(data.counterpart_id ? String(data.counterpart_id) : "");
      const mapped = (data.lines ?? []).map(lineFromScan);
      setLines(mapped.length ? mapped : [emptyLine()]);
      setVies(null);
      const vat = (data.counterpart_vat_number || "").replace(/\s/g, "");
      if (vat.length >= 4) {
        try {
          const v = await api.get<ViesLookup>(
            `/v1/counterparts/vies-lookup?vat=${encodeURIComponent(vat)}`
          );
          setVies(v);
        } catch {
          setVies(null);
        }
      }
      setStage(1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const createCounterpart = async () => {
    setBusy(t("scan.saving"));
    setError("");
    try {
      const ctype = direction === "in" ? "supplier" : "customer";
      const vat = ocrVat.replace(/\s/g, "");
      let created: Counterpart;
      if (vat.length >= 4 && vies && (vies.valid === true || vies.valid === 1)) {
        created = await api.post<Counterpart>("/v1/counterparts/vies", {
          vat_number: vat,
          counterpart_type: ctype,
        });
      } else {
        created = await api.post<Counterpart>("/v1/counterparts", {
          name: ocrName || vat || ocrEik || t("scan.counterpart"),
          eik: ocrEik,
          vat_number: vat,
          counterpart_type: ctype,
        });
      }
      setCounterparts((prev) => [...prev, created]);
      setCounterpartId(String(created.id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const saveDraft = async () => {
    if (!counterpartId) {
      setError(t("scan.need_counterpart"));
      return;
    }
    if (direction === "in" && !number.trim()) {
      setError(t("scan.need_number"));
      return;
    }
    if (docTypeRequiresOriginal(documentType) && !originalInvoiceId) {
      setError(t("invoices.need_original"));
      return;
    }
    const computed = priced;
    const zeroVat = computed.some((l) => num(l.vat_rate) === 0);
    if (!computed.length) {
      setError(t("scan.no_lines"));
      return;
    }
    if (fromXml && zeroVat && !vatExemption) {
      setError(t("invoices.vat_exemption_pick"));
      return;
    }
    setBusy(t("scan.saving"));
    setError("");
    try {
      const payloadLines = computed.map((l) => ({
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
      const created = await api.post<Invoice>("/v1/invoices", {
        direction,
        document_type: documentType,
        number,
        issue_date: issueDate,
        tax_event_date: taxEvent || issueDate,
        due_date: dueDate,
        accounting_month: period || issueDate.slice(0, 7),
        counterpart_id: Number(counterpartId),
        currency,
        currency_rate: currencyRate || "1",
        payment_method: paymentMethod,
        notes: [
          activeName ? `${t(fromXml ? "scan.ubl_from" : "scan.scanned_from")}: ${activeName}` : "",
          paymentNote,
        ]
          .filter(Boolean)
          .join(" · "),
        discount_percent: "0",
        discount_amount: "0",
        vat_exemption_reason: vatExemption,
        original_invoice_id: originalInvoiceId ? Number(originalInvoiceId) : 0,
        lines: payloadLines,
      });
      const mapItems = computed
        .filter((l) => l.product_id && l.description)
        .map((l) => ({
          scanned_name: l.description,
          product_id: l.product_id,
        }));
      if (mapItems.length) {
        await api.post("/v1/product-name-mappings", {
          counterpart_id: Number(counterpartId),
          items: mapItems,
        });
      }
      setQueue((prev) => prev.filter((q) => q.file.name !== activeName));
      router.push(`/invoices/${created.id}`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const found = counterparts.find((c) => String(c.id) === counterpartId);

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("scan.title")}</h1>
        {stage === 1 && (
          <button
            className="btn"
            onClick={() => {
              setStage(0);
              setError("");
            }}
          >
            {t("scan.back_list")}
          </button>
        )}
      </div>
      <p className="muted" style={{ marginTop: -8, marginBottom: 16 }}>
        {t("scan.hint")}
      </p>
      {error && <div className="flash-err">{error}</div>}
      {busy && (
        <div className="card content" style={{ textAlign: "center", padding: 40 }}>
          <p>{busy}</p>
        </div>
      )}

      {!busy && stage === 0 && (
        <div className="scan-grid">
          <section className="card" style={{ padding: 16 }}>
            <h2 className="invoice-section-title">{t("scan.upload")}</h2>
            <div className="field">
              <span className="label">{t("scan.type")}</span>
              <div className="scan-toggle">
                <button
                  className={`btn${direction === "in" ? " btn-primary" : ""}`}
                  onClick={() => setDirection("in")}
                >
                  {t("scan.purchases")}
                </button>
                <button
                  className={`btn${direction === "out" ? " btn-primary" : ""}`}
                  onClick={() => setDirection("out")}
                >
                  {t("scan.sales")}
                </button>
              </div>
            </div>
            <div className="field">
              <label className="label">{t("scan.period")}</label>
              <input
                className="input"
                type="month"
                value={period}
                onChange={(e) => setPeriod(e.target.value)}
              />
            </div>
            <label
              className={`drop-zone${dragging ? " drop-zone-active" : ""}`}
              onDragOver={(e) => {
                e.preventDefault();
                setDragging(true);
              }}
              onDragLeave={() => setDragging(false)}
              onDrop={onDrop}
            >
              <div className="drop-zone-icon">📁</div>
              <div>{t("scan.drop")}</div>
              <div className="muted">{t("scan.drop_or")}</div>
              <input
                type="file"
                accept=".pdf,.jpg,.jpeg,.png,.xml,application/pdf,application/xml,text/xml,image/*"
                multiple
                hidden
                onChange={onInput}
              />
            </label>
          </section>
          <section className="card" style={{ padding: 16 }}>
            <h2 className="invoice-section-title">{t("scan.queue")}</h2>
            {queue.length === 0 ? (
              <p className="muted">{t("scan.empty_queue")}</p>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>{t("scan.file")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {queue.map((q) => (
                    <tr key={q.id}>
                      <td>{q.file.name}</td>
                      <td className="icon-actions" style={{ textAlign: "right" }}>
                        <button
                          className="btn btn-primary btn-sm"
                          onClick={() => runExtract(q)}
                        >
                          {isXmlFile(q.file) ? t("scan.run_ubl") : t("scan.run")}
                        </button>
                        <IconButton
                          icon="delete"
                          title={t("common.delete")}
                          danger
                          onClick={() =>
                            setQueue((prev) => prev.filter((x) => x.id !== q.id))
                          }
                        />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>
        </div>
      )}

      {!busy && stage === 1 && (
        <ScanDraft
          t={t}
          direction={direction}
          documentType={documentType}
          setDocumentType={setDocumentType}
          number={number}
          setNumber={setNumber}
          issueDate={issueDate}
          setIssueDate={setIssueDate}
          dueDate={dueDate}
          setDueDate={setDueDate}
          currency={currency}
          setCurrency={setCurrency}
          paymentMethod={paymentMethod}
          setPaymentMethod={setPaymentMethod}
          vatExemption={vatExemption}
          setVatExemption={setVatExemption}
          exemptions={exemptions}
          showExemption={
            Boolean(vatExemption) ||
            (fromXml && priced.some((l) => num(l.vat_rate) === 0))
          }
          originalInvoiceId={originalInvoiceId}
          setOriginalInvoiceId={setOriginalInvoiceId}
          origNumber={origNumber}
          invoiceChoices={invoiceChoices}
          ocrName={ocrName}
          setOcrName={setOcrName}
          ocrEik={ocrEik}
          setOcrEik={setOcrEik}
          ocrVat={ocrVat}
          setOcrVat={setOcrVat}
          counterpartId={counterpartId}
          setCounterpartId={setCounterpartId}
          counterparts={counterparts}
          found={found}
          createCounterpart={createCounterpart}
          vies={vies}
          priced={priced}
          productById={productById}
          setLine={setLine}
          openProductPicker={openProductPicker}
          clearProduct={clearProduct}
          setLines={setLines}
          totals={totals}
          saveDraft={saveDraft}
          itemOpenFor={itemOpenFor}
          closeProductPicker={closeProductPicker}
          lines={lines}
          addingProduct={addingProduct}
          itemQuery={itemQuery}
          setItemQuery={setItemQuery}
          filteredProducts={filteredProducts}
          pickProduct={pickProduct}
          setAddingProduct={setAddingProduct}
          setNewName={setNewName}
          setCreateError={setCreateError}
          createError={createError}
          newName={newName}
          newCode={newCode}
          setNewCode={setNewCode}
          createProduct={createProduct}
          creating={creating}
        />
      )}
    </div>
  );
}

export default function ScanPage() {
  return (
    <RequireAuth>
      <ScanInner />
    </RequireAuth>
  );
}
