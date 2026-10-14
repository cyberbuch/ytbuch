"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { api, downloadFile, printFile } from "../../../lib/api";
import { Invoice, docTypePrefix, invoiceDownloadName } from "../../../lib/invoice";
import { PrintableInvoice } from "../../../components/PrintableInvoice";
import { RequireAuth } from "../../../components/RequireAuth";
import { useI18n } from "../../../components/I18nProvider";
import { IconButton } from "../../../components/IconButton";

function DetailInner() {
  const { t } = useI18n();
  const params = useParams();
  const router = useRouter();
  const id = Number(params.id);
  const [inv, setInv] = useState<Invoice | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [isCopy, setIsCopy] = useState(false);
  const [protoModal, setProtoModal] = useState(false);
  const [protoArticle, setProtoArticle] = useState("чл. 117");
  const [protoRate, setProtoRate] = useState("20");
  const [protoDate, setProtoDate] = useState("");
  const [protoNumber, setProtoNumber] = useState("");
  const [customsModal, setCustomsModal] = useState(false);
  const [mrnNumber, setMrnNumber] = useState("");
  const [mrnDate, setMrnDate] = useState("");
  const [mrnStat, setMrnStat] = useState("");
  const [mrnDuty, setMrnDuty] = useState("0.00");
  const [mrnVat, setMrnVat] = useState("");

  const load = useCallback(async () => {
    setError("");
    try {
      const data = await api.get<Invoice>(`/v1/invoices/${id}`);
      setInv(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [id]);

  useEffect(() => {
    if (id) load();
  }, [id, load]);

  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(label);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const filename = (ext: string) =>
    invoiceDownloadName(inv?.number || String(id), inv?.document_type || "01", ext);

  if (!inv && !error) {
    return <div className="muted">{t("common.loading")}</div>;
  }
  if (!inv) {
    return <div className="error-text">{error}</div>;
  }

  const draft = inv.status === "draft";
  const posted = inv.status === "posted";
  const isExcludedVat =
    inv.direction === "in" &&
    inv.document_type !== "07" &&
    inv.document_type !== "09" &&
    Number(inv.exclude_vat || 0) === 1;

  return (
    <div>
      <div className="page-head no-print">
        <h1 className="page-title">
          {inv.document_type === "09" && inv.vat_article
            ? `Протокол по ${inv.vat_article} от ЗДДС № ${inv.number}`
            : `${t(`invoices.document_type.${inv.document_type}`)} № ${inv.number}`}
        </h1>
        <div className="btn-row wrap">
          <Link className="btn" href="/invoices">
            {t("common.back")}
          </Link>
          {isExcludedVat && (
            <>
              <button
                type="button"
                className="btn"
                disabled={!!busy}
                onClick={async () => {
                  setProtoDate(inv.issue_date);
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
                disabled={!!busy}
                onClick={() => {
                  setMrnDate(inv.issue_date);
                  const st = inv.net_amount || "0.00";
                  setMrnStat(st);
                  setMrnDuty("0.00");
                  const n = parseFloat(String(st).replace(",", ".")) || 0;
                  setMrnVat((n * 0.2).toFixed(2));
                  setCustomsModal(true);
                }}
              >
                {t("invoices.issue_customs")}
              </button>
            </>
          )}
          {draft && (
            <IconButton icon="edit" title={t("common.edit")} href={`/invoices/${inv.id}/edit`} />
          )}
          {draft && inv.document_type !== "proforma" && (
            <button
              className="btn btn-primary"
              disabled={!!busy}
              onClick={() =>
                run("post", async () => {
                  await api.post(`/v1/invoices/${inv.id}/post`);
                  await load();
                })
              }
            >
              {t("invoices.post")}
            </button>
          )}
        </div>
      </div>

      {isExcludedVat && (
        <div
          className="card no-print"
          style={{
            padding: "12px 16px",
            marginBottom: 16,
            border: "1px solid #d97706",
            background: "rgba(245, 158, 11, 0.12)",
          }}
        >
          ⚠️ {t("invoices.exclude_vat_warning")}
        </div>
      )}

      {error && <div className="error-text no-print">{error}</div>}

      <div className="card no-print" style={{ padding: 16, marginBottom: 16 }}>
        <div className="btn-row wrap">
          <label className="check-inline">
            <input
              type="checkbox"
              checked={isCopy}
              onChange={(e) => setIsCopy(e.target.checked)}
            />
            {t("invoices.copy")}
          </label>
          <IconButton
            icon="print"
            title={t("invoices.print")}
            disabled={!!busy}
            onClick={() =>
              run("print", () =>
                printFile(`/v1/invoices/${inv.id}/print?format=pdf${isCopy ? "&copy=1" : ""}`)
              )
            }
          />
          <button
            className="btn btn-sm"
            disabled={busy === "ubl"}
            onClick={() =>
              run("ubl", () =>
                downloadFile(
                  `/v1/invoices/${inv.id}/ubl`,
                  `ubl-${docTypePrefix(inv.document_type || "01")}${inv.number}.xml`
                )
              )
            }
          >
            {t("invoices.ubl_export")}
          </button>
          {(["pdf", "docx", "odt"] as const).map((fmt) => (
            <button
              key={fmt}
              className="btn btn-sm"
              disabled={busy === fmt}
              onClick={() =>
                run(fmt, () =>
                  downloadFile(
                    `/v1/invoices/${inv.id}/print?format=${fmt}${isCopy ? "&copy=1" : ""}`,
                    filename(fmt)
                  )
                )
              }
            >
              {fmt.toUpperCase()}
            </button>
          ))}
          {draft && (
            <IconButton
              icon="delete"
              title={t("common.delete")}
              danger
              disabled={!!busy}
              onClick={() => {
                if (!window.confirm(t("common.confirmDelete"))) return;
                run("del", async () => {
                  await api.del(`/v1/invoices/${inv.id}`);
                  router.push("/invoices");
                });
              }}
            />
          )}
        </div>
        {inv.original_invoice_id ? (
          <div className="muted" style={{ marginTop: 8 }}>
            {t("invoices.linked_doc")}:{" "}
            <Link href={`/invoices/${inv.original_invoice_id}`}>
              #{inv.original_invoice_id}
            </Link>
          </div>
        ) : null}
        {posted && inv.journal_entry_id ? (
          <div className="muted" style={{ marginTop: 8 }}>
            {t("invoices.journal_link")}: {inv.journal_entry_id}
          </div>
        ) : null}
      </div>

      {protoModal && (
        <div className="modal-backdrop no-print" onClick={() => setProtoModal(false)}>
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
                  disabled={!!busy}
                  onClick={() =>
                    run("proto", async () => {
                      const created = await api.post<Invoice>(
                        `/v1/invoices/${inv.id}/issue-protocol`,
                        {
                          vat_article: protoArticle,
                          vat_rate: protoRate,
                          issue_date: protoDate || inv.issue_date,
                          number: protoNumber,
                        }
                      );
                      setProtoModal(false);
                      router.push(`/invoices/${created.id}`);
                    })
                  }
                >
                  {t("invoices.issue_protocol")}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {customsModal && (
        <div className="modal-backdrop no-print" onClick={() => setCustomsModal(false)}>
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
                      const s = parseFloat(st.replace(",", ".")) || 0;
                      const d = parseFloat(mrnDuty.replace(",", ".")) || 0;
                      setMrnVat(((s + d) * 0.2).toFixed(2));
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
                      const s = parseFloat(mrnStat.replace(",", ".")) || 0;
                      const d = parseFloat(du.replace(",", ".")) || 0;
                      setMrnVat(((s + d) * 0.2).toFixed(2));
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
                  disabled={!!busy || !mrnNumber.trim()}
                  onClick={() =>
                    run("customs", async () => {
                      const created = await api.post<Invoice>(
                        `/v1/invoices/${inv.id}/issue-customs`,
                        {
                          number: mrnNumber,
                          issue_date: mrnDate || inv.issue_date,
                          customs_statistical: mrnStat || inv.net_amount,
                          customs_duty: mrnDuty || "0.00",
                          customs_vat: mrnVat,
                        }
                      );
                      setCustomsModal(false);
                      router.push(`/invoices/${created.id}`);
                    })
                  }
                >
                  {t("invoices.issue_customs")}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      <PrintableInvoice invoice={inv} isCopy={isCopy} />
    </div>
  );
}

export default function InvoiceDetailPage() {
  return (
    <RequireAuth>
      <DetailInner />
    </RequireAuth>
  );
}
