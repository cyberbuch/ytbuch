//! Начални салда. Не се пишат в дневника (салдо не е оборот).
//! Януари на годината трябва да е отворен.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit, Row};
use crate::form::{self, auth_sub};
use crate::invoice_money::{self, cents};
use crate::invoice_read;
use crate::periods;
use crate::{api_err, AppState, ApiError};

#[derive(Deserialize, Default)]
pub(crate) struct YearQ {
    year: Option<String>,
}

fn year_of(raw: Option<&str>) -> i32 {
    let n = raw.unwrap_or("").trim().parse::<i32>().unwrap_or(0);
    if n < 2000 { 2026 } else { n }
}

fn is_zero(raw: &str) -> bool {
    matches!(cents(raw), Some(0) | None)
}

pub fn natural_side(number: &str) -> &'static str {
    let d0 = number.get(..1).unwrap_or("");
    if d0 == "1" {
        return "credit";
    }
    if d0 == "2" {
        return if number.starts_with("24") { "credit" } else { "debit" };
    }
    if d0 == "3" {
        return "debit";
    }
    if d0 == "4" {
        if number.starts_with("4531") {
            return "debit";
        }
        if number.starts_with("4532") {
            return "credit";
        }
        if number.starts_with("40") {
            return "credit";
        }
        if number.starts_with("41") {
            return "debit";
        }
        return "any";
    }
    if d0 == "5" || d0 == "6" {
        return "debit";
    }
    if d0 == "7" {
        return "credit";
    }
    "any"
}

pub fn account_kind(number: &str) -> &'static str {
    let d0 = number.get(..1).unwrap_or("");
    match d0 {
        "1" => "capital",
        "2" => "fixed_asset",
        "3" => "inventory",
        "4" if number.starts_with("40") => "payable",
        "4" if number.starts_with("41") => "receivable",
        "4" => "settlement",
        "5" => "cash",
        "6" => "expense",
        "7" => "revenue",
        _ => "other",
    }
}

struct Acc {
    number: String,
    name: String,
    analytic: String,
}

fn req_i32(obj: &Map<String, Value>, key: &str) -> Result<i32, ApiError> {
    match form::i32_field(obj, key)? {
        Some(v) => Ok(v),
        None => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} е задължително"),
        )),
    }
}

fn opt_text(obj: &Map<String, Value>, key: &str, default: &str) -> Result<String, ApiError> {
    Ok(form::str_field(obj, key)?.unwrap_or_else(|| default.to_string()))
}

async fn load_row(db: &crate::db::Db, id: i32) -> Result<Row, ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid id"));
    }
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT id, fiscal_year, account_id, debit, credit, counterpart_id, product_id, quantity, description FROM opening_balances WHERE id = {id}"
        ),
    )
    .await?;
    rows.into_iter().next().ok_or_else(|| {
        api_err(StatusCode::NOT_FOUND, "Opening balance not found")
    })
}

fn row_json(row: &Row, acc: Option<&Acc>, cp_name: &str, pr_name: &str, pr_code: &str) -> Value {
    let account_id = cell_i32_at(row, 2);
    let counterpart_id = cell_i32_at(row, 5);
    let product_id = cell_i32_at(row, 6);
    let mut o = json!({
        "id": cell_i32_at(row, 0),
        "fiscal_year": cell_i32_at(row, 1),
        "account_id": account_id,
        "debit": cell_str(row, 3),
        "credit": cell_str(row, 4),
        "counterpart_id": counterpart_id,
        "product_id": product_id,
        "quantity": cell_str(row, 7),
        "description": cell_str(row, 8),
    });
    if let Some(a) = acc {
        o["account_number"] = json!(a.number);
        o["account_name"] = json!(a.name);
        o["analytic_type"] = json!(a.analytic);
        o["natural_side"] = json!(natural_side(&a.number));
        o["account_kind"] = json!(account_kind(&a.number));
    }
    if counterpart_id > 0 && !cp_name.is_empty() {
        o["counterpart_name"] = json!(cp_name);
    }
    if product_id > 0 && !pr_name.is_empty() {
        o["product_name"] = json!(pr_name);
        o["product_code"] = json!(pr_code);
    }
    o
}

