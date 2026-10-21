//! Ръчно изписване на материали към разходна сметка от правилата.
//! Себестойността се смята при преизчислението, не се въвежда.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, qty};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_read;
use crate::pagination::{paginate, PageQuery};
use crate::periods;
use crate::stock_replay;
use crate::stock_rules::{self, Rule};
use crate::{api_err, AppState, ApiError};

const SEL: &str = "SELECT id, number, doc_date, location_id, status, notes, journal_entry_id, company_id FROM material_issues";

struct LineIn {
    product_id: i32,
    quantity: String,
    account_id: i32,
    inventory_account_id: i32,
}

pub async fn index(State(st): State<AppState>, headers: HeaderMap, Query(q): Query<PageQuery>) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let stamps = crate::audit::stamps_for(&st.db, company, "material_issue").await?;
    let rows = invoice_read::qry(&st.db, &format!("{SEL} WHERE company_id = {company} ORDER BY id")).await?;
    let mut items = Vec::new();
    for row in &rows {
        let mut item = one(&st, row, false).await?;
        let id = cell_i32_at(row, 0);
        crate::audit::attach_stamp(&mut item, stamps.get(&id));
        items.push(item);
    }
    Ok(paginate(items, &q))
}

pub async fn show(State(st): State<AppState>, headers: HeaderMap, Path(id): Path<i32>) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    Ok(Json(load_with_audit(&st, id, company).await?))
}

async fn load_with_audit(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let mut v = load(st, id, company).await?;
    let stamps = crate::audit::stamps_for(&st.db, company, "material_issue").await?;
    crate::audit::attach_stamp(&mut v, stamps.get(&id));
    v["audit_history"] = json!(crate::audit::history_for(&st.db, company, "material_issue", id).await?);
    Ok(v)
}

fn mat_total(doc: &Value) -> String {
    let mut sum = 0i128;
    if let Some(arr) = doc.get("lines").and_then(|v| v.as_array()) {
        for l in arr {
            let amt = l.get("amount").and_then(|v| v.as_str()).unwrap_or("0");
            sum = dec::add(sum, dec::or0(amt)).unwrap_or(sum);
        }
    }
    dec::money(sum)
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let lines = parse_lines(&st, company, obj).await?;
    if lines.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "документът няма редове"));
    }
    let id = invoice_read::nid(&st.db, "material_issues").await?;
    let number = format!("МИ-{id}");
    let date = form::str_field(obj, "doc_date")?.unwrap_or_default();
    let loc = form::i32_field(obj, "location_id")?.unwrap_or(0);
    form::ensure_ref(&st.db, "company_locations", loc, company, "обектът").await?;
    let notes = form::str_field(obj, "notes")?.unwrap_or_default();
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO material_issues (id, company_id, number, doc_date, location_id, status, notes, journal_entry_id) \
             VALUES ({id}, {company}, {}, {}, {loc}, 'draft', {}, 0)",
            lit(&number),
            lit(&date),
            lit(&notes)
        ),
    )
    .await?;
    if let Err(e) = insert_lines(&st, id, &lines).await {
        let _ = wipe(&st, id).await;
        return Err(e);
    }
    let after = load(&st, id, company).await?;
    let summary = format!("Създаден документ за изписване на материали {number} ({date})");
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "material_issue",
        id,
        &number,
        "create",
        &summary,
        "",
        "0.00",
        None,
        Some(&after),
    )
    .await;
    Ok((StatusCode::CREATED, Json(load_with_audit(&st, id, company).await?)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = company(&st, &headers).await?;
    let cur = load(&st, id, company).await?;
    draft(&cur)?;
    let obj = form::object(&body)?;
    let date = form::str_field(obj, "doc_date")?.unwrap_or_else(|| text(&cur, "doc_date"));
    let loc = form::i32_field(obj, "location_id")?.unwrap_or(int_of(&cur, "location_id"));
    form::ensure_ref(&st.db, "company_locations", loc, company, "обектът").await?;
    let notes = form::str_field(obj, "notes")?.unwrap_or_else(|| text(&cur, "notes"));
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE material_issues SET doc_date = {}, location_id = {loc}, notes = {} WHERE id = {id}",
            lit(&date),
            lit(&notes)
        ),
    )
    .await?;
    if obj.contains_key("lines") {
        let lines = parse_lines(&st, company, obj).await?;
        invoice_read::exe(&st.db, &format!("DELETE FROM material_issue_lines WHERE doc_id = {id}")).await?;
        insert_lines(&st, id, &lines).await?;
    }
    let after = load(&st, id, company).await?;
    let number = text(&after, "number");
    let summary = format!("Редактиран документ за изписване на материали {number} ({date})");
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "material_issue",
        id,
        &number,
        "update",
        &summary,
        &mat_total(&cur),
        &mat_total(&after),
        Some(&cur),
        Some(&after),
    )
    .await;
    Ok(Json(load_with_audit(&st, id, company).await?))
}

