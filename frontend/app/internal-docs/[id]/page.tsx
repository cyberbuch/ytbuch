"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { api, downloadFile } from "../../../lib/api";
import { InternalDoc } from "../../../lib/internalDoc";
import { CompanyLocation, fetchCompanyLocations } from "../../../lib/locations";
import { useI18n } from "../../../components/I18nProvider";
import { RequireAuth } from "../../../components/RequireAuth";
import { IconButton } from "../../../components/IconButton";

function Inner() {
  const { t } = useI18n();
  const params = useParams();
  const router = useRouter();
  const id = Number(params.id);
  const [doc, setDoc] = useState<InternalDoc | null>(null);
  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const d = await api.get<InternalDoc>(`/v1/internal-docs/${id}`);
      setDoc(d);
      setLocations(await fetchCompanyLocations());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  const locName = (lid: number) => locations.find((l) => l.id === lid)?.name ?? String(lid);

  const confirm = async () => {
    setBusy(true);
    setError("");
    try {
      await api.post(`/v1/internal-docs/${id}/confirm`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const del = async () => {
    if (!window.confirm(t("common.confirmDelete"))) return;
    try {
      await api.del(`/v1/internal-docs/${id}`);
      router.push("/internal-docs");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const printDoc = async () => {
    try {
      await downloadFile(`/v1/internal-docs/${id}/print?format=pdf`, `protokol-${doc?.number ?? id}.pdf`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  if (!doc && !error) return <p className="muted">{t("common.loading")}</p>;
  if (!doc) return <div className="error-text">{error}</div>;

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">
          {t("invoices.document_type.internal")} № {doc.number}
        </h1>
        <div className="btn-row">
          <Link className="btn" href="/internal-docs">
            {t("common.back")}
          </Link>
          {doc.status === "draft" && (
            <IconButton icon="edit" title={t("common.edit")} href={`/internal-docs/${id}/edit`} />
          )}
          {doc.status === "draft" && (
            <button className="btn btn-primary" onClick={confirm} disabled={busy}>
              {t("idoc.confirm")}
            </button>
          )}
          {doc.status === "confirmed" && (
            <IconButton icon="print" title={t("idoc.print")} onClick={printDoc} />
          )}
          {doc.status === "draft" && (
            <IconButton icon="delete" title={t("common.delete")} danger onClick={del} />
          )}
        </div>
      </div>
      {error && <div className="error-text">{error}</div>}
      <p className="muted">{t("idoc.confirm_hint")}</p>
      <section className="card card-pad" style={{ marginBottom: 16 }}>
        <p>
          <strong>{t("idoc.date")}:</strong> {doc.doc_date}
        </p>
        <p>
          <strong>{t("idoc.from")}:</strong> {locName(doc.from_location_id)}
        </p>
        <p>
          <strong>{t("idoc.to")}:</strong> {locName(doc.to_location_id)}
        </p>
        <p>
          <strong>{t("idoc.status")}:</strong> {t(`idoc.status.${doc.status}`)}
        </p>
        {doc.handed_by ? (
          <p>
            <strong>{t("idoc.handed_by")}:</strong> {doc.handed_by}
          </p>
        ) : null}
        {doc.received_by ? (
          <p>
            <strong>{t("idoc.received_by")}:</strong> {doc.received_by}
          </p>
        ) : null}
        {doc.notes ? (
          <p>
            <strong>{t("idoc.notes")}:</strong> {doc.notes}
          </p>
        ) : null}
      </section>
      <section className="card">
        <table className="table">
          <thead>
            <tr>
              <th>{t("idoc.line_kind")}</th>
              <th>{t("products.name")}</th>
              <th>{t("idoc.qty")}</th>
              <th>{t("idoc.cost")}</th>
              <th>{t("idoc.amount")}</th>
            </tr>
          </thead>
          <tbody>
            {(doc.lines ?? []).map((l, i) => (
              <tr key={l.id ?? i}>
                <td>{t(`idoc.line_kind.${l.line_kind}`)}</td>
                <td>{l.description}</td>
                <td>{l.quantity}</td>
                <td>{l.unit_cost}</td>
                <td>{l.amount}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}

export default function InternalDocDetailPage() {
  return (
    <RequireAuth>
      <Inner />
    </RequireAuth>
  );
}
