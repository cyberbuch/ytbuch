//! Поръчки за производство. Потвърждението изписва материали и заприходява продукция.
//! Сметка 611, а ако я няма — 601.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, money, qty};
use crate::form::{self, active_or_one, auth_sub, listed};
use crate::invoice_read;
use crate::periods;
use crate::posting::{self, Acct, Side};
use crate::recipes;
use crate::stock::{self, Snap};
use crate::{api_err, AppState, ApiError};

const SEL: &str = "SELECT id, number, recipe_id, output_product_id, quantity, location_id, order_date, status, notes, material_cost, journal_entry_id, company_id FROM production_orders";

pub async fn index(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let rows = invoice_read::qry(&st.db, &format!("{SEL} WHERE company_id = {company} ORDER BY id")).await?;
    let mut items = Vec::new();
    for row in &rows {
        items.push(one(&st, row, false).await?);
    }
    Ok(listed(items))
}

pub async fn show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    Ok(Json(load(&st, id, company).await?))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
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
    if let Err(e) = copy_lines(&st, id, &recipe, &quantity).await {
        let _ = wipe(&st, id).await;
        return Err(e);
    }
    Ok((StatusCode::CREATED, Json(load(&st, id, company).await?)))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let company = company(&st, &headers).await?;
    let cur = load(&st, id, company).await?;
    if cur.get("status").and_then(|v| v.as_str()) != Some("draft") {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "завършена поръчка не се изтрива"));
    }
    wipe(&st, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn confirm(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
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
    periods::assert_open(&st.db, &order_date).await?;
    let acc611 = acc_611(&st, company).await?;
    let method = stock::valuation(&st.db, company).await?;
    let mut snaps: Vec<Snap> = Vec::new();
    let mut sides: Vec<Side> = Vec::new();
    let mut total = 0i128;
    let mut updates = Vec::new();
    for line in &lines {
        let line_id = line.get("id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let product_id = line.get("product_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let q = line.get("quantity").and_then(|v| v.as_str()).unwrap_or("0");
        let prod = match product(&st, product_id).await {
            Ok(p) => p,
            Err(e) => return Err(undo_all(&st, &snaps, 0, e).await),
        };
        let (cost_s, snap) = match stock::issue(&st.db, product_id, q, &method, &prod.name, location_id).await {
            Ok(v) => v,
            Err(e) => return Err(undo_all(&st, &snaps, 0, e).await),
        };
        snaps.push(snap);
        let cost = dec::or0(&cost_s);
        if cost != 0 {
            let inv = match posting::prefer(&st.db, company, prod.inventory_account_id, &["302", "304"], "липсва складова сметка 302/304").await {
                Ok(a) => a,
                Err(e) => return Err(undo_all(&st, &snaps, 0, e).await),
            };
            sides.push(Side { account: copy_acct(&acc611), direction: "debit", amount: cost_s.clone() });
            sides.push(Side { account: inv, direction: "credit", amount: cost_s.clone() });
        }
        total = dec::add(total, cost).unwrap_or(total);
        let unit = if dec::or0(q) > 0 {
            dec::text(dec::div(cost, dec::or0(q)).unwrap_or(0), 6)
        } else {
            "0.000000".into()
        };
        updates.push((line_id, unit, cost_s));
    }
    let out_qty = dec::or0(&text(&cur, "quantity"));
    let unit_out = if out_qty > 0 { dec::text(dec::div(total, out_qty).unwrap_or(0), 6) } else { "0.000000".into() };
    let snap = match stock::receive(&st.db, output_id, id, 0, &order_date, &text(&cur, "quantity"), &unit_out, location_id).await {
        Ok((_, snap)) => snap,
        Err(e) => return Err(undo_all(&st, &snaps, 0, e).await),
    };
    snaps.push(snap);
    let mut entry = 0;
    if total != 0 {
        let out = match product(&st, output_id).await {
            Ok(p) => p,
            Err(e) => return Err(undo_all(&st, &snaps, 0, e).await),
        };
        let inv = match posting::prefer(
            &st.db,
            company,
            out.inventory_account_id,
            &["303", "304"],
            "липсва складова сметка 303/304 за готовата продукция",
        )
        .await
        {
            Ok(a) => a,
            Err(e) => return Err(undo_all(&st, &snaps, 0, e).await),
        };
        let total_s = money(total);
        sides.push(Side { account: inv, direction: "debit", amount: total_s.clone() });
        sides.push(Side { account: copy_acct(&acc611), direction: "credit", amount: total_s });
        entry = match posting::write_entry(
            &st.db,
            company,
            &order_date,
            "production",
            id,
            &text(&cur, "number"),
            &format!("Производство {}", text(&cur, "number")),
            0,
            "",
            &posting::month_of(&order_date),
            &sides,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => return Err(undo_all(&st, &snaps, 0, e).await),
        };
    }
    if let Err(e) = finish(&st, id, &updates, &money(total), entry).await {
        return Err(undo_all(&st, &snaps, entry, e).await);
    }
    Ok(Json(load(&st, id, company).await?))
}

struct Prod {
    name: String,
    inventory_account_id: i32,
}

async fn product(st: &AppState, id: i32) -> Result<Prod, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT name, inventory_account_id FROM products WHERE id = {id}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"));
    };
    Ok(Prod { name: cell_str(row, 0), inventory_account_id: cell_i32_at(row, 1) })
}

