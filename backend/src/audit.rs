//! Одитна следа: кой потребител, кога (timestamp) и какво е съставил,
//! редактирал, осчетоводил, върнал в чернова или изтрил по документи
//! (фактури, вътрешни документи, изписване на материали, производство)
//! и счетоводни статии в дневника.
//! Записва се стара и нова сума, както и пълен JSON snapshot преди и след
//! промяната, за да се хваща всяка подмяна на сума, контрагент или сметка.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{self, cell_i32_at, cell_str, lit, Db, Row};
use crate::form::{active_or_one, auth_sub, db_err};
use crate::identity;
use crate::pagination::{paginate, PageQuery};
use crate::{AppState, ApiError};

const SEL: &str = "SELECT id, company_id, user_id, user_login, user_name, entity_type, \
entity_id, entity_number, action, summary, old_amount, new_amount, old_data, new_data, \
created_at FROM audit_logs";

#[derive(Clone, Default)]
pub struct EntityStamp {
    pub created_by: String,
    pub created_at: String,
    pub updated_by: String,
    pub updated_at: String,
    pub edit_count: i32,
}

pub struct Actor {
    pub id: i32,
    pub login: String,
    pub name: String,
}

pub async fn resolve_actor(db: &Db, sub: &str) -> Actor {
    let login = sub.trim().to_string();
    if login.is_empty() {
        return Actor {
            id: 0,
            login: "system".into(),
            name: "Система".into(),
        };
    }
    if let Ok(Some(u)) = identity::by_login(db, &login).await {
        let name = if u.name.trim().is_empty() {
            u.email.clone()
        } else {
            format!("{} ({})", u.name.trim(), u.email.trim())
        };
        return Actor {
            id: u.id,
            login: u.username,
            name,
        };
    }
    Actor {
        id: 0,
        login: login.clone(),
        name: login,
    }
}

fn compact_json(v: Option<&Value>) -> String {
    let Some(val) = v else {
        return String::new();
    };
    if val.is_null() {
        return String::new();
    }
    let s = serde_json::to_string(val).unwrap_or_default();
    if s.len() > 16_000 {
        let mut cut: String = s.chars().take(16_000).collect();
        cut.push_str("...");
        cut
    } else {
        s
    }
}

/// Записва събитие в одитната следа. Грешка при логване се записва в
/// лога на сървъра, без да чупи основната операция, но на здрава база
/// винаги минава.
pub async fn record(
    db: &Db,
    company_id: i32,
    sub: &str,
    entity_type: &str,
    entity_id: i32,
    entity_number: &str,
    action: &str,
    summary: &str,
    old_amount: &str,
    new_amount: &str,
    old_data: Option<&Value>,
    new_data: Option<&Value>,
) {
    let actor = resolve_actor(db, sub).await;
    let Ok(id) = db.next_id("audit_logs").await else {
        tracing::warn!("audit: не мога да взема next_id за audit_logs");
        return;
    };
    let stamp = db::now_stamp();
    let old_json = compact_json(old_data);
    let new_json = compact_json(new_data);
    let sql = format!(
        "INSERT INTO audit_logs (id, company_id, user_id, user_login, user_name, entity_type, \
         entity_id, entity_number, action, summary, old_amount, new_amount, old_data, new_data, created_at) \
         VALUES ({id}, {company_id}, {}, {}, {}, {}, {entity_id}, {}, {}, {}, {}, {}, {}, {}, {})",
        actor.id,
        lit(&actor.login),
        lit(&actor.name),
        lit(entity_type),
        lit(entity_number),
        lit(action),
        lit(summary),
        lit(old_amount),
        lit(new_amount),
        lit(&old_json),
        lit(&new_json),
        lit(&stamp),
    );
    if let Err(e) = db.exec(&sql).await {
        tracing::warn!("audit: грешка при запис в audit_logs: {e}");
    }
}

fn parse_json_opt(raw: &str) -> Value {
    let t = raw.trim();
    if t.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(t).unwrap_or_else(|_| Value::String(t.to_string()))
}

fn row_json(r: &Row, with_snapshots: bool) -> Value {
    let old_amt = cell_str(r, 10);
    let new_amt = cell_str(r, 11);
    let amount_changed = !old_amt.is_empty() && !new_amt.is_empty() && old_amt != new_amt;
    let mut v = json!({
        "id": cell_i32_at(r, 0),
        "company_id": cell_i32_at(r, 1),
        "user_id": cell_i32_at(r, 2),
        "user_login": cell_str(r, 3),
        "user_name": cell_str(r, 4),
        "entity_type": cell_str(r, 5),
        "entity_id": cell_i32_at(r, 6),
        "entity_number": cell_str(r, 7),
        "action": cell_str(r, 8),
        "summary": cell_str(r, 9),
        "old_amount": old_amt,
        "new_amount": new_amt,
        "amount_changed": amount_changed,
        "created_at": cell_str(r, 14),
    });
    if with_snapshots {
        v["old_data"] = parse_json_opt(&cell_str(r, 12));
        v["new_data"] = parse_json_opt(&cell_str(r, 13));
    }
    v
}

