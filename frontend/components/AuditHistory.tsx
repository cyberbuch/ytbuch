"use client";

import { useState } from "react";
import { AuditEntry, formatBgDateTime } from "../lib/invoice";
import { useI18n } from "./I18nProvider";

export function AuditStampBadge({
  createdBy,
  createdAt,
  updatedBy,
  updatedAt,
  editCount,
}: {
  createdBy?: string;
  createdAt?: string;
  updatedBy?: string;
  updatedAt?: string;
  editCount?: number;
}) {
  const { t } = useI18n();
  if (!createdBy && !updatedBy) {
    return <span className="muted">—</span>;
  }
  const edited =
    (editCount && editCount > 0) ||
    (updatedAt && createdAt && updatedAt !== createdAt);
  return (
    <div style={{ fontSize: 12, lineHeight: 1.35 }}>
      {createdBy && (
        <div>
          <span className="muted">{t("audit.created_by")}:</span>{" "}
          <strong>{createdBy}</strong>{" "}
          <span className="muted">({formatBgDateTime(createdAt)})</span>
        </div>
      )}
      {edited && updatedBy && (
        <div style={{ color: "var(--warning, #d97706)", marginTop: 2 }}>
          <span>{t("audit.updated_by")}:</span> <strong>{updatedBy}</strong>{" "}
          <span>({formatBgDateTime(updatedAt)})</span>
          {editCount && editCount > 0 ? (
            <span className="badge badge-warning" style={{ marginLeft: 6 }}>
              {editCount} {t("audit.editions")}
            </span>
          ) : null}
        </div>
      )}
    </div>
  );
}

export function AuditHistoryPanel({
  history,
  createdBy,
  createdAt,
  updatedBy,
  updatedAt,
  editCount,
}: {
  history?: AuditEntry[];
  createdBy?: string;
  createdAt?: string;
  updatedBy?: string;
  updatedAt?: string;
  editCount?: number;
}) {
  const { t } = useI18n();
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const items = history ?? [];

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

  return (
    <div className="card no-print" style={{ padding: 16, marginTop: 16 }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 12,
          marginBottom: items.length > 0 ? 12 : 0,
        }}
      >
        <h3 style={{ margin: 0 }}>{t("audit.history_title")}</h3>
        <AuditStampBadge
          createdBy={createdBy}
          createdAt={createdAt}
          updatedBy={updatedBy}
          updatedAt={updatedAt}
          editCount={editCount}
        />
      </div>

      {items.length === 0 ? (
        <div className="muted" style={{ marginTop: 8 }}>
          {t("audit.no_history")}
        </div>
      ) : (
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th>{t("audit.when")}</th>
                <th>{t("audit.user")}</th>
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
                const hasSnap = Boolean(item.old_data || item.new_data);
                const open = expandedId === item.id;
                return (
                  <tr key={item.id}>
                    <td style={{ whiteSpace: "nowrap" }}>
                      {formatBgDateTime(item.created_at)}
                    </td>
                    <td>
                      <strong>{item.user_name || item.user_login || "—"}</strong>
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
                                  maxHeight: 220,
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
                                  maxHeight: 220,
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
                          onClick={() =>
                            setExpandedId(open ? null : item.id)
                          }
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
    </div>
  );
}
