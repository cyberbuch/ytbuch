//! Правила за изписване. Номера на сметки не се подставят.
//! Видът е goods, production или material. Продукцията носи и междинна сметка.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit, Db};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_read;
use crate::{api_err, AppState, ApiError};

#[derive(Clone)]
pub struct Rule {
    pub kind: String,
    pub inventory_account_id: i32,
    pub offset_account_id: i32,
    pub clearing_account_id: i32,
}

pub async fn load(db: &Db, company: i32) -> Result<Vec<Rule>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT id, kind, inventory_account_id, offset_account_id, clearing_account_id \
             FROM stock_account_rules WHERE company_id = {company} ORDER BY id"
        ),
    )
    .await?;
    Ok(rows
        .iter()
        .map(|r| Rule {
            kind: cell_str(r, 1),
            inventory_account_id: cell_i32_at(r, 2),
            offset_account_id: cell_i32_at(r, 3),
            clearing_account_id: cell_i32_at(r, 4),
        })
        .collect())
}

pub fn pair<'a>(rules: &'a [Rule], kind: &str, inventory: i32, offset: i32) -> Option<&'a Rule> {
    rules.iter().find(|r| r.kind == kind && r.inventory_account_id == inventory && r.offset_account_id == offset)
}

pub fn material_inventory(rules: &[Rule], inventory: i32) -> Option<&Rule> {
    rules.iter().find(|r| r.kind == "material" && r.inventory_account_id == inventory && inventory > 0)
}

struct Named {
    name: String,
    inventory: i32,
    revenue: i32,
}

async fn named(db: &Db, id: i32, company: i32) -> Result<Named, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT name, inventory_account_id, revenue_account_id FROM products WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"));
    };
    Ok(Named {
        name: cell_str(row, 0),
        inventory: cell_i32_at(row, 1),
        revenue: cell_i32_at(row, 2),
    })
}

/// Поръчката не тръгва, ако готовата продукция или материал нямат правило.
pub async fn check_production(db: &Db, company: i32, output_id: i32, lines: &[Value]) -> Result<(), ApiError> {
    let rules = load(db, company).await?;
    let out = named(db, output_id, company).await?;
    match pair(&rules, "production", out.inventory, out.revenue) {
        Some(r) if r.clearing_account_id > 0 => {}
        _ => {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("няма правило за продукция за {}", out.name),
            ))
        }
    }
    for line in lines {
        let pid = line.get("product_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        if pid <= 0 {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "ред без артикул"));
        }
        let mat = named(db, pid, company).await?;
        if material_inventory(&rules, mat.inventory).is_none() {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("няма правило за материал за {}", mat.name),
            ));
        }
    }
    Ok(())
}

fn kind_ok(kind: &str) -> bool {
    matches!(kind, "goods" | "production" | "material")
}

fn row_json(r: &crate::db::Row) -> Value {
    json!({
        "id": cell_i32_at(r, 0),
        "kind": cell_str(r, 1),
        "inventory_account_id": cell_i32_at(r, 2),
        "offset_account_id": cell_i32_at(r, 3),
        "clearing_account_id": cell_i32_at(r, 4),
    })
}

async fn account_ok(db: &Db, id: i32, company: i32) -> Result<(), ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "сметката е задължителна"));
    }
    let rows = invoice_read::qry(
        db,
        &format!("SELECT id FROM accounts WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    if rows.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("липсва сметка с id {id}")));
    }
    Ok(())
}

pub async fn index(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, kind, inventory_account_id, offset_account_id, clearing_account_id \
             FROM stock_account_rules WHERE company_id = {company} ORDER BY id"
        ),
    )
    .await?;
    let items: Vec<Value> = rows.iter().map(row_json).collect();
    let count = items.len();
    Ok(Json(json!({ "items": items, "count": count })))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let kind = form::required(form::str_field(obj, "kind")?, "kind")?;
    let kind = kind.trim();
    if !kind_ok(kind) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "видът е goods, production или material"));
    }
    let inventory = form::i32_field(obj, "inventory_account_id")?.unwrap_or(0);
    let offset = form::i32_field(obj, "offset_account_id")?.unwrap_or(0);
    let mut clearing = form::i32_field(obj, "clearing_account_id")?.unwrap_or(0);
    account_ok(&st.db, inventory, company).await?;
    account_ok(&st.db, offset, company).await?;
    if kind == "production" {
        account_ok(&st.db, clearing, company).await?;
    } else {
        clearing = 0;
    }
    let id = invoice_read::nid(&st.db, "stock_account_rules").await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO stock_account_rules (id, company_id, kind, inventory_account_id, offset_account_id, clearing_account_id) \
             VALUES ({id}, {company}, {}, {inventory}, {offset}, {clearing})",
            lit(kind)
        ),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "kind": kind,
            "inventory_account_id": inventory,
            "offset_account_id": offset,
            "clearing_account_id": clearing,
        })),
    ))
}

pub async fn delete(State(st): State<AppState>, headers: HeaderMap, Path(id): Path<i32>) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT company_id FROM stock_account_rules WHERE id = {id}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "правилото не е намерено"));
    };
    if cell_i32_at(row, 0) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "правилото не е намерено"));
    }
    invoice_read::exe(&st.db, &format!("DELETE FROM stock_account_rules WHERE id = {id}")).await?;
    Ok(StatusCode::NO_CONTENT)
}
