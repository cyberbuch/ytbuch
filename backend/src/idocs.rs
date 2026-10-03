//! Вътрешни протоколи. Потвърждението мести артикули и ДМА между обекти.
//! PDF печатът остава за фаза 6. Обектите нямат колона is_active.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, money, qty};
use crate::form::{self, active_or_one, auth_sub, listed};
use crate::invoice_read;
use crate::periods;
use crate::stock;
use crate::{api_err, AppState, ApiError};

const DOC_SEL: &str = "SELECT id, number, doc_date, from_location_id, to_location_id, status, notes, handed_by, received_by, company_id FROM internal_docs";

pub async fn next_number(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    Ok(Json(json!({ "number": next_of(&st, company).await? })))
}

pub async fn index(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let rows = invoice_read::qry(&st.db, &format!("{DOC_SEL} WHERE company_id = {company} ORDER BY id")).await?;
    let mut items = Vec::new();
    for row in &rows {
        items.push(one(&st, row).await?);
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
    let lines = parse_lines(obj)?;
    let mut number = form::str_field(obj, "number")?.unwrap_or_default();
    if number.trim().is_empty() {
        number = next_of(&st, company).await?;
    }
    let id = invoice_read::nid(&st.db, "internal_docs").await?;
    invoice_read::exe(&st.db, &insert_doc(id, company, &number, obj, "draft")?).await?;
    if let Err(e) = insert_lines(&st, id, &lines).await {
        let _ = wipe(&st, id).await;
        return Err(e);
    }
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
    if cur.get("status").and_then(|v| v.as_str()) == Some("confirmed") {
        return Err(api_err(StatusCode::CONFLICT, "протоколът е вече потвърден"));
    }
    let obj = form::object(&body)?;
    let number = form::str_field(obj, "number")?.unwrap_or_else(|| text(&cur, "number"));
    let doc_date = form::str_field(obj, "doc_date")?.unwrap_or_else(|| text(&cur, "doc_date"));
    let from_id = form::i32_field(obj, "from_location_id")?.unwrap_or(int_of(&cur, "from_location_id"));
    let to_id = form::i32_field(obj, "to_location_id")?.unwrap_or(int_of(&cur, "to_location_id"));
    let notes = form::str_field(obj, "notes")?.unwrap_or_else(|| text(&cur, "notes"));
    let handed = form::str_field(obj, "handed_by")?.unwrap_or_else(|| text(&cur, "handed_by"));
    let received = form::str_field(obj, "received_by")?.unwrap_or_else(|| text(&cur, "received_by"));
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE internal_docs SET number = {}, doc_date = {}, from_location_id = {from_id}, to_location_id = {to_id}, notes = {}, handed_by = {}, received_by = {} WHERE id = {id}",
            lit(&number), lit(&doc_date), lit(&notes), lit(&handed), lit(&received)
        ),
    )
    .await?;
    if obj.contains_key("lines") {
        let lines = parse_lines(obj)?;
        invoice_read::exe(&st.db, &format!("DELETE FROM internal_doc_lines WHERE doc_id = {id}")).await?;
        if let Err(e) = insert_lines(&st, id, &lines).await {
            return Err(e);
        }
    }
    Ok(Json(load(&st, id, company).await?))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let company = company(&st, &headers).await?;
    need(id)?;
    let cur = load(&st, id, company).await?;
    if cur.get("status").and_then(|v| v.as_str()) == Some("confirmed") {
        return Err(api_err(StatusCode::CONFLICT, "потвърденият протокол не се трие"));
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
    need(id)?;
    let cur = load(&st, id, company).await?;
    if cur.get("status").and_then(|v| v.as_str()) == Some("confirmed") {
        return Err(api_err(StatusCode::CONFLICT, "протоколът е вече потвърден"));
    }
    let doc_date = text(&cur, "doc_date");
    periods::assert_open(&st.db, &doc_date).await?;
    let from_id = int_of(&cur, "from_location_id");
    let to_id = int_of(&cur, "to_location_id");
    if from_id <= 0 || to_id <= 0 || from_id == to_id {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "нужни са два различни активни обекта"));
    }
    let from_name = place(&st, company, from_id, "обектът-източник не е намерен").await?;
    let to_name = place(&st, company, to_id, "обектът-приемник не е намерен").await?;
    let lines = cur.get("lines").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut snaps = Vec::new();
    let mut moved = Vec::new();
    for line in &lines {
        if let Err(e) = apply_line(&st, company, &cur, line, from_id, to_id, &from_name, &to_name, &doc_date, &mut snaps, &mut moved).await {
            let _ = stock::undo(&st.db, &snaps).await;
            undo_assets(&st, &moved).await;
            return Err(e);
        }
    }
    if let Err(e) = invoice_read::exe(&st.db, &format!("UPDATE internal_docs SET status = 'confirmed' WHERE id = {id}")).await {
        let _ = stock::undo(&st.db, &snaps).await;
        undo_assets(&st, &moved).await;
        return Err(e);
    }
    Ok(Json(load(&st, id, company).await?))
}

