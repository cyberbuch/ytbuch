"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, ListResponse } from "../../lib/api";
import { InternalDoc } from "../../lib/internalDoc";
import { CompanyLocation, fetchCompanyLocations } from "../../lib/locations";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";

function Inner() {
  const { t } = useI18n();
  const [rows, setRows] = useState<InternalDoc[]>([]);
  const [locations, setLocations] = useState<CompanyLocation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [q, setQ] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const data = await api.get<ListResponse<InternalDoc>>("/v1/internal-docs");
      setRows(data.items ?? []);
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

  const locName = (id: number) => locations.find((l) => l.id === id)?.name ?? String(id);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter((d) =>
      `${d.number} ${d.doc_date} ${locName(d.from_location_id)} ${locName(d.to_location_id)} ${d.status}`
        .toLowerCase()
        .includes(s)
    );
  }, [rows, q, locations]);

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
              </tr>
            </thead>
            <tbody>
              {filtered.map((d) => (
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
                </tr>
              ))}
            </tbody>
          </table>
        )}
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
