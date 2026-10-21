"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";
import { fetchAll } from "../../lib/api";
import { InternalDoc } from "../../lib/internalDoc";
import { CompanyLocation, fetchCompanyLocations } from "../../lib/locations";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { Pagination } from "../../components/Pagination";
import { AuditStampBadge } from "../../components/AuditHistory";

const PAGE_SIZE = 50;

function Inner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<InternalDoc[]>([]);
  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      setRows(await fetchAll<InternalDoc>("/v1/internal-docs"));
      setLocations(await fetchCompanyLocations());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const locName = useCallback(
    (id: number) => locations.find((l) => l.id === id)?.name ?? String(id),
    [locations]
  );

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((d) =>
      `${d.number} ${d.doc_date} ${locName(d.from_location_id)} ${locName(d.to_location_id)} ${d.status}`
        .toLowerCase()
        .includes(s)
    );
  }, [rows, q, locName]);

  const pageCount = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const page = Math.min(Math.floor(offset / PAGE_SIZE), pageCount - 1);
  const start = page * PAGE_SIZE;
  const paged = filtered.slice(start, start + PAGE_SIZE);

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("idoc.title")}</h1>
        <Link className="btn btn-primary" href="/internal-docs/new">
          {t("idoc.new")}
        </Link>
      </div>
      {error && <div className="error-text">{error}</div>}
      <div className="card" style={{ marginBottom: 12 }}>
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={t("common.search")}
        />
      </div>
      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : filtered.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("idoc.number")}</th>
                <th>{t("idoc.date")}</th>
                <th>{t("idoc.from")}</th>
                <th>{t("idoc.to")}</th>
                <th>{t("idoc.status")}</th>
                <th>{t("audit.col_stamp")}</th>
              </tr>
            </thead>
            <tbody>
              {paged.map((d) => (
                <tr key={d.id}>
                  <td>
                    <Link href={`/internal-docs/${d.id}`}>{d.number}</Link>
                  </td>
                  <td>{d.doc_date}</td>
                  <td>{locName(d.from_location_id)}</td>
                  <td>{locName(d.to_location_id)}</td>
                  <td>
                    <span
                      className={`badge ${d.status === "confirmed" ? "badge-success" : "badge-warning"}`}
                    >
                      {t(`idoc.status.${d.status}`)}
                    </span>
                  </td>
                  <td>
                    <AuditStampBadge
                      createdBy={d.created_by}
                      createdAt={d.created_at}
                      updatedBy={d.updated_by}
                      updatedAt={d.updated_at}
                      editCount={d.edit_count}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <Pagination
          total={filtered.length}
          limit={PAGE_SIZE}
          offset={start}
          onPageChange={setOffset}
        />
      </div>
    </div>
  );
}

export default function InternalDocsPage() {
  return (
    <RequireAuth>
      <Inner />
    </RequireAuth>
  );
}
