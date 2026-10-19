//! Запис на сметка и стока. Един ред, без SQL транзакция.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit, Row};
use crate::form::{self, active_or_one, auth_sub, db_err};
use crate::{api_err, AppState, ApiError};

struct Account {
    id: i32,
    company_id: i32,
    number: String,
    name: String,
    saft_account_type: String,
    analytic_type: String,
}

fn account_json(a: &Account) -> Value {
    json!({
        "id": a.id,
        "number": a.number,
        "name": a.name,
        "saft_account_type": a.saft_account_type,
        "analytic_type": a.analytic_type,
    })
}

fn account_from(r: &Row) -> Account {
    Account {
        id: cell_i32_at(r, 0),
        company_id: cell_i32_at(r, 1),
        number: cell_str(r, 2),
        name: cell_str(r, 3),
        saft_account_type: cell_str(r, 4),
        analytic_type: cell_str(r, 5),
    }
}

const ACC_SEL: &str =
    "SELECT id, company_id, number, name, saft_account_type, analytic_type FROM accounts";

async fn account_load(st: &AppState, id: i32, company: i32) -> Result<Account, ApiError> {
    let rows = st
        .db
        .query(&format!("{ACC_SEL} WHERE id = {id} AND company_id = {company}"))
        .await
        .map_err(db_err)?;
    rows.into_iter()
        .next()
        .map(|r| account_from(&r))
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "сметката не е намерена"))
}

fn analytic_ok(value: &str) -> bool {
    matches!(value, "none" | "counterpart" | "product")
}

async fn number_taken(st: &AppState, company_id: i32, number: &str, except: i32) -> Result<bool, ApiError> {
    let rows = st
        .db
        .query(&format!(
            "SELECT id FROM accounts WHERE company_id = {company_id} AND number = {} AND id <> {except}",
            lit(number)
        ))
        .await
        .map_err(db_err)?;
    Ok(!rows.is_empty())
}

pub async fn account_show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    Ok(Json(account_json(&account_load(&st, id, company).await?)))
}

pub async fn account_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let number = form::required(form::str_field(obj, "number")?, "number")?;
    let name = form::required(form::str_field(obj, "name")?, "name")?;
    let saft = form::str_field(obj, "saft_account_type")?.unwrap_or_default();
    let analytic = form::str_field(obj, "analytic_type")?.unwrap_or_else(|| "none".into());
    if !analytic_ok(analytic.trim()) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "analytic_type е none, counterpart или product",
        ));
    }
    let company_id = active_or_one(&st, &sub).await?;
    let number = number.trim().to_string();
    if number_taken(&st, company_id, &number, 0).await? {
        return Err(api_err(StatusCode::CONFLICT, "номерът вече съществува"));
    }
    let id = st.db.next_id("accounts").await.map_err(db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO accounts (id, company_id, number, name, saft_account_type, analytic_type) \
             VALUES ({id}, {company_id}, {}, {}, {}, {})",
            lit(&number),
            lit(name.trim()),
            lit(saft.trim()),
            lit(analytic.trim())
        ))
        .await
        .map_err(db_err)?;
    Ok((StatusCode::CREATED, Json(account_json(&account_load(&st, id, company_id).await?))))
}

pub async fn account_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let mut a = account_load(&st, id, company).await?;
    if let Some(v) = form::str_field(obj, "number")? {
        let number = v.trim().to_string();
        if number.is_empty() {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "number е задължително"));
        }
        a.number = number;
    }
    if let Some(v) = form::str_field(obj, "name")? {
        if v.trim().is_empty() {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
        }
        a.name = v;
    }
    if let Some(v) = form::str_field(obj, "saft_account_type")? {
        a.saft_account_type = v;
    }
    if let Some(v) = form::str_field(obj, "analytic_type")? {
        if !analytic_ok(v.trim()) {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "analytic_type е none, counterpart или product",
            ));
        }
        a.analytic_type = v;
    }
    if number_taken(&st, a.company_id, a.number.trim(), a.id).await? {
        return Err(api_err(StatusCode::CONFLICT, "номерът вече съществува"));
    }
    st.db
        .exec(&format!(
            "UPDATE accounts SET number = {}, name = {}, saft_account_type = {}, analytic_type = {} WHERE id = {} AND company_id = {}",
            lit(a.number.trim()),
            lit(a.name.trim()),
            lit(a.saft_account_type.trim()),
            lit(a.analytic_type.trim()),
            a.id,
            a.company_id
        ))
        .await
        .map_err(db_err)?;
    Ok(Json(account_json(&account_load(&st, id, company).await?)))
}

pub async fn account_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let a = account_load(&st, id, company).await?;
    st.db
        .exec(&format!("DELETE FROM accounts WHERE id = {} AND company_id = {}", a.id, a.company_id))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

