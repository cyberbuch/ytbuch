//! Дълготрайни активи и категории. Сумите остават низ.

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

const FA_SEL: &str = "SELECT id, name, inventory_number, category_id, acquisition_date, put_into_service_date, cost, salvage_value, useful_life_months, depreciation_method, accounting_depreciation_rate, tax_depreciation_rate, accumulated_depreciation, tax_accumulated_depreciation, status, serial_number, manufacturer, model, responsible_person, is_conserved, location_id, company_id FROM fixed_assets";

pub async fn categories(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let _ = company(&st, &headers).await?;
    let rows = invoice_read::qry(&st.db, "SELECT id, name, cita_category, min_depreciation_rate, max_depreciation_rate, default_method FROM fixed_asset_categories ORDER BY id").await?;
    let items: Vec<Value> = rows.iter().map(|r| json!({
        "id": cell_i32_at(r, 0),
        "name": cell_str(r, 1),
        "cita_category": cell_str(r, 2),
        "min_depreciation_rate": cell_str(r, 3),
        "max_depreciation_rate": cell_str(r, 4),
        "default_method": cell_str(r, 5),
    })).collect();
    Ok(paginate(items, &q))
}

pub async fn category_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _ = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let name = form::required(form::str_field(obj, "name")?, "name")?;
    let id = invoice_read::nid(&st.db, "fixed_asset_categories").await?;
    let min = rate(obj, "min_depreciation_rate", "0")?;
    let max = rate(obj, "max_depreciation_rate", "0")?;
    let method = opt(obj, "default_method", "linear")?;
    let cita = opt(obj, "cita_category", "")?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO fixed_asset_categories (id, name, cita_category, min_depreciation_rate, max_depreciation_rate, default_method, asset_account_id, depreciation_account_id, expense_account_id) \
             VALUES ({id}, {}, {}, {}, {}, {}, 0, 0, 0)",
            lit(&name), lit(&cita), lit(&min), lit(&max), lit(&method)
        ),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({
        "id": id, "name": name, "cita_category": cita, "min_depreciation_rate": min,
        "max_depreciation_rate": max, "default_method": method,
    }))))
}

pub async fn category_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let _ = company(&st, &headers).await?;
    need(id)?;
    let rows = invoice_read::qry(&st.db, &format!("SELECT name, cita_category, min_depreciation_rate, max_depreciation_rate, default_method FROM fixed_asset_categories WHERE id = {id}")).await?;
    let Some(cur) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Категорията не е намерена"));
    };
    let obj = form::object(&body)?;
    let name = form::str_field(obj, "name")?.unwrap_or_else(|| cell_str(cur, 0));
    if name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    let cita = form::str_field(obj, "cita_category")?.unwrap_or_else(|| cell_str(cur, 1));
    let min = match form::str_field(obj, "min_depreciation_rate")? { Some(s) => money_rate(&s)?, None => cell_str(cur, 2) };
    let max = match form::str_field(obj, "max_depreciation_rate")? { Some(s) => money_rate(&s)?, None => cell_str(cur, 3) };
    let method = form::str_field(obj, "default_method")?.unwrap_or_else(|| cell_str(cur, 4));
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE fixed_asset_categories SET name = {}, cita_category = {}, min_depreciation_rate = {}, max_depreciation_rate = {}, default_method = {} WHERE id = {id}",
            lit(&name), lit(&cita), lit(&min), lit(&max), lit(&method)
        ),
    )
    .await?;
    Ok(Json(json!({
        "id": id, "name": name, "cita_category": cita, "min_depreciation_rate": min,
        "max_depreciation_rate": max, "default_method": method,
    })))
}

