"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { ACTIVE_COMPANY_EVENT, api, ListResponse } from "../../lib/api";
import { AuditEntry, formatBgDateTime } from "../../lib/invoice";
import { useI18n } from "../../components/I18nProvider";
import { RequireAuth } from "../../components/RequireAuth";
import { Pagination } from "../../components/Pagination";

const PAGE_SIZE = 50;

const ENTITY_TYPES = [
  "invoice",
  "journal",
  "internal_doc",
  "material_issue",
  "production_order",
  "dividend",
] as const;

const ACTIONS = ["create", "update", "post", "unpost", "delete"] as const;

function AuditInner() {
  const { t } = useI18n();
  const [items, setItems] = useState<AuditEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [entityType, setEntityType] = useState("");
  const [action, setAction] = useState("");
  const [userFilter, setUserFilter] = useState("");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");
  const [changedOnly, setChangedOnly] = useState(false);
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);
  const [expandedId, setExpandedId] = useState<number | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const res = await api.get<ListResponse<AuditEntry>>("/v1/audit-logs", {
        entity_type: entityType || undefined,
        action: action || undefined,
        user: userFilter.trim() || undefined,
        from: fromDate || undefined,
        to: toDate || undefined,
        changed_only: changedOnly ? "1" : undefined,
        q: q.trim() || undefined,
        limit: PAGE_SIZE,
        offset,
      });
      setItems(res.items ?? []);
      setTotal(res.total ?? res.count ?? 0);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [entityType, action, userFilter, fromDate, toDate, changedOnly, q, offset]);

  useEffect(() => {
    load();
    const onCompany = () => {
      setOffset(0);
      load();
    };
    window.addEventListener(ACTIVE_COMPANY_EVENT, onCompany);
    return () => window.removeEventListener(ACTIVE_COMPANY_EVENT, onCompany);
  }, [load]);

  const actionBadge = (act: string) => {
    switch (act) {
      case "create":
        return "badge-success";
      case "update":
        return "badge-warning";
      case "post":
        return "badge-success";
      case "unpost":
      case "delete":
        return "badge-danger";
      default:
        return "badge-muted";
    }
  };

  const entityLink = (item: AuditEntry) => {
    if (item.action === "delete" || !item.entity_id) return null;
    if (item.entity_type === "invoice") return `/invoices/${item.entity_id}`;
    if (item.entity_type === "internal_doc") return `/internal-docs/${item.entity_id}`;
    if (item.entity_type === "journal") return `/journal`;
    if (item.entity_type === "material_issue") return `/material-issues`;
    if (item.entity_type === "production_order") return `/production`;
    if (item.entity_type === "dividend") return `/dividends`;
    return null;
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("audit.title")}</h1>
      </div>

      <p className="muted" style={{ marginBottom: 12 }}>
        {t("audit.hint")}
      </p>

      {error && <div className="error-text">{error}</div>}

      <div className="card" style={{ padding: 16, marginBottom: 12 }}>
        <div className="form-grid">
          <div className="field">
            <label className="label">{t("common.search")}</label>
            <input
              className="input"
              value={q}
              onChange={(e) => {
                setOffset(0);
                setQ(e.target.value);
              }}
              placeholder={t("common.search")}
            />
          </div>
          <div className="field">
            <label className="label">{t("audit.entity")}</label>
            <select
              className="select"
              value={entityType}
              onChange={(e) => {
                setOffset(0);
                setEntityType(e.target.value);
              }}
            >
              <option value="">{t("audit.all_entities")}</option>
              {ENTITY_TYPES.map((et) => (
                <option key={et} value={et}>
                  {t(`audit.entity.${et}`)}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label className="label">{t("audit.action")}</label>
            <select
              className="select"
              value={action}
              onChange={(e) => {
                setOffset(0);
                setAction(e.target.value);
              }}
            >
              <option value="">{t("audit.all_actions")}</option>
              {ACTIONS.map((act) => (
                <option key={act} value={act}>
                  {t(`audit.action.${act}`)}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label className="label">{t("audit.user")}</label>
            <input
              className="input"
              value={userFilter}
              onChange={(e) => {
                setOffset(0);
                setUserFilter(e.target.value);
              }}
              placeholder="superadmin, email…"
            />
          </div>
          <div className="field">
            <label className="label">{t("reports.from")}</label>
            <input
              className="input"
              type="date"
              value={fromDate}
              onChange={(e) => {
                setOffset(0);
                setFromDate(e.target.value);
              }}
            />
          </div>
          <div className="field">
            <label className="label">{t("reports.to")}</label>
            <input
              className="input"
              type="date"
              value={toDate}
              onChange={(e) => {
                setOffset(0);
                setToDate(e.target.value);
              }}
            />
          </div>
        </div>
        <div style={{ marginTop: 12 }}>
          <label className="check-inline">
            <input
              type="checkbox"
              checked={changedOnly}
              onChange={(e) => {
                setOffset(0);
                setChangedOnly(e.target.checked);
              }}
            />
            {t("audit.changed_only")}
          </label>
        </div>
      </div>

      <div className="card">
        {loading ? (
          <div className="content muted">{t("common.loading")}</div>
        ) : items.length === 0 ? (
          <div className="content muted">{t("common.empty")}</div>
        ) : (
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>{t("audit.when")}</th>
                  <th>{t("audit.user")}</th>
                  <th>{t("audit.entity")}</th>
                  <th>{t("audit.number")}</th>
                  <th>{t("audit.action")}</th>
                  <th>{t("audit.amounts")}</th>
                  <th>{t("audit.summary")}</th>
                  <th style={{ width: 90 }}>{t("audit.details")}</th>
                </tr>
              </thead>
              <tbody>
                {items.map((item) => {
                  const changed =
                    item.old_amount &&
                    item.new_amount &&
                    item.old_amount !== item.new_amount;
                  const href = entityLink(item);
                  const hasSnap = Boolean(item.old_data || item.new_data);
                  const open = expandedId === item.id;
                  return (
                    <tr
                      key={item.id}
                      style={
                        changed
                          ? { background: "rgba(220, 38, 38, 0.08)" }
                          : undefined
                      }
                    >
                      <td style={{ whiteSpace: "nowrap" }}>
                        {formatBgDateTime(item.created_at)}
                      </td>
                      <td>
                        <strong>{item.user_name || item.user_login || "—"}</strong>
                      </td>
                      <td>{t(`audit.entity.${item.entity_type}`)}</td>
                      <td>
                        {href ? (
                          <Link href={href}>
                            {item.entity_number || `#${item.entity_id}`}
                          </Link>
                        ) : (
                          <span>{item.entity_number || `#${item.entity_id}`}</span>
                        )}
                      </td>
                      <td>
                        <span className={`badge ${actionBadge(item.action)}`}>
                          {t(`audit.action.${item.action}`)}
                        </span>
                      </td>
                      <td style={{ whiteSpace: "nowrap" }}>
                        {item.old_amount && item.new_amount ? (
                          changed ? (
                            <span style={{ color: "#dc2626", fontWeight: 600 }}>
                              {item.old_amount} → {item.new_amount}{" "}
                              <span className="badge badge-danger">
                                {t("audit.amount_changed_badge")}
                              </span>
                            </span>
                          ) : (
                            <span>{item.new_amount}</span>
                          )
                        ) : (
                          <span>{item.new_amount || item.old_amount || "—"}</span>
                        )}
                      </td>
                      <td>
                        <div>{item.summary}</div>
                        {open && hasSnap && (
                          <div
                            style={{
                              display: "grid",
                              gridTemplateColumns: "1fr 1fr",
                              gap: 12,
                              marginTop: 10,
                            }}
                          >
                            {Boolean(item.old_data) && (
                              <div>
                                <div className="muted" style={{ marginBottom: 4 }}>
                                  {t("audit.before")}
                                </div>
                                <pre
                                  style={{
                                    fontSize: 11,
                                    maxHeight: 240,
                                    overflow: "auto",
                                    padding: 8,
                                    borderRadius: 6,
                                    background: "rgba(0,0,0,0.06)",
                                    margin: 0,
                                  }}
                                >
                                  {JSON.stringify(item.old_data, null, 2)}
                                </pre>
                              </div>
                            )}
                            {Boolean(item.new_data) && (
                              <div>
                                <div className="muted" style={{ marginBottom: 4 }}>
                                  {t("audit.after")}
                                </div>
                                <pre
                                  style={{
                                    fontSize: 11,
                                    maxHeight: 240,
                                    overflow: "auto",
                                    padding: 8,
                                    borderRadius: 6,
                                    background: "rgba(0,0,0,0.06)",
                                    margin: 0,
                                  }}
                                >
                                  {JSON.stringify(item.new_data, null, 2)}
                                </pre>
                              </div>
                            )}
                          </div>
                        )}
                      </td>
                      <td>
                        {hasSnap && (
                          <button
                            type="button"
                            className="btn btn-sm"
                            onClick={() => setExpandedId(open ? null : item.id)}
                          >
                            {open ? "▲" : "JSON"}
                          </button>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
        <Pagination
          total={total}
          limit={PAGE_SIZE}
          offset={offset}
          onPageChange={setOffset}
        />
      </div>
    </div>
  );
}

export default function AuditPage() {
  return (
    <RequireAuth>
      <AuditInner />
    </RequireAuth>
  );
}