struct Product {
    id: i32,
    company_id: i32,
    name: String,
    code: String,
    description: String,
    unit: String,
    price: String,
    cost_price: String,
    vat_rate: String,
    is_service: i32,
    is_inventory: i32,
    inventory_account_id: i32,
    revenue_account_id: i32,
    stock_quantity: String,
    stock_value: String,
    is_active: i32,
}

fn product_json(p: &Product) -> Value {
    json!({
        "id": p.id,
        "name": p.name,
        "code": p.code,
        "description": p.description,
        "unit": p.unit,
        "price": p.price,
        "cost_price": p.cost_price,
        "vat_rate": p.vat_rate,
        "is_service": p.is_service,
        "is_inventory": p.is_inventory,
        "inventory_account_id": p.inventory_account_id,
        "revenue_account_id": p.revenue_account_id,
        "stock_quantity": p.stock_quantity,
        "stock_value": p.stock_value,
        "is_active": p.is_active,
    })
}

fn product_from(r: &Row) -> Product {
    Product {
        id: cell_i32_at(r, 0),
        company_id: cell_i32_at(r, 1),
        name: cell_str(r, 2),
        code: cell_str(r, 3),
        description: cell_str(r, 4),
        unit: cell_str(r, 5),
        price: cell_str(r, 6),
        cost_price: cell_str(r, 7),
        vat_rate: cell_str(r, 8),
        is_service: cell_i32_at(r, 9),
        is_inventory: cell_i32_at(r, 10),
        inventory_account_id: cell_i32_at(r, 11),
        revenue_account_id: cell_i32_at(r, 12),
        stock_quantity: cell_str(r, 13),
        stock_value: cell_str(r, 14),
        is_active: cell_i32_at(r, 15),
    }
}

const PROD_SEL: &str = "SELECT id, company_id, name, code, description, unit, price, cost_price, \
vat_rate, is_service, is_inventory, inventory_account_id, revenue_account_id, stock_quantity, \
stock_value, is_active FROM products";

async fn code_taken(st: &AppState, company_id: i32, code: &str, except: i32) -> Result<bool, ApiError> {
    let t = code.trim();
    if t.is_empty() {
        return Ok(false);
    }
    let rows = st
        .db
        .query(&format!(
            "SELECT id, code FROM products WHERE company_id = {company_id} AND id <> {except}"
        ))
        .await
        .map_err(db_err)?;
    Ok(rows.iter().any(|r| cell_str(r, 1).trim().eq_ignore_ascii_case(t)))
}

async fn next_numeric_code(st: &AppState, company_id: i32) -> Result<String, ApiError> {
    let rows = st
        .db
        .query(&format!(
            "SELECT code FROM products WHERE company_id = {company_id}"
        ))
        .await
        .map_err(db_err)?;
    let mut max_num: i64 = 0;
    for r in &rows {
        let code = cell_str(r, 0);
        let t = code.trim();
        if t.is_empty() {
            continue;
        }
        if let Ok(n) = t.parse::<i64>() {
            if n > max_num {
                max_num = n;
            }
        }
    }
    let mut candidate = max_num + 1;
    loop {
        let code_str = candidate.to_string();
        if !code_taken(st, company_id, &code_str, 0).await? {
            return Ok(code_str);
        }
        candidate += 1;
    }
}

async fn product_load(st: &AppState, id: i32, company: i32) -> Result<Product, ApiError> {
    let rows = st
        .db
        .query(&format!("{PROD_SEL} WHERE id = {id} AND company_id = {company}"))
        .await
        .map_err(db_err)?;
    rows.into_iter()
        .next()
        .map(|r| product_from(&r))
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "стоката не е намерена"))
}

fn finish_product(p: &mut Product) {
    if p.unit.trim().is_empty() {
        p.unit = "C62".into();
    }
    if p.price.trim().is_empty() {
        p.price = "0".into();
    }
    if p.cost_price.trim().is_empty() {
        p.cost_price = "0".into();
    }
    if p.vat_rate.trim().is_empty() {
        p.vat_rate = "20".into();
    }
    if p.stock_quantity.trim().is_empty() {
        p.stock_quantity = "0".into();
    }
    if p.is_service != 0 {
        p.is_service = 1;
        p.is_inventory = 0;
        p.stock_quantity = "0".into();
        p.stock_value = "0".into();
    }
}

fn apply_product(p: &mut Product, obj: &serde_json::Map<String, Value>) -> Result<(), ApiError> {
    if let Some(v) = form::str_field(obj, "name")? {
        if v.trim().is_empty() {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
        }
        p.name = v;
    }
    if let Some(v) = form::str_field(obj, "code")? {
        p.code = v;
    }
    if let Some(v) = form::str_field(obj, "description")? {
        p.description = v;
    }
    if let Some(v) = form::str_field(obj, "unit")? {
        p.unit = v;
    }
    if let Some(v) = form::str_field(obj, "price")? {
        p.price = v;
    }
    if let Some(v) = form::str_field(obj, "cost_price")? {
        p.cost_price = v;
    }
    if let Some(v) = form::str_field(obj, "vat_rate")? {
        p.vat_rate = v;
    }
    if let Some(v) = form::flag_field(obj, "is_service")? {
        p.is_service = v;
    }
    if let Some(v) = form::flag_field(obj, "is_inventory")? {
        p.is_inventory = v;
    }
    if let Some(v) = form::i32_field(obj, "inventory_account_id")? {
        p.inventory_account_id = v;
    }
    if let Some(v) = form::i32_field(obj, "revenue_account_id")? {
        p.revenue_account_id = v;
    }
    if let Some(v) = form::flag_field(obj, "is_active")? {
        p.is_active = v;
    }
    Ok(())
}