pub async fn category_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let _ = company(&st, &headers).await?;
    need(id)?;
    invoice_read::exe(&st.db, &format!("DELETE FROM fixed_asset_categories WHERE id = {id}")).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let rows = invoice_read::qry(&st.db, &format!("{FA_SEL} WHERE company_id = {company} ORDER BY id")).await?;
    let items: Vec<Value> = rows.iter().map(fa_json).collect();
    Ok(paginate(items, &q))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let f = fields(obj, None)?;
    let id = invoice_read::nid(&st.db, "fixed_assets").await?;
    invoice_read::exe(&st.db, &insert_sql(id, company, &f)).await?;
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
    let f = fields(obj, Some(&cur))?;
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE fixed_assets SET name = {}, inventory_number = {}, category_id = {}, acquisition_date = {}, put_into_service_date = {}, cost = {}, salvage_value = {}, useful_life_months = {}, depreciation_method = {}, accounting_depreciation_rate = {}, tax_depreciation_rate = {}, status = {}, serial_number = {}, manufacturer = {}, model = {}, responsible_person = {}, is_conserved = {}, location_id = {} WHERE id = {id}",
            lit(&f.name), lit(&f.inventory_number), f.category_id, lit(&f.acquisition_date), lit(&f.put_into_service_date),
            lit(&f.cost), lit(&f.salvage), lit(&f.life), lit(&f.method), lit(&f.acc_rate), lit(&f.tax_rate),
            lit(&f.status), lit(&f.serial), lit(&f.maker), lit(&f.model), lit(&f.person), f.conserved, f.location_id
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
    invoice_read::exe(&st.db, &format!("DELETE FROM fixed_asset_events WHERE asset_id = {id}")).await?;
    invoice_read::exe(&st.db, &format!("DELETE FROM fixed_assets WHERE id = {id}")).await?;
    Ok(StatusCode::NO_CONTENT)
}

struct Fields {
    name: String,
    inventory_number: String,
    category_id: i32,
    acquisition_date: String,
    put_into_service_date: String,
    cost: String,
    salvage: String,
    life: String,
    method: String,
    acc_rate: String,
    tax_rate: String,
    status: String,
    serial: String,
    maker: String,
    model: String,
    person: String,
    conserved: i32,
    location_id: i32,
}

fn fields(obj: &Map<String, Value>, cur: Option<&Value>) -> Result<Fields, ApiError> {
    let name = pick_str(obj, cur, "name", "")?;
    if name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    Ok(Fields {
        name,
        inventory_number: pick_str(obj, cur, "inventory_number", "")?,
        category_id: pick_i32(obj, cur, "category_id")?,
        acquisition_date: pick_str(obj, cur, "acquisition_date", "")?,
        put_into_service_date: pick_str(obj, cur, "put_into_service_date", "")?,
        cost: money_of(&pick_str(obj, cur, "cost", "0")?)?,
        salvage: money_of(&pick_str(obj, cur, "salvage_value", "0")?)?,
        life: pick_str(obj, cur, "useful_life_months", "0")?,
        method: pick_str(obj, cur, "depreciation_method", "linear")?,
        acc_rate: money_rate(&pick_str(obj, cur, "accounting_depreciation_rate", "0")?)?,
        tax_rate: money_rate(&pick_str(obj, cur, "tax_depreciation_rate", "0")?)?,
        status: pick_str(obj, cur, "status", "active")?,
        serial: pick_str(obj, cur, "serial_number", "")?,
        maker: pick_str(obj, cur, "manufacturer", "")?,
        model: pick_str(obj, cur, "model", "")?,
        person: pick_str(obj, cur, "responsible_person", "")?,
        conserved: pick_flag(obj, cur, "is_conserved")?,
        location_id: pick_i32(obj, cur, "location_id")?,
    })
}

fn insert_sql(id: i32, company: i32, f: &Fields) -> String {
    format!(
        "INSERT INTO fixed_assets (id, company_id, name, inventory_number, category_id, acquisition_date, put_into_service_date, cost, salvage_value, useful_life_months, depreciation_method, accounting_depreciation_rate, tax_depreciation_rate, accumulated_depreciation, tax_accumulated_depreciation, status, serial_number, manufacturer, model, responsible_person, is_conserved, location_id) \
         VALUES ({id}, {company}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, '0.00', '0.00', {}, {}, {}, {}, {}, {}, {})",
        lit(&f.name), lit(&f.inventory_number), f.category_id, lit(&f.acquisition_date), lit(&f.put_into_service_date),
        lit(&f.cost), lit(&f.salvage), lit(&f.life), lit(&f.method), lit(&f.acc_rate), lit(&f.tax_rate),
        lit(&f.status), lit(&f.serial), lit(&f.maker), lit(&f.model), lit(&f.person), f.conserved, f.location_id
    )
}

