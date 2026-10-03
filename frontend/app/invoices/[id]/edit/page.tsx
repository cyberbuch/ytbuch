"use client";

import { useParams } from "next/navigation";
import { InvoiceForm } from "../../../../components/InvoiceForm";
import { RequireAuth } from "../../../../components/RequireAuth";
import { useI18n } from "../../../../components/I18nProvider";

function EditInner() {
  const { t } = useI18n();
  const params = useParams();
  const raw = params.id;
  const id = Number(Array.isArray(raw) ? raw[0] : raw);
  if (!Number.isFinite(id) || id <= 0) {
    return <div className="error-text">{t("common.error")}</div>;
  }
  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("invoices.edit")}</h1>
      </div>
      <InvoiceForm mode="edit" invoiceId={id} />
    </div>
  );
}

export default function EditInvoicePage() {
  return (
    <RequireAuth>
      <EditInner />
    </RequireAuth>
  );
}
