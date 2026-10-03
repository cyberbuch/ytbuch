"use client";

import { useEffect, useState } from "react";
import { numberToWordsBg } from "../lib/numberToWordsBg";
import {
  Invoice,
  InvoiceParty,
  docTypePrefix,
  docTypeTitle,
  formatBgDate,
} from "../lib/invoice";
import { api, ListResponse } from "../lib/api";
import { VatExemption, vatexDisplay } from "../lib/vatExemptions";
import { useI18n } from "./I18nProvider";

function fmt(s: string | undefined): string {
  if (!s) return "0.00";
  const n = parseFloat(String(s).replace(",", "."));
  if (Number.isNaN(n)) return s;
  return n.toFixed(2);
}

function partyBlock(title: string, p: InvoiceParty | undefined, extra?: string) {
  const p_ = p ?? {};
  return (
    <div className="print-box">
      <div className="print-box-title">{title}</div>
      <div>{p_.name || "—"}</div>
      {(p_.address || p_.city) && (
        <div>
          {p_.address} {p_.city}
        </div>
      )}
      <div>ЕИК: {p_.eik || "—"}</div>
      <div>ДДС №: {p_.vat_number || "—"}</div>
      {extra ? <div>{extra}</div> : null}
    </div>
  );
}

export function PrintableInvoice({
  invoice,
  isCopy = false,
}: {
  invoice: Invoice;
  isCopy?: boolean;
}) {
  const { t, lang } = useI18n();
  const [exemptions, setExemptions] = useState<VatExemption[]>([]);
  useEffect(() => {
    api
      .get<ListResponse<VatExemption>>("/v1/vat-exemptions")
      .then((d) => setExemptions(d.items ?? []))
      .catch(() => setExemptions([]));
  }, []);
  const company = invoice.company;
  const cp = invoice.counterpart;
  const outgoing = invoice.direction !== "in";
  const recipient = outgoing ? cp : company;
  const supplier = outgoing ? company : cp;
  const lines = invoice.lines ?? [];
  const currency = invoice.currency || "BGN";
  const cancelled = invoice.status === "cancelled";

  return (
    <div className="print-sheet">
      {cancelled && <div className="print-watermark">АНУЛИРАНА</div>}
      <div className="print-head">
        <div>
          <div className="print-title">{docTypeTitle(invoice.document_type)}</div>
          <div className="print-copy">
            ( {isCopy ? t("invoices.copy") : t("invoices.original")} )
          </div>
        </div>
        <div className="print-number">
          № {docTypePrefix(invoice.document_type)}
          {invoice.number}
        </div>
        <div className="print-dates">
          <div>
            {t("invoices.issue_date")}: {formatBgDate(invoice.issue_date)}
          </div>
          {invoice.tax_event_date && (
            <div>
              {t("invoices.tax_event_date")}: {formatBgDate(invoice.tax_event_date)}
            </div>
          )}
        </div>
      </div>

      <div className="print-parties">
        {partyBlock(
          t("invoices.recipient"),
          recipient,
          recipient?.contact_person
            ? `${t("counterparts.contact_person")}: ${recipient.contact_person}`
            : undefined
        )}
        {partyBlock(
          t("invoices.supplier"),
          supplier,
          supplier?.mol ? `МОЛ: ${supplier.mol}` : undefined
        )}
      </div>

      <table className="print-lines">
        <thead>
          <tr>
            <th>No</th>
            <th>{t("invoices.item")}</th>
            <th className="r">{t("invoices.qty")}</th>
            <th>{t("products.unit")}</th>
            <th className="r">{t("invoices.unit_price")}</th>
            <th className="r">{t("invoices.line_total")}</th>
          </tr>
        </thead>
        <tbody>
          {lines.map((l, i) => (
            <tr key={i}>
              <td>{i + 1}</td>
              <td>{l.description}</td>
              <td className="r">{l.quantity}</td>
              <td>{l.unit}</td>
              <td className="r">{fmt(l.unit_price)}</td>
              <td className="r">{fmt(l.net_amount)}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <div className="print-bottom">
        <div className="print-meta">
          {company?.iban && (
            <div>
              <b>{t("invoices.bank_account")}</b>
              <div>IBAN: {company.iban}</div>
              {company.bic && <div>BIC: {company.bic}</div>}
            </div>
          )}
          {invoice.payment_method && (
            <div>
              {t("invoices.payment_method")}: {invoice.payment_method}
            </div>
          )}
          {invoice.due_date && (
            <div>
              {t("invoices.due_date")}: {invoice.due_date}
            </div>
          )}
          {invoice.notes && (
            <div>
              {t("invoices.notes")}: {invoice.notes}
            </div>
          )}
          {invoice.vat_exemption_reason && (
            <div>
              <b>{t("invoices.vat_exemption")}:</b>{" "}
              {vatexDisplay(invoice.vat_exemption_reason, exemptions, lang)}
            </div>
          )}
        </div>
        <div className="print-totals">
          <div>
            <span>{t("invoices.taxable")}:</span>
            <b>
              {fmt(invoice.net_amount)} {currency}
            </b>
          </div>
          <div>
            <span>{t("invoices.vat")}:</span>
            <b>
              {fmt(invoice.vat_amount)} {currency}
            </b>
          </div>
          {invoice.discount_amount && parseFloat(invoice.discount_amount) > 0 && (
            <div>
              <span>{t("invoices.discount")}:</span>
              <b>
                {fmt(invoice.discount_amount)} {currency}
              </b>
            </div>
          )}
          <div className="print-grand">
            <span>{t("invoices.amount_due")}:</span>
            <b>
              {fmt(invoice.total_amount)} {currency}
            </b>
          </div>
          <div className="print-words">
            {t("invoices.in_words")}: {numberToWordsBg(invoice.total_amount, currency)}
          </div>
        </div>
      </div>
    </div>
  );
}