pub(crate) fn fa_json(r: &crate::db::Row) -> Value {
    let cat = cell_i32_at(r, 3);
    let mut o = json!({
        "id": cell_i32_at(r, 0),
        "name": cell_str(r, 1),
        "inventory_number": cell_str(r, 2),
        "acquisition_date": cell_str(r, 4),
        "put_into_service_date": cell_str(r, 5),
        "cost": cell_str(r, 6),
        "salvage_value": cell_str(r, 7),
        "useful_life_months": cell_str(r, 8),
        "depreciation_method": cell_str(r, 9),
        "accounting_depreciation_rate": cell_str(r, 10),
        "tax_depreciation_rate": cell_str(r, 11),
        "accumulated_depreciation": cell_str(r, 12),
        "tax_accumulated_depreciation": cell_str(r, 13),
        "status": cell_str(r, 14),
        "serial_number": cell_str(r, 15),
        "manufacturer": cell_str(r, 16),
        "model": cell_str(r, 17),
        "responsible_person": cell_str(r, 18),
        "is_conserved": cell_i32_at(r, 19),
        "location_id": cell_i32_at(r, 20),
        "company_id": cell_i32_at(r, 21),
    });
    if cat > 0 { o["category_id"] = json!(cat); }
    o
}

pub(crate) async fn load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let rows = invoice_read::qry(&st.db, &format!("{FA_SEL} WHERE id = {id}")).await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Активът не е намерен"));
    };
    if cell_i32_at(row, 21) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "Активът не е намерен"));
    }
    Ok(fa_json(row))
}

pub(crate) const SEL: &str = FA_SEL;

fn pick_str(obj: &Map<String, Value>, cur: Option<&Value>, key: &str, default: &str) -> Result<String, ApiError> {
    if let Some(v) = form::str_field(obj, key)? {
        return Ok(v);
    }
    Ok(cur.and_then(|c| c.get(key)).and_then(|v| v.as_str()).unwrap_or(default).to_string())
}

fn pick_i32(obj: &Map<String, Value>, cur: Option<&Value>, key: &str) -> Result<i32, ApiError> {
    if let Some(v) = form::i32_field(obj, key)? {
        return Ok(v);
    }
    Ok(cur.and_then(|c| c.get(key)).and_then(|v| v.as_i64()).unwrap_or(0) as i32)
}

fn pick_flag(obj: &Map<String, Value>, cur: Option<&Value>, key: &str) -> Result<i32, ApiError> {
    if let Some(v) = form::flag_field(obj, key)? {
        return Ok(v);
    }
    Ok(cur.and_then(|c| c.get(key)).and_then(|v| v.as_i64()).unwrap_or(0) as i32)
}

fn money_of(raw: &str) -> Result<String, ApiError> {
    let Some(v) = dec::parse(raw) else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност"));
    };
    Ok(money(v))
}

fn money_rate(raw: &str) -> Result<String, ApiError> {
    let Some(v) = dec::parse(raw) else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна норма"));
    };
    Ok(dec::text(v, 4))
}

fn rate(obj: &Map<String, Value>, key: &str, default: &str) -> Result<String, ApiError> {
    money_rate(&opt(obj, key, default)?)
}

fn opt(obj: &Map<String, Value>, key: &str, default: &str) -> Result<String, ApiError> {
    Ok(form::str_field(obj, key)?.unwrap_or_else(|| default.to_string()))
}

pub(crate) fn need(id: i32) -> Result<(), ApiError> {
    if id <= 0 { Err(api_err(StatusCode::BAD_REQUEST, "invalid id")) } else { Ok(()) }
}

pub(crate) async fn company(st: &AppState, headers: &HeaderMap) -> Result<i32, ApiError> {
    let sub = auth_sub(st, headers)?;
    let id = active_or_one(st, &sub).await?;
    if id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "няма избрана активна фирма"));
    }
    Ok(id)
}