pub async fn delete(State(st): State<AppState>, headers: HeaderMap, Path(id): Path<i32>) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = company(&st, &headers).await?;
    let cur = load(&st, id, company).await?;
    draft(&cur)?;
    let number = text(&cur, "number");
    wipe(&st, id).await?;
    let summary = format!("Изтрит документ за изписване на материали {number}");
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "material_issue",
        id,
        &number,
        "delete",
        &summary,
        &mat_total(&cur),
        "",
        Some(&cur),
        None,
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn confirm(State(st): State<AppState>, headers: HeaderMap, Path(id): Path<i32>) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = company(&st, &headers).await?;
    let cur = load(&st, id, company).await?;
    draft(&cur)?;
    let loc = int_of(&cur, "location_id");
    if loc <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "нужен е складов обект"));
    }
    let date = text(&cur, "doc_date");
    periods::assert_open(&st.db, company, &date).await?;
    let lines = cur.get("lines").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if lines.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "документът няма редове"));
    }
    let rules = stock_rules::load(&st.db, company).await?;
    for line in &lines {
        let debit = line.get("account_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let credit = line.get("inventory_account_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        if stock_rules::pair(&rules, "material", credit, debit).is_none() {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "редът не е по правило за материали"));
        }
    }
    invoice_read::exe(&st.db, &format!("UPDATE material_issues SET status = 'confirmed' WHERE id = {id}")).await?;
    let out = match stock_replay::rebuild(&st.db, company, "9999-12-31").await {
        Ok(v) => v,
        Err(e) => {
            let _ = invoice_read::exe(&st.db, &format!("UPDATE material_issues SET status = 'draft' WHERE id = {id}")).await;
            return Err(e);
        }
    };
    let after = load(&st, id, company).await?;
    let number = text(&after, "number");
    let tot = mat_total(&after);
    let summary = format!("Потвърдено изписване на материали {number} ({date}), сума {tot}");
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "material_issue",
        id,
        &number,
        "post",
        &summary,
        "0.00",
        &tot,
        Some(&cur),
        Some(&after),
    )
    .await;
    let mut body = load_with_audit(&st, id, company).await?;
    body["warnings"] = json!(out.warnings);
    Ok(Json(body))
}

pub async fn unpost(State(st): State<AppState>, headers: HeaderMap, Path(id): Path<i32>) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = company(&st, &headers).await?;
    let cur = load(&st, id, company).await?;
    if text(&cur, "status") != "confirmed" {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "документът не е потвърден"));
    }
    periods::assert_open(&st.db, company, &text(&cur, "doc_date")).await?;
    let old_tot = mat_total(&cur);
    let entry = int_of(&cur, "journal_entry_id");
    invoice_read::wipe_journal(&st.db, entry).await?;
    invoice_read::exe(
        &st.db,
        &format!("UPDATE material_issue_lines SET unit_cost = '0.000000', amount = '0.00' WHERE doc_id = {id}"),
    )
    .await?;
    invoice_read::exe(&st.db, &format!("UPDATE material_issues SET status = 'draft', journal_entry_id = 0 WHERE id = {id}")).await?;
    let out = stock_replay::rebuild(&st.db, company, "9999-12-31").await?;
    let after = load(&st, id, company).await?;
    let number = text(&after, "number");
    let summary = format!("Върнато в чернова изписване на материали {number}, стара сума {old_tot}");
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "material_issue",
        id,
        &number,
        "unpost",
        &summary,
        &old_tot,
        "0.00",
        Some(&cur),
        Some(&after),
    )
    .await;
    let mut body = load_with_audit(&st, id, company).await?;
    body["warnings"] = json!(out.warnings);
    Ok(Json(body))
}