async fn apply_line(
    st: &AppState,
    company: i32,
    doc: &Value,
    line: &Value,
    from_id: i32,
    to_id: i32,
    from_name: &str,
    to_name: &str,
    doc_date: &str,
    snaps: &mut Vec<stock::Snap>,
    moved: &mut Vec<(i32, i32, i32)>,
) -> Result<(), ApiError> {
    let kind = line.get("line_kind").and_then(|v| v.as_str()).unwrap_or("");
    if kind == "product" {
        let product_id = line.get("product_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let name = product_name(st, company, product_id).await?;
        let q = line.get("quantity").and_then(|v| v.as_str()).unwrap_or("0");
        let snap = stock::transfer(&st.db, product_id, q, from_id, to_id, doc_date, &name).await?;
        snaps.push(snap);
        return Ok(());
    }
    let asset_id = line.get("asset_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let asset = crate::assets::load(st, asset_id, company)
        .await
        .map_err(|_| api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("ДМА не е намерено: {asset_id}")))?;
    let loc = asset.get("location_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if loc != from_id && loc != 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("{name} не е в обекта-източник")));
    }
    invoice_read::exe(&st.db, &format!("UPDATE fixed_assets SET location_id = {to_id} WHERE id = {asset_id}")).await?;
    let number = text(doc, "number");
    let reason = format!("протокол {number}: {from_name} → {to_name}");
    let event_id = invoice_read::nid(&st.db, "fixed_asset_events").await?;
    if let Err(e) = invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO fixed_asset_events (id, asset_id, event_type, event_date, amount, prev_value, location, reason) \
             VALUES ({event_id}, {asset_id}, 'movement', {}, '0.00', '', {}, {})",
            lit(doc_date),
            lit(to_name),
            lit(&reason)
        ),
    )
    .await
    {
        let _ = invoice_read::exe(&st.db, &format!("UPDATE fixed_assets SET location_id = {loc} WHERE id = {asset_id}")).await;
        return Err(e);
    }
    moved.push((asset_id, loc, event_id));
    Ok(())
}

async fn undo_assets(st: &AppState, moved: &[(i32, i32, i32)]) {
    for (asset_id, loc, event_id) in moved.iter().rev() {
        let _ = invoice_read::exe(&st.db, &format!("DELETE FROM fixed_asset_events WHERE id = {event_id}")).await;
        let _ = invoice_read::exe(&st.db, &format!("UPDATE fixed_assets SET location_id = {loc} WHERE id = {asset_id}")).await;
    }
}

struct LineIn {
    kind: String,
    product_id: i32,
    asset_id: i32,
    quantity: String,
    unit_cost: String,
    amount: String,
    description: String,
}

fn parse_lines(obj: &Map<String, Value>) -> Result<Vec<LineIn>, ApiError> {
    let Some(raw) = obj.get("lines") else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "lines е задължителен масив"));
    };
    let Some(arr) = raw.as_array() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "lines е задължителен масив"));
    };
    if arr.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "протоколът трябва да има поне един ред"));
    }
    let mut lines = Vec::new();
    for el in arr {
        let Some(line) = el.as_object() else {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "lines е задължителен масив"));
        };
        let kind = form::str_field(line, "line_kind")?.unwrap_or_default();
        if kind != "product" && kind != "fixed_asset" {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "line_kind трябва да е product или fixed_asset"));
        }
        let product_id = id_field(line, "product_id")?;
        let asset_id = id_field(line, "asset_id")?;
        if kind == "product" && product_id <= 0 {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулният ред изисква product_id"));
        }
        if kind == "fixed_asset" && asset_id <= 0 {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "редът за ДМА изисква asset_id"));
        }
        let quantity = qty_of(&form::str_field(line, "quantity")?.unwrap_or_else(|| "0".into()))?;
        let unit_cost = money_of(&form::str_field(line, "unit_cost")?.unwrap_or_else(|| "0".into()))?;
        let amount = match form::str_field(line, "amount")? {
            Some(s) if !s.trim().is_empty() => money_of(&s)?,
            _ => money(dec::mul(dec::or0(&quantity), dec::or0(&unit_cost)).unwrap_or(0)),
        };
        lines.push(LineIn {
            kind,
            product_id,
            asset_id,
            quantity,
            unit_cost,
            amount,
            description: form::str_field(line, "description")?.unwrap_or_default(),
        });
    }
    Ok(lines)
}

fn id_field(obj: &Map<String, Value>, key: &str) -> Result<i32, ApiError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(0),
        Some(Value::Number(n)) => n.as_i64().and_then(|v| i32::try_from(v).ok()).ok_or_else(|| bad_id(key)),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                Ok(0)
            } else {
                t.parse::<i32>().map_err(|_| bad_id(key))
            }
        }
        Some(_) => Err(bad_id(key)),
    }
}

fn bad_id(key: &str) -> ApiError {
    let msg = if key == "asset_id" { "невалиден asset_id" } else { "невалиден product_id" };
    api_err(StatusCode::UNPROCESSABLE_ENTITY, msg)
}