/// Връща карта `entity_id -> EntityStamp` за даден тип обект във фирмата.
pub async fn stamps_for(
    db: &Db,
    company_id: i32,
    entity_type: &str,
) -> Result<HashMap<i32, EntityStamp>, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT entity_id, user_name, user_login, action, created_at FROM audit_logs \
             WHERE company_id = {company_id} AND entity_type = {} ORDER BY id",
            lit(entity_type)
        ))
        .await
        .map_err(db_err)?;
    let mut map: HashMap<i32, EntityStamp> = HashMap::new();
    for r in &rows {
        let eid = cell_i32_at(r, 0);
        if eid <= 0 {
            continue;
        }
        let uname = cell_str(r, 1);
        let ulogin = cell_str(r, 2);
        let who = if !uname.trim().is_empty() { uname } else { ulogin };
        let action = cell_str(r, 3);
        let when = cell_str(r, 4);
        let entry = map.entry(eid).or_default();
        if entry.created_at.is_empty() || action == "create" {
            if entry.created_at.is_empty() {
                entry.created_by = who.clone();
                entry.created_at = when.clone();
            }
        }
        entry.updated_by = who;
        entry.updated_at = when;
        if action != "create" {
            entry.edit_count += 1;
        }
    }
    Ok(map)
}

pub fn attach_stamp(v: &mut Value, stamp: Option<&EntityStamp>) {
    if let Some(s) = stamp {
        v["created_by"] = json!(s.created_by);
        v["created_at"] = json!(s.created_at);
        v["updated_by"] = json!(s.updated_by);
        v["updated_at"] = json!(s.updated_at);
        v["edit_count"] = json!(s.edit_count);
    } else {
        v["created_by"] = json!("");
        v["created_at"] = json!("");
        v["updated_by"] = json!("");
        v["updated_at"] = json!("");
        v["edit_count"] = json!(0);
    }
}

pub async fn history_for(
    db: &Db,
    company_id: i32,
    entity_type: &str,
    entity_id: i32,
) -> Result<Vec<Value>, ApiError> {
    let rows = db
        .query(&format!(
            "{SEL} WHERE company_id = {company_id} AND entity_type = {} AND entity_id = {entity_id} ORDER BY id DESC",
            lit(entity_type)
        ))
        .await
        .map_err(db_err)?;
    Ok(rows.iter().map(|r| row_json(r, true)).collect())
}

#[derive(Deserialize, Default)]
pub struct AuditQuery {
    pub entity_type: Option<String>,
    pub entity_id: Option<i32>,
    pub action: Option<String>,
    pub user: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub changed_only: Option<String>,
    pub q: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let rows = st
        .db
        .query(&format!("{SEL} WHERE company_id = {company} ORDER BY id DESC"))
        .await
        .map_err(db_err)?;

    let et_filter = q.entity_type.as_deref().unwrap_or("").trim();
    let eid_filter = q.entity_id.unwrap_or(0);
    let act_filter = q.action.as_deref().unwrap_or("").trim();
    let user_filter = q.user.as_deref().unwrap_or("").trim().to_lowercase();
    let from_filter = q.from.as_deref().unwrap_or("").trim();
    let to_filter = q.to.as_deref().unwrap_or("").trim();
    let changed_only = matches!(
        q.changed_only.as_deref().unwrap_or("").trim(),
        "1" | "true" | "yes"
    );
    let search = q.q.as_deref().unwrap_or("").trim().to_lowercase();

    let mut items = Vec::new();
    for r in &rows {
        let et = cell_str(r, 5);
        let eid = cell_i32_at(r, 6);
        let act = cell_str(r, 8);
        let ulogin = cell_str(r, 3);
        let uname = cell_str(r, 4);
        let eno = cell_str(r, 7);
        let sum = cell_str(r, 9);
        let old_amt = cell_str(r, 10);
        let new_amt = cell_str(r, 11);
        let created = cell_str(r, 14);
        let day = if created.len() >= 10 { &created[..10] } else { created.as_str() };

        if !et_filter.is_empty() && et != et_filter {
            continue;
        }
        if eid_filter > 0 && eid != eid_filter {
            continue;
        }
        if !act_filter.is_empty() && act != act_filter {
            continue;
        }
        if !user_filter.is_empty()
            && !ulogin.to_lowercase().contains(&user_filter)
            && !uname.to_lowercase().contains(&user_filter)
        {
            continue;
        }
        if !from_filter.is_empty() && day < from_filter {
            continue;
        }
        if !to_filter.is_empty() && day > to_filter {
            continue;
        }
        if changed_only {
            let diff = !old_amt.is_empty() && !new_amt.is_empty() && old_amt != new_amt;
            let risky = act == "update" || act == "delete" || act == "unpost";
            if !diff && !risky {
                continue;
            }
        }
        if !search.is_empty() {
            let hay = format!("{ulogin} {uname} {et} {eid} {eno} {act} {sum} {old_amt} {new_amt}")
                .to_lowercase();
            if !hay.contains(&search) {
                continue;
            }
        }
        items.push(row_json(r, true));
    }

    let page = PageQuery {
        limit: q.limit,
        offset: q.offset,
    };
    Ok(paginate(items, &page))
}

pub async fn entity_history(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((entity_type, entity_id)): Path<(String, i32)>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let items = history_for(&st.db, company, entity_type.trim(), entity_id).await?;
    Ok(Json(json!({
        "items": items,
        "count": items.len(),
    })))
}
