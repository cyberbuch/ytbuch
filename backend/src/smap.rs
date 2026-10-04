//! Дт/Кт кореспонденции към тип движение в SAF-T. По активната фирма.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit, Db};
use crate::form::{self, active_or_one, auth_sub, db_err};
use crate::saft;
use crate::smap_def;
use crate::{api_err, AppState, ApiError};

#[derive(Deserialize)]
pub struct KindQuery {
    kind: Option<String>,
}

fn kind_ok(kind: &str) -> bool {
    matches!(kind, "stock" | "asset" | "cash")
}

fn type_rows(kind: &str) -> Vec<String> {
    if kind == "cash" {
        return smap_def::CASH_TYPES.iter().map(|s| (*s).to_string()).collect();
    }
    let key = if kind == "asset" { "asset_movements" } else { "stock_movements" };
    saft::catalog()
        .get(key)
        .cloned()
        .unwrap_or_default()
}

fn type_known(kind: &str, code: &str) -> bool {
    type_rows(kind).iter().any(|line| line.split('|').next() == Some(code))
}

fn type_name(kind: &str, code: &str) -> String {
    type_rows(kind)
        .iter()
        .find_map(|line| {
            let mut p = line.splitn(3, '|');
            let c = p.next().unwrap_or("");
            let name = p.next().unwrap_or("");
            (c == code).then(|| name.to_string())
        })
        .unwrap_or_else(|| code.to_string())
}

fn types_json(kind: &str) -> Vec<Value> {
    type_rows(kind)
        .iter()
        .map(|line| {
            let mut p = line.splitn(3, '|');
            json!({
                "code": p.next().unwrap_or(""),
                "name": p.next().unwrap_or(""),
                "name_en": p.next().unwrap_or(""),
            })
        })
        .collect()
}

fn row_json(row: &crate::db::Row) -> Value {
    let kind = cell_str(row, 2);
    let code = cell_str(row, 3);
    json!({
        "id": cell_i32_at(row, 0),
        "kind": kind,
        "type_code": code,
        "type_name": type_name(&kind, &code),
        "debit_account": cell_str(row, 4),
        "credit_account": cell_str(row, 5),
        "debit_analytical": cell_str(row, 6),
        "credit_analytical": cell_str(row, 7),
        "description": cell_str(row, 8),
        "is_active": cell_i32_at(row, 9),
    })
}

const SEL: &str = "SELECT id, company_id, kind, type_code, debit_account, credit_account, debit_analytical, credit_analytical, description, is_active FROM saft_account_mappings";

async fn one(db: &Db, id: i32, company: i32) -> Result<Value, ApiError> {
    let rows = db
        .query(&format!("{SEL} WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "кореспонденцията не е намерена"));
    };
    if cell_i32_at(row, 1) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "кореспонденцията не е намерена"));
    }
    Ok(row_json(row))
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<KindQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let kind = q.kind.unwrap_or_default();
    let kind = kind.trim();
    if !kind_ok(kind) {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            "непознат вид кореспонденция (kind=stock|asset|cash)",
        ));
    }
    let company = active_or_one(&st, &sub).await?;
    let rows = st
        .db
        .query(&format!("{SEL} WHERE company_id = {company} AND kind = {}", lit(kind)))
        .await
        .map_err(db_err)?;
    let items: Vec<Value> = rows.iter().map(row_json).collect();
    let count = items.len();
    Ok(Json(json!({
        "kind": kind,
        "items": items,
        "count": count,
        "types": types_json(kind),
    })))
}