fn qty_of(raw: &str) -> Result<String, ApiError> {
    match dec::parse(raw) {
        Some(v) => Ok(qty(v)),
        None => Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност")),
    }
}

fn money_of(raw: &str) -> Result<String, ApiError> {
    match dec::parse(raw) {
        Some(v) => Ok(money(v)),
        None => Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност")),
    }
}

fn insert_doc(id: i32, company: i32, number: &str, obj: &Map<String, Value>, status: &str) -> Result<String, ApiError> {
    let doc_date = form::str_field(obj, "doc_date")?.unwrap_or_default();
    let from_id = form::i32_field(obj, "from_location_id")?.unwrap_or(0);
    let to_id = form::i32_field(obj, "to_location_id")?.unwrap_or(0);
    let notes = form::str_field(obj, "notes")?.unwrap_or_default();
    let handed = form::str_field(obj, "handed_by")?.unwrap_or_default();
    let received = form::str_field(obj, "received_by")?.unwrap_or_default();
    Ok(format!(
        "INSERT INTO internal_docs (id, company_id, number, doc_date, from_location_id, to_location_id, status, notes, handed_by, received_by) \
         VALUES ({id}, {company}, {}, {}, {from_id}, {to_id}, {}, {}, {}, {})",
        lit(number), lit(&doc_date), lit(status), lit(&notes), lit(&handed), lit(&received)
    ))
}

async fn insert_lines(st: &AppState, doc_id: i32, lines: &[LineIn]) -> Result<(), ApiError> {
    for line in lines {
        let id = invoice_read::nid(&st.db, "internal_doc_lines").await?;
        invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO internal_doc_lines (id, doc_id, line_kind, product_id, asset_id, quantity, unit_cost, amount, description) \
                 VALUES ({id}, {doc_id}, {}, {}, {}, {}, {}, {}, {})",
                lit(&line.kind), line.product_id, line.asset_id, lit(&line.quantity), lit(&line.unit_cost), lit(&line.amount), lit(&line.description)
            ),
        )
        .await?;
    }
    Ok(())
}

async fn next_of(st: &AppState, company: i32) -> Result<String, ApiError> {
    let rows = invoice_read::qry(&st.db, &format!("SELECT number FROM internal_docs WHERE company_id = {company}")).await?;
    let mut max = 0i64;
    for row in &rows {
        if let Ok(n) = cell_str(row, 0).trim().parse::<i64>() {
            if n > max {
                max = n;
            }
        }
    }
    Ok(format!("{:010}", max + 1))
}

async fn load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    need(id)?;
    let rows = invoice_read::qry(&st.db, &format!("{DOC_SEL} WHERE id = {id}")).await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Internal document not found"));
    };
    if cell_i32_at(row, 9) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "Internal document not found"));
    }
    one(st, row).await
}

async fn one(st: &AppState, row: &crate::db::Row) -> Result<Value, ApiError> {
    let id = cell_i32_at(row, 0);
    let lines = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, line_kind, product_id, asset_id, quantity, unit_cost, amount, description \
             FROM internal_doc_lines WHERE doc_id = {id} ORDER BY id"
        ),
    )
    .await?;
    let items: Vec<Value> = lines
        .iter()
        .map(|r| {
            json!({
                "id": cell_i32_at(r, 0),
                "line_kind": cell_str(r, 1),
                "product_id": cell_i32_at(r, 2),
                "asset_id": cell_i32_at(r, 3),
                "quantity": cell_str(r, 4),
                "unit_cost": cell_str(r, 5),
                "amount": cell_str(r, 6),
                "description": cell_str(r, 7),
            })
        })
        .collect();
    Ok(json!({
        "id": id,
        "number": cell_str(row, 1),
        "doc_date": cell_str(row, 2),
        "from_location_id": cell_i32_at(row, 3),
        "to_location_id": cell_i32_at(row, 4),
        "status": cell_str(row, 5),
        "notes": cell_str(row, 6),
        "handed_by": cell_str(row, 7),
        "received_by": cell_str(row, 8),
        "lines": items,
    }))
}

async fn wipe(st: &AppState, id: i32) -> Result<(), ApiError> {
    invoice_read::exe(&st.db, &format!("DELETE FROM internal_doc_lines WHERE doc_id = {id}")).await?;
    invoice_read::exe(&st.db, &format!("DELETE FROM internal_docs WHERE id = {id}")).await
}

async fn place(st: &AppState, company: i32, id: i32, missing: &str) -> Result<String, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT name FROM company_locations WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, missing));
    };
    let name = cell_str(row, 0);
    Ok(if name.is_empty() { id.to_string() } else { name })
}

async fn product_name(st: &AppState, company: i32, id: i32) -> Result<String, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT name FROM products WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    rows.first()
        .map(|r| cell_str(r, 0))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"))
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn int_of(v: &Value, key: &str) -> i32 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0) as i32
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
