//! Счетоводни месеци. Липсващ ред е отворен период.
//! Затворен месец спира пост на фактура и запис на начално салдо.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::form::{self, auth_sub};
use crate::invoice_read;
use crate::{api_err, AppState, ApiError};

#[derive(Deserialize, Default)]
pub(crate) struct YearQ {
    year: Option<String>,
}

fn key_of(date: &str) -> &str {
    if date.len() >= 7 && date.is_char_boundary(7) {
        &date[..7]
    } else {
        date
    }
}

fn parts(key: &str) -> (i32, i32) {
    let year = key.get(..4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let month = if key.len() >= 7 {
        key.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(0)
    } else {
        0
    };
    (year, month)
}

/// Невалидна дата минава. Затворен месец е 422.
pub async fn assert_open(db: &crate::db::Db, date_or_month: &str) -> Result<(), ApiError> {
    let key = key_of(date_or_month);
    let (year, month) = parts(key);
    if year < 1 || !(1..=12).contains(&month) {
        return Ok(());
    }
    let rows = invoice_read::qry(
        db,
        &format!("SELECT month, status FROM accounting_periods WHERE year = {year}"),
    )
    .await?;
    for row in rows {
        if cell_i32_at(&row, 0) == month && cell_str(&row, 1) == "closed" {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("периодът {key} е затворен"),
            ));
        }
    }
    Ok(())
}

fn year_of(raw: Option<&str>) -> i32 {
    let n = raw.unwrap_or("").trim().parse::<i32>().unwrap_or(0);
    if n < 2000 { 2026 } else { n }
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

async fn rows_for(db: &crate::db::Db, year: i32) -> Result<Vec<crate::db::Row>, ApiError> {
    invoice_read::qry(
        db,
        &format!(
            "SELECT id, month, status, closed_at, notes FROM accounting_periods WHERE year = {year} ORDER BY id"
        ),
    )
    .await
}

fn list_json(rows: &[crate::db::Row], year: i32) -> Value {
    let mut items = Vec::new();
    for month in 1..=12 {
        let mut status = "open".to_string();
        let mut closed_at = String::new();
        let mut notes = String::new();
        let mut id = 0;
        for row in rows {
            if cell_i32_at(row, 1) != month {
                continue;
            }
            let st = cell_str(row, 2);
            if !st.is_empty() {
                status = st;
            }
            closed_at = cell_str(row, 3);
            notes = cell_str(row, 4);
            id = cell_i32_at(row, 0);
        }
        let mut item = json!({
            "year": year,
            "month": month,
            "status": status,
            "closed_at": closed_at,
            "notes": notes,
        });
        if id > 0 {
            item["id"] = json!(id);
        }
        items.push(item);
    }
    json!({ "items": items, "year": year })
}

async fn year_list(db: &crate::db::Db, year: i32) -> Result<Value, ApiError> {
    let rows = rows_for(db, year).await?;
    Ok(list_json(&rows, year))
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<YearQ>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    Ok(Json(year_list(&st.db, year_of(q.year.as_deref())).await?))
}

fn body_year_month(body: &Value) -> Result<(i32, i32, &Map<String, Value>), ApiError> {
    let obj = form::object(body)?;
    Ok((req_i32(obj, "year")?, req_i32(obj, "month")?, obj))
}

pub async fn close(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let (year, month, obj) = body_year_month(&body)?;
    if !(1..=12).contains(&month) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "month трябва да е 1–12",
        ));
    }
    let notes = form::str_field(obj, "notes")?.unwrap_or_default();
    let closed_at = form::str_field(obj, "closed_at")?.unwrap_or_default();
    let rows = rows_for(&st.db, year).await?;
    let mut id = 0;
    for row in &rows {
        if cell_i32_at(row, 1) == month {
            id = cell_i32_at(row, 0);
        }
    }
    if id > 0 {
        invoice_read::exe(
            &st.db,
            &format!(
                "UPDATE accounting_periods SET status = 'closed', closed_at = {}, notes = {} WHERE id = {id}",
                lit(&closed_at),
                lit(&notes)
            ),
        )
        .await?;
    } else {
        let nid = invoice_read::nid(&st.db, "accounting_periods").await?;
        invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO accounting_periods (id, year, month, status, closed_at, notes) VALUES ({nid}, {year}, {month}, 'closed', {}, {})",
                lit(&closed_at),
                lit(&notes)
            ),
        )
        .await?;
    }
    Ok(Json(year_list(&st.db, year).await?))
}

pub async fn reopen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let (year, month, _) = body_year_month(&body)?;
    let rows = rows_for(&st.db, year).await?;
    let mut id = 0;
    for row in &rows {
        if cell_i32_at(row, 1) == month {
            id = cell_i32_at(row, 0);
        }
    }
    if id > 0 {
        invoice_read::exe(
            &st.db,
            &format!(
                "UPDATE accounting_periods SET status = 'open', closed_at = '', notes = '' WHERE id = {id}"
            ),
        )
        .await?;
    }
    Ok(Json(year_list(&st.db, year).await?))
}
