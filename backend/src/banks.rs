//! Банкови сметки. Салдото е низ с два знака.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, money};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_read;
use crate::pagination::{paginate, PageQuery};
use crate::{api_err, AppState, ApiError};

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id, iban, bic, bank_name, currency, name, account_number, gl_account_id, buffer_account_id, balance, company_id FROM bank_accounts WHERE company_id = {company} ORDER BY name, id"),
    )
    .await?;
    let mut items = Vec::new();
    for row in &rows {
        items.push(one(&st, row).await?);
    }
    Ok(paginate(items, &q))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let name = form::required(form::str_field(obj, "name")?, "name")?;
    let iban = form::required(form::str_field(obj, "iban")?, "iban")?;
    let balance = norm_balance(opt(obj, "balance", "0")?)?;
    let gl = form::i32_field(obj, "gl_account_id")?.unwrap_or(0);
    let buf = form::i32_field(obj, "buffer_account_id")?.unwrap_or(0);
    form::ensure_ref(&st.db, "accounts", gl, company, "сметката").await?;
    form::ensure_ref(&st.db, "accounts", buf, company, "сметката").await?;
    let dup = invoice_read::qry(
        &st.db,
        &format!("SELECT id FROM bank_accounts WHERE company_id = {company} AND iban = {}", lit(&iban)),
    )
    .await?;
    if !dup.is_empty() {
        return Err(api_err(StatusCode::CONFLICT, "IBAN вече съществува"));
    }
    let id = invoice_read::nid(&st.db, "bank_accounts").await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO bank_accounts (id, company_id, iban, bic, bank_name, currency, name, account_number, gl_account_id, buffer_account_id, balance) \
             VALUES ({id}, {company}, {}, {}, {}, {}, {}, {}, {gl}, {buf}, {})",
            lit(&iban),
            lit(&opt(obj, "bic", "")?),
            lit(&opt(obj, "bank_name", "")?),
            lit(&opt(obj, "currency", "BGN")?),
            lit(&name),
            lit(&opt(obj, "account_number", "")?),
            lit(&balance)
        ),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(load(&st, id, company).await?)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    need(id)?;
    let cur = load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let name = form::str_field(obj, "name")?.unwrap_or_else(|| cur["name"].as_str().unwrap_or("").into());
    let iban = form::str_field(obj, "iban")?.unwrap_or_else(|| cur["iban"].as_str().unwrap_or("").into());
    if name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    if iban.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "iban е задължително"));
    }
    let balance = match form::str_field(obj, "balance")? {
        Some(s) => norm_balance(s)?,
        None => cur["balance"].as_str().unwrap_or("0.00").to_string(),
    };
    let gl = form::i32_field(obj, "gl_account_id")?.unwrap_or(cur["gl_account_id"].as_i64().unwrap_or(0) as i32);
    let buf = form::i32_field(obj, "buffer_account_id")?.unwrap_or(cur["buffer_account_id"].as_i64().unwrap_or(0) as i32);
    form::ensure_ref(&st.db, "accounts", gl, company, "сметката").await?;
    form::ensure_ref(&st.db, "accounts", buf, company, "сметката").await?;
    let bic = form::str_field(obj, "bic")?.unwrap_or_else(|| cur["bic"].as_str().unwrap_or("").into());
    let bank_name = form::str_field(obj, "bank_name")?.unwrap_or_else(|| cur["bank_name"].as_str().unwrap_or("").into());
    let currency = form::str_field(obj, "currency")?.unwrap_or_else(|| cur["currency"].as_str().unwrap_or("BGN").into());
    let account_number = form::str_field(obj, "account_number")?.unwrap_or_else(|| cur["account_number"].as_str().unwrap_or("").into());
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE bank_accounts SET name = {}, iban = {}, bic = {}, bank_name = {}, currency = {}, account_number = {}, balance = {}, gl_account_id = {gl}, buffer_account_id = {buf} WHERE id = {id}",
            lit(&name), lit(&iban), lit(&bic), lit(&bank_name), lit(&currency), lit(&account_number), lit(&balance)
        ),
    )
    .await?;
    Ok(Json(load(&st, id, company).await?))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let company = company(&st, &headers).await?;
    need(id)?;
    let _ = load(&st, id, company).await?;
    invoice_read::exe(&st.db, &format!("DELETE FROM bank_accounts WHERE id = {id}")).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id, iban, bic, bank_name, currency, name, account_number, gl_account_id, buffer_account_id, balance, company_id FROM bank_accounts WHERE id = {id}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Bank account not found"));
    };
    if cell_i32_at(row, 10) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "Bank account not found"));
    }
    one(st, row).await
}

async fn one(st: &AppState, row: &crate::db::Row) -> Result<Value, ApiError> {
    let gl = cell_i32_at(row, 7);
    let buf = cell_i32_at(row, 8);
    let owner = cell_i32_at(row, 10);
    let mut o = json!({
        "id": cell_i32_at(row, 0),
        "iban": cell_str(row, 1),
        "bic": cell_str(row, 2),
        "bank_name": cell_str(row, 3),
        "currency": cell_str(row, 4),
        "name": cell_str(row, 5),
        "account_number": cell_str(row, 6),
        "gl_account_id": gl,
        "buffer_account_id": buf,
        "balance": cell_str(row, 9),
        "company_id": owner,
    });
    if let Some(code) = code(st, gl, owner).await? {
        o["gl_account_code"] = json!(code);
    }
    if let Some(code) = code(st, buf, owner).await? {
        o["buffer_account_code"] = json!(code);
    }
    Ok(o)
}

async fn code(st: &AppState, id: i32, company: i32) -> Result<Option<String>, ApiError> {
    if id <= 0 {
        return Ok(None);
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT number FROM accounts WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    Ok(rows.first().map(|r| cell_str(r, 0)).filter(|s| !s.is_empty()))
}

fn opt(obj: &Map<String, Value>, key: &str, default: &str) -> Result<String, ApiError> {
    Ok(form::str_field(obj, key)?.unwrap_or_else(|| default.to_string()))
}

fn norm_balance(raw: String) -> Result<String, ApiError> {
    let Some(v) = dec::parse(&raw) else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидно начално салдо"));
    };
    Ok(money(v))
}

fn need(id: i32) -> Result<(), ApiError> {
    if id <= 0 { Err(api_err(StatusCode::BAD_REQUEST, "invalid id")) } else { Ok(()) }
}

async fn company(st: &AppState, headers: &HeaderMap) -> Result<i32, ApiError> {
    let sub = auth_sub(st, headers)?;
    let id = active_or_one(st, &sub).await?;
    if id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "няма избрана активна фирма"));
    }
    Ok(id)
}
