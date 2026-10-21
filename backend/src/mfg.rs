//! Поръчки за производство. Потвърждението маркира поръчката.
//! Себестойността и записът в дневника идват от преизчислението по правилата.

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

use crate::recipes;
use crate::stock_replay;
use crate::stock_rules;
use crate::{api_err, AppState, ApiError};

const SEL: &str = "SELECT id, number, recipe_id, output_product_id, quantity, location_id, order_date, status, notes, material_cost, journal_entry_id, company_id FROM production_orders";

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let stamps = crate::audit::stamps_for(&st.db, company, "production_order").await?;
    let rows = invoice_read::qry(&st.db, &format!("{SEL} WHERE company_id = {company} ORDER BY id")).await?;
    let mut items: Vec<Value> = Vec::new();
    for row in &rows {
        let mut item = one(&st, row, false).await?;
        let id = cell_i32_at(row, 0);
        crate::audit::attach_stamp(&mut item, stamps.get(&id));
        items.push(item);
    }
    Ok(paginate(items, &q))
}

pub async fn show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    Ok(Json(load_with_audit(&st, id, company).await?))
}

async fn load_with_audit(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let mut v = load(st, id, company).await?;
    let stamps = crate::audit::stamps_for(&st.db, company, "production_order").await?;
    crate::audit::attach_stamp(&mut v, stamps.get(&id));
    v["audit_history"] = json!(crate::audit::history_for(&st.db, company, "production_order", id).await?);
    Ok(v)
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let recipe_id = form::i32_field(obj, "recipe_id")?.unwrap_or(0);
    if recipe_id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "рецептата е задължителна"));
    }
    let recipe = recipes::load_recipe(&st, recipe_id, company).await?;
    let quantity = qty_of(&form::required(form::str_field(obj, "quantity")?, "quantity")?)?;
    if dec::or0(&quantity) <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "количеството трябва да е по-голямо от 0"));
    }
    let location_id = form::i32_field(obj, "location_id")?.unwrap_or(0);
    form::ensure_ref(&st.db, "company_locations", location_id, company, "обектът").await?;
    form::ensure_ref(&st.db, "products", recipe.output_id, company, "артикулът").await?;
    for line in &recipe.lines {
        form::ensure_ref(&st.db, "products", line.product_id, company, "артикулът").await?;
    }
    let order_date = form::str_field(obj, "order_date")?.unwrap_or_default();
    let notes = form::str_field(obj, "notes")?.unwrap_or_default();
    let id = invoice_read::nid(&st.db, "production_orders").await?;
    let number = format!("ПР-{id}");
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO production_orders (id, company_id, number, recipe_id, output_product_id, quantity, location_id, order_date, status, notes, material_cost, journal_entry_id) \
             VALUES ({id}, {company}, {}, {recipe_id}, {}, {}, {location_id}, {}, 'draft', {}, '0.00', 0)",
            lit(&number), recipe.output_id, lit(&quantity), lit(&order_date), lit(&notes)
        ),
    )
    .await?;
    if let Err(e) = copy_lines(&st, id, company, &recipe, &quantity).await {
        let _ = wipe(&st, id).await;
        return Err(e);
    }
    let after = load(&st, id, company).await?;
    let summary = format!("Създадена поръчка за производство {number} ({order_date}), к-во {quantity}");
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "production_order",
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

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = company(&st, &headers).await?;
    let cur = load(&st, id, company).await?;
    if cur.get("status").and_then(|v| v.as_str()) != Some("draft") {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "завършена поръчка не се изтрива"));
    }
    let number = text(&cur, "number");
    wipe(&st, id).await?;
    let summary = format!("Изтрита поръчка за производство {number}");
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "production_order",
        id,
        &number,
        "delete",
        &summary,
        &text(&cur, "material_cost"),
        "",
        Some(&cur),
        None,
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn confirm(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = company(&st, &headers).await?;
    let cur = load(&st, id, company).await?;
    if cur.get("status").and_then(|v| v.as_str()) == Some("completed") {
        return Err(api_err(StatusCode::CONFLICT, "поръчката е вече завършена"));
    }
    let location_id = int_of(&cur, "location_id");
    if location_id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "нужен е складов обект"));
    }
    let output_id = int_of(&cur, "output_product_id");
    if output_id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "липсва изходен артикул"));
    }
    let lines = cur.get("lines").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if lines.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "поръчката няма материали"));
    }
    let order_date = text(&cur, "order_date");
    periods::assert_open(&st.db, company, &order_date).await?;
    stock_rules::check_production(&st.db, company, output_id, &lines).await?;
    invoice_read::exe(
        &st.db,
        &format!("UPDATE production_orders SET status = 'completed', material_cost = '0.00', journal_entry_id = 0 WHERE id = {id}"),
    )
    .await?;
    if let Err(e) = stock_replay::rebuild(&st.db, company, "9999-12-31").await {
        let _ = invoice_read::exe(&st.db, &format!("UPDATE production_orders SET status = 'draft' WHERE id = {id}")).await;
        return Err(e);
    }
    let after = load(&st, id, company).await?;
    let number = text(&after, "number");
    let cost = text(&after, "material_cost");
    let summary = format!("Потвърдена поръчка за производство {number} ({order_date}), себестойност {cost}");
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "production_order",
        id,
        &number,
        "post",
        &summary,
        "0.00",
        &cost,
        Some(&cur),
        Some(&after),
    )
    .await;
    Ok(Json(load_with_audit(&st, id, company).await?))
}