async fn maps(db: &crate::db::Db) -> Result<(HashMap<i32, Acc>, HashMap<i32, String>, HashMap<i32, (String, String)>), ApiError> {
    let mut accounts = HashMap::new();
    for row in invoice_read::qry(db, "SELECT id, number, name, analytic_type FROM accounts").await? {
        accounts.insert(
            cell_i32_at(&row, 0),
            Acc {
                number: cell_str(&row, 1),
                name: cell_str(&row, 2),
                analytic: cell_str(&row, 3),
            },
        );
    }
    let mut cps = HashMap::new();
    for row in invoice_read::qry(db, "SELECT id, name FROM counterparts").await? {
        cps.insert(cell_i32_at(&row, 0), cell_str(&row, 1));
    }
    let mut prs = HashMap::new();
    for row in invoice_read::qry(db, "SELECT id, name, code FROM products").await? {
        prs.insert(cell_i32_at(&row, 0), (cell_str(&row, 1), cell_str(&row, 2)));
    }
    Ok((accounts, cps, prs))
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<YearQ>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let year = year_of(q.year.as_deref());
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, fiscal_year, account_id, debit, credit, counterpart_id, product_id, quantity, description FROM opening_balances WHERE fiscal_year = {year} ORDER BY id"
        ),
    )
    .await?;
    let (accounts, cps, prs) = maps(&st.db).await?;
    let mut items = Vec::new();
    let mut td: i64 = 0;
    let mut tc: i64 = 0;
    for row in &rows {
        let aid = cell_i32_at(row, 2);
        let cid = cell_i32_at(row, 5);
        let pid = cell_i32_at(row, 6);
        let (pn, pc) = prs.get(&pid).cloned().unwrap_or_default();
        items.push(row_json(
            row,
            accounts.get(&aid),
            cps.get(&cid).map(String::as_str).unwrap_or(""),
            &pn,
            &pc,
        ));
        if let Some(c) = cents(&cell_str(row, 3)) {
            td += c;
        }
        if let Some(c) = cents(&cell_str(row, 4)) {
            tc += c;
        }
    }
    Ok(Json(json!({
        "items": items,
        "count": items.len(),
        "fiscal_year": year,
        "total_debit": invoice_money::money(td),
        "total_credit": invoice_money::money(tc),
        "difference": invoice_money::money(td - tc),
    })))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let year = req_i32(obj, "fiscal_year")?;
    let account_id = req_i32(obj, "account_id")?;
    let debit = opt_text(obj, "debit", "0")?;
    let credit = opt_text(obj, "credit", "0")?;
    let counterpart_id = form::i32_field(obj, "counterpart_id")?.unwrap_or(0);
    let product_id = form::i32_field(obj, "product_id")?.unwrap_or(0);
    let quantity = opt_text(obj, "quantity", "0")?;
    let description = opt_text(obj, "description", "")?;
    if is_zero(&debit) && is_zero(&credit) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "въведете дебит или кредит"));
    }
    if !is_zero(&debit) && !is_zero(&credit) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "салдото е или дебит, или кредит — не и двете",
        ));
    }
    periods::assert_open(&st.db, &format!("{year}-01")).await?;
    let acc = invoice_read::qry(
        &st.db,
        &format!("SELECT number, analytic_type FROM accounts WHERE id = {account_id}"),
    )
    .await?;
    let Some(acc) = acc.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Account not found"));
    };
    let number = cell_str(acc, 0);
    let analytic = cell_str(acc, 1);
    let side = natural_side(&number);
    if side == "debit" && !is_zero(&credit) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{number} е дебитна по НСС — началното салдо е в дебит"),
        ));
    }
    if side == "credit" && !is_zero(&debit) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{number} е кредитна по НСС — началното салдо е в кредит"),
        ));
    }
    if analytic == "counterpart" && counterpart_id <= 0 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{number} е аналитична към контрагент"),
        ));
    }
    if analytic == "product" && product_id <= 0 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{number} е аналитична към продукт"),
        ));
    }
    let id = invoice_read::nid(&st.db, "opening_balances").await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO opening_balances (id, fiscal_year, account_id, debit, credit, counterpart_id, product_id, quantity, description) VALUES ({id}, {year}, {account_id}, {}, {}, {counterpart_id}, {product_id}, {}, {})",
            lit(&debit),
            lit(&credit),
            lit(&quantity),
            lit(&description)
        ),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "natural_side": side,
            "account_kind": account_kind(&number),
        })),
    ))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let row = load_row(&st.db, id).await?;
    let year = cell_i32_at(&row, 1);
    periods::assert_open(&st.db, &format!("{year}-01")).await?;
    let mut sets = Vec::new();
    if let Some(v) = form::str_field(obj, "debit")? {
        sets.push(format!("debit = {}", lit(&v)));
    }
    if let Some(v) = form::str_field(obj, "credit")? {
        sets.push(format!("credit = {}", lit(&v)));
    }
    if let Some(v) = form::str_field(obj, "description")? {
        sets.push(format!("description = {}", lit(&v)));
    }
    if let Some(v) = form::str_field(obj, "quantity")? {
        sets.push(format!("quantity = {}", lit(&v)));
    }
    if let Some(v) = form::i32_field(obj, "counterpart_id")? {
        sets.push(format!("counterpart_id = {v}"));
    }
    if let Some(v) = form::i32_field(obj, "product_id")? {
        sets.push(format!("product_id = {v}"));
    }
    if !sets.is_empty() {
        invoice_read::exe(
            &st.db,
            &format!("UPDATE opening_balances SET {} WHERE id = {id}", sets.join(", ")),
        )
        .await?;
    }
    let row = load_row(&st.db, id).await?;
    let (accounts, cps, prs) = maps(&st.db).await?;
    let aid = cell_i32_at(&row, 2);
    let cid = cell_i32_at(&row, 5);
    let pid = cell_i32_at(&row, 6);
    let (pn, pc) = prs.get(&pid).cloned().unwrap_or_default();
    Ok(Json(row_json(
        &row,
        accounts.get(&aid),
        cps.get(&cid).map(String::as_str).unwrap_or(""),
        &pn,
        &pc,
    )))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    auth_sub(&st, &headers)?;
    let row = load_row(&st.db, id).await?;
    let year = cell_i32_at(&row, 1);
    periods::assert_open(&st.db, &format!("{year}-01")).await?;
    invoice_read::exe(&st.db, &format!("DELETE FROM opening_balances WHERE id = {id}")).await?;
    Ok(StatusCode::NO_CONTENT)
}