async fn acc_611(st: &AppState, company: i32) -> Result<Acct, ApiError> {
    if let Some(a) = posting::by_number(&st.db, company, "611").await? {
        return Ok(a);
    }
    posting::by_number(&st.db, company, "601")
        .await?
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "липсва сметка с номер 601"))
}

fn copy_acct(a: &Acct) -> Acct {
    Acct { id: a.id, number: a.number.clone(), name: a.name.clone() }
}

async fn undo_all(st: &AppState, snaps: &[Snap], entry: i32, e: ApiError) -> ApiError {
    let _ = stock::undo(&st.db, snaps).await;
    if entry > 0 {
        let _ = invoice_read::wipe_journal(&st.db, entry).await;
    }
    e
}

async fn finish(st: &AppState, id: i32, updates: &[(i32, String, String)], total: &str, entry: i32) -> Result<(), ApiError> {
    for (i, (line_id, unit, amount)) in updates.iter().enumerate() {
        if let Err(e) = invoice_read::exe(
            &st.db,
            &format!(
                "UPDATE production_order_lines SET unit_cost = {}, amount = {} WHERE id = {line_id}",
                lit(unit),
                lit(amount)
            ),
        )
        .await
        {
            for (line_id2, _, _) in updates.iter().take(i) {
                let _ = invoice_read::exe(
                    &st.db,
                    &format!("UPDATE production_order_lines SET unit_cost = '0.000000', amount = '0.00' WHERE id = {line_id2}"),
                )
                .await;
            }
            return Err(e);
        }
    }
    if let Err(e) = invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE production_orders SET status = 'completed', material_cost = {}, journal_entry_id = {entry} WHERE id = {id}",
            lit(total)
        ),
    )
    .await
    {
        for (line_id, _, _) in updates {
            let _ = invoice_read::exe(
                &st.db,
                &format!("UPDATE production_order_lines SET unit_cost = '0.000000', amount = '0.00' WHERE id = {line_id}"),
            )
            .await;
        }
        return Err(e);
    }
    Ok(())
}

async fn copy_lines(st: &AppState, order_id: i32, recipe: &recipes::Recipe, order_qty: &str) -> Result<(), ApiError> {
    for line in &recipe.lines {
        let need = scale(&line.quantity, order_qty, &recipe.output_qty, &line.wastage);
        let name = product(st, line.product_id).await.map(|p| p.name).unwrap_or_default();
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