async fn parse_lines(st: &AppState, company: i32, obj: &serde_json::Map<String, Value>) -> Result<Vec<LineIn>, ApiError> {
    let Some(arr) = obj.get("lines").and_then(|v| v.as_array()) else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "липсват редове"));
    };
    let rules = stock_rules::load(&st.db, company).await?;
    let mut out = Vec::new();
    for line in arr {
        let Some(map) = line.as_object() else { continue };
        let product_id = form::i32_field(map, "product_id")?.unwrap_or(0);
        let quantity = qty_of(&form::required(form::str_field(map, "quantity")?, "quantity")?)?;
        if dec::or0(&quantity) <= 0 || product_id <= 0 {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "редът има артикул и положително количество"));
        }
        let account_id = form::i32_field(map, "account_id")?.unwrap_or(0);
        let inventory_account_id = form::i32_field(map, "inventory_account_id")?.unwrap_or(0);
        form::ensure_ref(&st.db, "products", product_id, company, "артикулът").await?;
        form::ensure_ref(&st.db, "accounts", account_id, company, "записът").await?;
        form::ensure_ref(&st.db, "accounts", inventory_account_id, company, "записът").await?;
        if !rule_ok(&rules, inventory_account_id, account_id) {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "сметката не е в правилата за материали"));
        }
        out.push(LineIn { product_id, quantity, account_id, inventory_account_id });
    }
    Ok(out)
}

fn rule_ok(rules: &[Rule], inventory: i32, offset: i32) -> bool {
    stock_rules::pair(rules, "material", inventory, offset).is_some()
}

fn qty_of(raw: &str) -> Result<String, ApiError> {
    match dec::parse(raw) {
        Some(v) => Ok(qty(v)),
        None => Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидно количество")),
    }
}

async fn insert_lines(st: &AppState, doc: i32, lines: &[LineIn]) -> Result<(), ApiError> {
    for (i, line) in lines.iter().enumerate() {
        let id = invoice_read::nid(&st.db, "material_issue_lines").await?;
        invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO material_issue_lines (id, doc_id, product_id, quantity, unit_cost, amount, account_id, inventory_account_id, line_no) \
                 VALUES ({id}, {doc}, {}, {}, '0.000000', '0.00', {}, {}, {})",
                line.product_id,
                lit(&line.quantity),
                line.account_id,
                line.inventory_account_id,
                i as i32 + 1
            ),
        )
        .await?;
    }
    Ok(())
}

async fn load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid id"));
    }
    let rows = invoice_read::qry(&st.db, &format!("{SEL} WHERE id = {id}")).await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "документът не е намерен"));
    };
    if cell_i32_at(row, 7) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "документът не е намерен"));
    }
    one(st, row, true).await
}

async fn one(st: &AppState, row: &crate::db::Row, with_lines: bool) -> Result<Value, ApiError> {
    let id = cell_i32_at(row, 0);
    let mut item = json!({
        "id": id,
        "number": cell_str(row, 1),
        "doc_date": cell_str(row, 2),
        "location_id": cell_i32_at(row, 3),
        "status": cell_str(row, 4),
        "notes": cell_str(row, 5),
        "journal_entry_id": cell_i32_at(row, 6),
    });
    if with_lines {
        let rows = invoice_read::qry(
            &st.db,
            &format!(
                "SELECT id, product_id, quantity, unit_cost, amount, account_id, inventory_account_id, line_no \
                 FROM material_issue_lines WHERE doc_id = {id} ORDER BY line_no, id"
            ),
        )
        .await?;
        item["lines"] = json!(rows.iter().map(|r| json!({
            "id": cell_i32_at(r, 0),
            "product_id": cell_i32_at(r, 1),
            "quantity": cell_str(r, 2),
            "unit_cost": cell_str(r, 3),
            "amount": cell_str(r, 4),
            "account_id": cell_i32_at(r, 5),
            "inventory_account_id": cell_i32_at(r, 6),
            "line_no": cell_i32_at(r, 7),
        })).collect::<Vec<_>>());
    }
    Ok(item)
}

async fn wipe(st: &AppState, id: i32) -> Result<(), ApiError> {
    invoice_read::exe(&st.db, &format!("DELETE FROM material_issue_lines WHERE doc_id = {id}")).await?;
    invoice_read::exe(&st.db, &format!("DELETE FROM material_issues WHERE id = {id}")).await
}

fn draft(v: &Value) -> Result<(), ApiError> {
    if text(v, "status") == "draft" {
        Ok(())
    } else {
        Err(api_err(StatusCode::CONFLICT, "потвърден документ не се променя"))
    }
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn int_of(v: &Value, key: &str) -> i32 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0) as i32
}

async fn company(st: &AppState, headers: &HeaderMap) -> Result<i32, ApiError> {
    let sub = auth_sub(st, headers)?;
    let id = active_or_one(st, &sub).await?;
    if id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "няма избрана активна фирма"));
    }
    Ok(id)
}