async fn product_save(st: &AppState, p: &Product) -> Result<(), ApiError> {
    st.db
        .exec(&format!(
            "UPDATE products SET name = {}, code = {}, description = {}, unit = {}, price = {}, \
             cost_price = {}, vat_rate = {}, is_service = {}, is_inventory = {}, \
             inventory_account_id = {}, revenue_account_id = {}, stock_quantity = {}, \
             stock_value = {}, is_active = {} WHERE id = {} AND company_id = {}",
            lit(p.name.trim()),
            lit(p.code.trim()),
            lit(&p.description),
            lit(p.unit.trim()),
            lit(p.price.trim()),
            lit(p.cost_price.trim()),
            lit(p.vat_rate.trim()),
            p.is_service,
            p.is_inventory,
            p.inventory_account_id,
            p.revenue_account_id,
            lit(p.stock_quantity.trim()),
            lit(&p.stock_value),
            p.is_active,
            p.id,
            p.company_id
        ))
        .await
        .map_err(db_err)?;
    Ok(())
}

pub async fn product_show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    Ok(Json(product_json(&product_load(&st, id, company).await?)))
}

pub async fn product_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let mut p = Product {
        id: 0,
        company_id: active_or_one(&st, &sub).await?,
        name: String::new(),
        code: String::new(),
        description: String::new(),
        unit: "C62".into(),
        price: "0".into(),
        cost_price: "0".into(),
        vat_rate: "20".into(),
        is_service: 0,
        is_inventory: 0,
        inventory_account_id: 0,
        revenue_account_id: 0,
        stock_quantity: "0".into(),
        stock_value: "0".into(),
        is_active: 1,
    };
    apply_product(&mut p, obj)?;
    if p.name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    if p.code.trim().is_empty() {
        p.code = next_numeric_code(&st, p.company_id).await?;
    }
    if code_taken(&st, p.company_id, &p.code, 0).await? {
        return Err(api_err(StatusCode::CONFLICT, "артикул с този код вече съществува"));
    }
    form::ensure_ref(&st.db, "accounts", p.inventory_account_id, p.company_id, "сметката").await?;
    form::ensure_ref(&st.db, "accounts", p.revenue_account_id, p.company_id, "сметката").await?;
    finish_product(&mut p);
    p.stock_quantity = "0".into();
    p.stock_value = "0".into();
    p.id = st.db.next_id("products").await.map_err(db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO products (id, company_id, name, code, description, unit, price, cost_price, \
             vat_rate, is_service, is_inventory, inventory_account_id, revenue_account_id, \
             stock_quantity, stock_value, is_active) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, \
             {}, {}, {}, {}, {}, {})",
            p.id,
            p.company_id,
            lit(p.name.trim()),
            lit(p.code.trim()),
            lit(&p.description),
            lit(p.unit.trim()),
            lit(p.price.trim()),
            lit(p.cost_price.trim()),
            lit(p.vat_rate.trim()),
            p.is_service,
            p.is_inventory,
            p.inventory_account_id,
            p.revenue_account_id,
            lit(p.stock_quantity.trim()),
            lit(&p.stock_value),
            p.is_active
        ))
        .await
        .map_err(db_err)?;
    Ok((StatusCode::CREATED, Json(product_json(&product_load(&st, p.id, p.company_id).await?))))
}

pub async fn product_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let mut p = product_load(&st, id, company).await?;
    let qty = p.stock_quantity.clone();
    let val = p.stock_value.clone();
    apply_product(&mut p, obj)?;
    if code_taken(&st, p.company_id, &p.code, p.id).await? {
        return Err(api_err(StatusCode::CONFLICT, "артикул с този код вече съществува"));
    }
    form::ensure_ref(&st.db, "accounts", p.inventory_account_id, p.company_id, "сметката").await?;
    form::ensure_ref(&st.db, "accounts", p.revenue_account_id, p.company_id, "сметката").await?;
    finish_product(&mut p);
    p.stock_quantity = qty;
    p.stock_value = val;
    product_save(&st, &p).await?;
    Ok(Json(product_json(&product_load(&st, id, company).await?)))
}

pub async fn product_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let p = product_load(&st, id, company).await?;
    st.db
        .exec(&format!(
            "DELETE FROM products WHERE id = {} AND company_id = {}",
            p.id, p.company_id
        ))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

