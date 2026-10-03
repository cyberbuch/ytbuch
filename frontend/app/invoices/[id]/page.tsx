"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { api, downloadFile } from "../../../lib/api";
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

  return (
    <div>
      <div className="page-head no-print">
        <h1 className="page-title">
          {t(`invoices.document_type.${inv.document_type}`)} № {inv.number}
        </h1>
        <div className="btn-row">
          <Link className="btn" href="/invoices">
            {t("common.back")}
          </Link>
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
          <IconButton icon="print" title={t("invoices.print")} onClick={() => window.print()} />
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
        {posted && inv.journal_entry_id ? (
          <div className="muted" style={{ marginTop: 8 }}>
            {t("invoices.journal_link")}: {inv.journal_entry_id}
          </div>
        ) : null}
      </div>

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
