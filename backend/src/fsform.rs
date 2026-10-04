//! Формули за финансовите отчети. Каталогът е в кода, записът е по фирма.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::form::{self, active_or_one, auth_sub};
use crate::reports_fs;
use crate::reports_nom;
use crate::{api_err, AppState, ApiError};

pub const SQL: &str = "\
CREATE TABLE IF NOT EXISTS fs_line_formulas (\
    id INTEGER PRIMARY KEY,\
    company_id INTEGER,\
    statement VARCHAR,\
    line_code VARCHAR,\
    formula VARCHAR\
)";

#[derive(Deserialize)]
pub struct StQ {
    statement: Option<String>,
}

pub async fn lines(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<StQ>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let statement = q.statement.as_deref().unwrap_or("").trim();
    if statement.is_empty() {
        let items: Vec<Value> = reports_nom::statements()
            .into_iter()
            .map(|(id, name)| json!({ "id": id, "name": name }))
            .collect();
        let count = items.len();
        return Ok(Json(json!({ "items": items, "count": count })));
    }
    if !reports_nom::known(statement) {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            "statement трябва да е balance/pl/cashflow/equity",
        ));
    }
    let items: Vec<Value> = reports_nom::lines(statement)
        .into_iter()
        .map(|l| {
            json!({
                "code": l.code,
                "parent": l.parent,
                "kind": l.kind,
                "side": l.side,
                "name": l.name,
            })
        })
        .collect();
    let count = items.len();
    Ok(Json(json!({ "statement": statement, "items": items, "count": count })))
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<StQ>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let statement = known_get(q.statement.as_deref().unwrap_or(""))?;
    let rows = rows_of(&st, company, statement).await?;
    let items: Vec<Value> = rows.iter().map(row_json).collect();
    let count = items.len();
    Ok(Json(json!({ "statement": statement, "items": items, "count": count })))
}

pub async fn put(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let statement_owned = req(obj, "statement")?;
    let statement = known_write(&statement_owned)?;
    let code = req(obj, "line_code")?;
    if code.is_empty() || !reports_nom::has_code(statement, &code) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "непозната статия"));
    }
    let formula = present(obj, "formula")?;
    if let Err(e) = reports_fs::check(&formula) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, e));
    }
    let rows = rows_of(&st, company, statement).await?;
    if let Some(row) = rows.iter().find(|r| cell_str(r, 3) == code) {
        let id = cell_i32_at(row, 0);
        crate::invoice_read::exe(
            &st.db,
            &format!("UPDATE fs_line_formulas SET formula = {} WHERE id = {id}", lit(&formula)),
        )
        .await?;
        return Ok(Json(one(&st, id).await?));
    }
    let id = crate::invoice_read::nid(&st.db, "fs_line_formulas").await?;
    crate::invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO fs_line_formulas (id, company_id, statement, line_code, formula) VALUES ({id}, {company}, {}, {}, {})",
            lit(statement),
            lit(&code),
            lit(&formula)
        ),
    )
    .await?;
    Ok(Json(one(&st, id).await?))
}

pub async fn defaults(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let statement_owned = req(obj, "statement")?;
    let statement = known_write(&statement_owned)?;
    let rows = rows_of(&st, company, statement).await?;
    if !rows.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "вече има формули за този отчет. Изтрийте ги първо.",
        ));
    }
    let defs = reports_nom::defaults(statement);
    for (code, formula) in &defs {
        let id = crate::invoice_read::nid(&st.db, "fs_line_formulas").await?;
        crate::invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO fs_line_formulas (id, company_id, statement, line_code, formula) VALUES ({id}, {company}, {}, {}, {})",
                lit(statement),
                lit(code),
                lit(formula)
            ),
        )
        .await?;
    }
    Ok(Json(json!({ "statement": statement, "count": defs.len() })))
}

pub async fn clear(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let statement_owned = req(obj, "statement")?;
    let statement = known_write(&statement_owned)?;
    let rows = rows_of(&st, company, statement).await?;
    let n = rows.len();
    crate::invoice_read::exe(
        &st.db,
        &format!(
            "DELETE FROM fs_line_formulas WHERE company_id = {company} AND statement = {}",
            lit(statement)
        ),
    )
    .await?;
    Ok(Json(json!({ "statement": statement, "count": n })))
}

pub async fn validate(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let formula = present(obj, "formula")?;
    match reports_fs::check(&formula) {
        Ok(tokens) => Ok(Json(json!({ "ok": 1, "error": "", "tokens": tokens }))),
        Err(e) => Ok(Json(json!({ "ok": 0, "error": e, "tokens": 0 }))),
    }
}

async fn company(st: &AppState, headers: &HeaderMap) -> Result<i32, ApiError> {
    let sub = auth_sub(st, headers)?;
    let id = active_or_one(st, &sub).await?;
    form::ensure_company(&st.db, id).await?;
    Ok(id)
}

fn known_get(raw: &str) -> Result<&str, ApiError> {
    let statement = raw.trim();
    if reports_nom::known(statement) {
        Ok(statement)
    } else {
        Err(api_err(
            StatusCode::BAD_REQUEST,
            "statement трябва да е balance/pl/cashflow/equity",
        ))
    }
}

fn known_write(statement: &str) -> Result<&str, ApiError> {
    if reports_nom::known(statement) {
        Ok(statement)
    } else {
        Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "statement трябва да е balance/pl/cashflow/equity",
        ))
    }
}

fn req(obj: &serde_json::Map<String, Value>, key: &str) -> Result<String, ApiError> {
    match form::str_field(obj, key)? {
        Some(s) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        _ => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} е задължително"),
        )),
    }
}

fn present(obj: &serde_json::Map<String, Value>, key: &str) -> Result<String, ApiError> {
    match form::str_field(obj, key)? {
        Some(s) => Ok(s.trim().to_string()),
        None => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} е задължително"),
        )),
    }
}

async fn rows_of(st: &AppState, company: i32, statement: &str) -> Result<Vec<crate::db::Row>, ApiError> {
    crate::invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, company_id, statement, line_code, formula FROM fs_line_formulas WHERE company_id = {company} AND statement = {} ORDER BY id",
            lit(statement)
        ),
    )
    .await
}

fn row_json(row: &crate::db::Row) -> Value {
    json!({
        "id": cell_i32_at(row, 0),
        "company_id": cell_i32_at(row, 1),
        "statement": cell_str(row, 2),
        "line_code": cell_str(row, 3),
        "formula": cell_str(row, 4),
    })
}

async fn one(st: &AppState, id: i32) -> Result<Value, ApiError> {
    let rows = crate::invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, company_id, statement, line_code, formula FROM fs_line_formulas WHERE id = {id}"
        ),
    )
    .await?;
    rows.first().map(row_json).ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "формулата не се записа"))
}
