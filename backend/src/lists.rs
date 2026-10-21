//! Списъци за демо фирмата: сметки, контрагенти, фактури, дневник, настройки.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{self, cell_i32_at, cell_str};
use crate::pagination::{paginate, PageQuery};
use crate::{api_err, auth, companies, AppState, ApiError};

async fn rows(st: &AppState, headers: &HeaderMap, sql: &str) -> Result<Vec<db::Row>, ApiError> {
    auth::subject(&st.jwt_secret, headers)?;
    st.db
        .query(sql)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn accounts(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let company = crate::form::active_or_one(&st, &sub).await?;
    let rows = rows(
        &st,
        &headers,
        &format!(
            "SELECT id, number, name, saft_account_type, analytic_type FROM accounts WHERE company_id = {company} ORDER BY number"
        ),
    )
    .await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": cell_i32_at(r, 0),
                "number": cell_str(r, 1),
                "name": cell_str(r, 2),
                "saft_account_type": cell_str(r, 3),
                "analytic_type": cell_str(r, 4),
            })
        })
        .collect();
    Ok(paginate(items, &q))
}

pub async fn products(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let company = crate::form::active_or_one(&st, &sub).await?;
    let rows = rows(
        &st,
        &headers,
        &format!(
            "SELECT id, name, code, description, unit, price, cost_price, vat_rate, is_service, is_inventory, inventory_account_id, revenue_account_id, stock_quantity, stock_value, is_active FROM products WHERE company_id = {company} ORDER BY id"
        ),
    )
    .await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": cell_i32_at(r, 0),
                "name": cell_str(r, 1),
                "code": cell_str(r, 2),
                "description": cell_str(r, 3),
                "unit": cell_str(r, 4),
                "price": cell_str(r, 5),
                "cost_price": cell_str(r, 6),
                "vat_rate": cell_str(r, 7),
                "is_service": cell_i32_at(r, 8),
                "is_inventory": cell_i32_at(r, 9),
                "inventory_account_id": cell_i32_at(r, 10),
                "revenue_account_id": cell_i32_at(r, 11),
                "stock_quantity": cell_str(r, 12),
                "stock_value": cell_str(r, 13),
                "is_active": cell_i32_at(r, 14),
            })
        })
        .collect();
    Ok(paginate(items, &q))
}

fn entry_json(r: &db::Row) -> Value {
    json!({
        "id": cell_i32_at(r, 0),
        "entry_date": cell_str(r, 1),
        "document_type": cell_str(r, 2),
        "document_id": cell_i32_at(r, 3),
        "document_number": cell_str(r, 4),
        "description": cell_str(r, 5),
        "vat_type": cell_str(r, 6),
        "counterpart_id": cell_i32_at(r, 7),
        "counterpart_name": cell_str(r, 8),
        "document_date": cell_str(r, 9),
        "vat_period": cell_str(r, 10),
    })
}

pub async fn journal(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let company = crate::form::active_or_one(&st, &sub).await?;
    let stamps = crate::audit::stamps_for(&st.db, company, "journal").await?;
    let rows = rows(
        &st,
        &headers,
        &format!(
            "SELECT id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period FROM journal_entries WHERE company_id = {company} ORDER BY id"
        ),
    )
    .await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            let id = cell_i32_at(r, 0);
            let mut v = entry_json(r);
            crate::audit::attach_stamp(&mut v, stamps.get(&id));
            v
        })
        .collect();
    Ok(paginate(items, &q))
}

pub async fn journal_show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let company = crate::form::active_or_one(&st, &sub).await?;
    let mut rows = rows(
        &st,
        &headers,
        &format!(
            "SELECT id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period FROM journal_entries WHERE id = {id} AND company_id = {company}"
        ),
    )
    .await?;
    let Some(row) = rows.pop() else {
        return Err(api_err(StatusCode::NOT_FOUND, "записът не е намерен"));
    };
    let mut value = entry_json(&row);
    let lines = st
        .db
        .query(&format!(
            "SELECT id, account_id, direction, amount, vat_amount, account_number, account_name FROM journal_lines WHERE entry_id = {id} ORDER BY id"
        ))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    value["lines"] = json!(lines
        .iter()
        .map(|r| json!({
            "id": cell_i32_at(r, 0),
            "account_id": cell_i32_at(r, 1),
            "direction": cell_str(r, 2),
            "amount": cell_str(r, 3),
            "vat_amount": cell_str(r, 4),
            "account_number": cell_str(r, 5),
            "account_name": cell_str(r, 6),
        }))
        .collect::<Vec<_>>());
    let stamps = crate::audit::stamps_for(&st.db, company, "journal").await?;
    crate::audit::attach_stamp(&mut value, stamps.get(&id));
    value["audit_history"] = json!(crate::audit::history_for(&st.db, company, "journal", id).await?);
    Ok(Json(value))
}

pub async fn company_settings(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let company = crate::form::active_or_one(&st, &sub).await?;
    if id != company {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    }
    let Some(company) = companies::json_by_id(&st.db, id).await? else {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    };
    let locations = st
        .db
        .query(&format!(
            "SELECT id, name, location_type, street_name, building_number, city, post_code, region, country, is_main FROM company_locations WHERE company_id = {id} ORDER BY id"
        ))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let owners = st
        .db
        .query(&format!(
            "SELECT id, first_name_bg, last_name_bg, egn, first_name_latin, last_name_latin, country, ownership_percentage FROM beneficial_owners WHERE company_id = {id} ORDER BY id"
        ))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let parents = st
        .db
        .query(&format!(
            "SELECT id, name_bg, uic, name_latin, country FROM ultimate_parents WHERE company_id = {id} ORDER BY id"
        ))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let series = st
        .db
        .query(&format!(
            "SELECT id, name, document_types, pad_width, start_number, next_number FROM document_series WHERE company_id = {id} ORDER BY id"
        ))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({
        "company": company,
        "locations": locations.iter().map(|r| json!({
            "id": cell_i32_at(r, 0),
            "name": cell_str(r, 1),
            "location_type": cell_str(r, 2),
            "street_name": cell_str(r, 3),
            "building_number": cell_str(r, 4),
            "city": cell_str(r, 5),
            "post_code": cell_str(r, 6),
            "region": cell_str(r, 7),
            "country": cell_str(r, 8),
            "is_main": cell_i32_at(r, 9),
        })).collect::<Vec<_>>(),
        "beneficial_owners": owners.iter().map(|r| json!({
            "id": cell_i32_at(r, 0),
            "first_name_bg": cell_str(r, 1),
            "last_name_bg": cell_str(r, 2),
            "egn": cell_str(r, 3),
            "first_name_latin": cell_str(r, 4),
            "last_name_latin": cell_str(r, 5),
            "country": cell_str(r, 6),
            "ownership_percentage": cell_str(r, 7),
        })).collect::<Vec<_>>(),
        "ultimate_parents": parents.iter().map(|r| json!({
            "id": cell_i32_at(r, 0),
            "name_bg": cell_str(r, 1),
            "uic": cell_str(r, 2),
            "name_latin": cell_str(r, 3),
            "country": cell_str(r, 4),
        })).collect::<Vec<_>>(),
        "document_series": series.iter().map(|r| json!({
            "id": cell_i32_at(r, 0),
            "name": cell_str(r, 1),
            "document_types": cell_str(r, 2),
            "pad_width": cell_i32_at(r, 3),
            "start_number": cell_i32_at(r, 4),
            "next_number": cell_str(r, 5),
        })).collect::<Vec<_>>(),
    })))
}
