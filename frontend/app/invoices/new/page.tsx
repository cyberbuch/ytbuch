"use client";

import { InvoiceForm } from "../../../components/InvoiceForm";
import { RequireAuth } from "../../../components/RequireAuth";
import { useI18n } from "../../../components/I18nProvider";

function NewInner() {
  const { t } = useI18n();
  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("invoices.new")}</h1>
      </div>
      <InvoiceForm mode="create" />
    </div>
  );
}

export default function NewInvoicePage() {
  return (
    <RequireAuth>
      <NewInner />
    </RequireAuth>
  );
}