async fn product_name(st: &AppState, id: i32, company: i32) -> Result<String, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT name FROM products WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"));
    };
    Ok(cell_str(row, 0))
}

async fn copy_lines(st: &AppState, order_id: i32, company: i32, recipe: &recipes::Recipe, order_qty: &str) -> Result<(), ApiError> {
    for line in &recipe.lines {
        let need = scale(&line.quantity, order_qty, &recipe.output_qty, &line.wastage);
        let name = product_name(st, line.product_id, company).await.unwrap_or_default();
        let id = invoice_read::nid(&st.db, "production_order_lines").await?;
        invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO production_order_lines (id, order_id, product_id, product_name, quantity, unit, unit_cost, amount) \
                 VALUES ({id}, {order_id}, {}, {}, {}, {}, '0.000000', '0.00')",
                line.product_id, lit(&name), lit(&need), lit(&line.unit)
            ),
        )
        .await?;
    }
    Ok(())
}

fn scale(line_qty: &str, order_qty: &str, output_qty: &str, wastage: &str) -> String {
    let line = dec::or0(line_qty);
    let order = dec::or0(order_qty);
    let mut out = dec::or0(output_qty);
    if out <= 0 {
        out = dec::parse("1").unwrap_or(1);
    }
    let base = dec::div(dec::mul(line, order).unwrap_or(0), out).unwrap_or(0);
    let hundred = dec::parse("100").unwrap_or(1);
    let one = dec::parse("1").unwrap_or(1);
    let factor = dec::add(one, dec::div(dec::or0(wastage), hundred).unwrap_or(0)).unwrap_or(one);
    qty(dec::mul(base, factor).unwrap_or(0))
}

fn qty_of(raw: &str) -> Result<String, ApiError> {
    match dec::parse(raw) {
        Some(v) => Ok(qty(v)),
        None => Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност")),
    }
}

async fn load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid id"));
    }
    let rows = invoice_read::qry(&st.db, &format!("{SEL} WHERE id = {id}")).await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Поръчката не е намерена"));
    };
    if cell_i32_at(row, 11) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "Поръчката не е намерена"));
    }
    one(st, row, true).await
}

async fn one(st: &AppState, row: &crate::db::Row, with_lines: bool) -> Result<Value, ApiError> {
    let id = cell_i32_at(row, 0);
    let mut item = json!({
        "id": id,
        "number": cell_str(row, 1),
        "recipe_id": cell_i32_at(row, 2),
        "output_product_id": cell_i32_at(row, 3),
        "quantity": cell_str(row, 4),
        "location_id": cell_i32_at(row, 5),
        "order_date": cell_str(row, 6),
        "status": cell_str(row, 7),
        "notes": cell_str(row, 8),
        "material_cost": cell_str(row, 9),
        "journal_entry_id": cell_i32_at(row, 10),
    });
    if with_lines {
        let rows = invoice_read::qry(
            &st.db,
            &format!(
                "SELECT id, product_id, product_name, quantity, unit, unit_cost, amount \
                 FROM production_order_lines WHERE order_id = {id} ORDER BY id"
            ),
        )
        .await?;
        item["lines"] = json!(rows.iter().map(|r| json!({
            "id": cell_i32_at(r, 0),
            "product_id": cell_i32_at(r, 1),
            "product_name": cell_str(r, 2),
            "quantity": cell_str(r, 3),
            "unit": cell_str(r, 4),
            "unit_cost": cell_str(r, 5),
            "amount": cell_str(r, 6),
        })).collect::<Vec<_>>());
    }
    Ok(item)
}

async fn wipe(st: &AppState, id: i32) -> Result<(), ApiError> {
    invoice_read::exe(&st.db, &format!("DELETE FROM production_order_lines WHERE order_id = {id}")).await?;
    invoice_read::exe(&st.db, &format!("DELETE FROM production_orders WHERE id = {id}")).await
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