fn accounts(obj: &serde_json::Map<String, Value>) -> Result<(String, String, String, String, String), ApiError> {
    let debit = form::required(form::str_field(obj, "debit_account")?, "debit_account")?;
    let credit = form::required(form::str_field(obj, "credit_account")?, "credit_account")?;
    if debit.trim().is_empty() || credit.trim().is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "дебит и кредит сметките са задължителни",
        ));
    }
    let dan = form::str_field(obj, "debit_analytical")?.unwrap_or_default();
    let kan = form::str_field(obj, "credit_analytical")?.unwrap_or_default();
    let desc = form::str_field(obj, "description")?.unwrap_or_default();
    Ok((
        debit.trim().to_string(),
        credit.trim().to_string(),
        dan.trim().to_string(),
        kan.trim().to_string(),
        desc.trim().to_string(),
    ))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let kind = form::required(form::str_field(obj, "kind")?, "kind")?;
    let kind = kind.trim();
    if !kind_ok(kind) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "непознат вид кореспонденция (kind=stock|asset|cash)",
        ));
    }
    let code = form::required(form::str_field(obj, "type_code")?, "type_code")?;
    let code = code.trim();
    if !type_known(kind, code) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "непознат тип движение за този вид",
        ));
    }
    let (debit, credit, dan, kan, desc) = accounts(obj)?;
    let company = active_or_one(&st, &sub).await?;
    let id = st.db.next_id("saft_account_mappings").await.map_err(db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO saft_account_mappings (id, company_id, kind, type_code, debit_account, credit_account, debit_analytical, credit_analytical, description, is_active) \
             VALUES ({id}, {company}, {}, {}, {}, {}, {}, {}, {}, 1)",
            lit(kind), lit(code), lit(&debit), lit(&credit), lit(&dan), lit(&kan), lit(&desc)
        ))
        .await
        .map_err(db_err)?;
    Ok((StatusCode::CREATED, Json(one(&st.db, id, company).await?)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let current = one(&st.db, id, company).await?;
    let kind = current["kind"].as_str().unwrap_or("");
    let obj = form::object(&body)?;
    if let Some(code) = form::str_field(obj, "type_code")? {
        let code = code.trim();
        if !type_known(kind, code) {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "непознат тип движение за този вид",
            ));
        }
        st.db
            .exec(&format!("UPDATE saft_account_mappings SET type_code = {} WHERE id = {id}", lit(code)))
            .await
            .map_err(db_err)?;
    }
    for (field, col) in [
        ("debit_account", "debit_account"),
        ("credit_account", "credit_account"),
        ("debit_analytical", "debit_analytical"),
        ("credit_analytical", "credit_analytical"),
        ("description", "description"),
    ] {
        if let Some(v) = form::str_field(obj, field)? {
            if (field == "debit_account" || field == "credit_account") && v.trim().is_empty() {
                return Err(api_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "дебит и кредит сметките са задължителни",
                ));
            }
            st.db
                .exec(&format!(
                    "UPDATE saft_account_mappings SET {col} = {} WHERE id = {id}",
                    lit(v.trim())
                ))
                .await
                .map_err(db_err)?;
        }
    }
    if let Some(flag) = form::flag_field(obj, "is_active")? {
        st.db
            .exec(&format!("UPDATE saft_account_mappings SET is_active = {flag} WHERE id = {id}"))
            .await
            .map_err(db_err)?;
    }
    Ok(Json(one(&st.db, id, company).await?))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    one(&st.db, id, company).await?;
    st.db
        .exec(&format!("DELETE FROM saft_account_mappings WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn defaults(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let kind = form::required(form::str_field(obj, "kind")?, "kind")?;
    let kind = kind.trim();
    if !kind_ok(kind) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "непознат вид кореспонденция (kind=stock|asset|cash)",
        ));
    }
    let company = active_or_one(&st, &sub).await?;
    let have = st
        .db
        .query(&format!(
            "SELECT id FROM saft_account_mappings WHERE company_id = {company} AND kind = {}",
            lit(kind)
        ))
        .await
        .map_err(db_err)?;
    if !have.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "вече има кореспонденции за този вид. Изтрийте ги първо.",
        ));
    }
    let mut n = 0;
    let mut made = Vec::new();
    for line in smap_def::rows(kind) {
        let mut p = line.splitn(4, '|');
        let code = p.next().unwrap_or("");
        let debit = p.next().unwrap_or("");
        let credit = p.next().unwrap_or("");
        let desc = p.next().unwrap_or("");
        if code.is_empty() {
            continue;
        }
        let id = st.db.next_id("saft_account_mappings").await.map_err(db_err)?;
        if let Err(e) = st.db.exec(&format!(
            "INSERT INTO saft_account_mappings (id, company_id, kind, type_code, debit_account, credit_account, debit_analytical, credit_analytical, description, is_active) \
             VALUES ({id}, {company}, {}, {}, {}, {}, '', '', {}, 1)",
            lit(kind), lit(code), lit(debit), lit(credit), lit(desc)
        )).await {
            for old in &made {
                let _ = st.db.exec(&format!("DELETE FROM saft_account_mappings WHERE id = {old}")).await;
            }
            return Err(db_err(e));
        }
        made.push(id);
        n += 1;
    }
    Ok(Json(json!({ "kind": kind, "count": n })))
}
