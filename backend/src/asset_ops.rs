//! Амортизация и събития по ДМА. Един запис: Дт 603 / Кт 241.
//! И двете натрупани колони стават счетоводната сума.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::assets::{self, SEL};
use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, money};
use crate::form::{self, listed};
use crate::invoice_read;
use crate::periods;
use crate::posting::{self, Side};
use crate::{api_err, AppState, ApiError};

pub async fn preview(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = assets::company(&st, &headers).await?;
    let rows = active_rows(&st, company).await?;
    let mut items = Vec::new();
    let mut total = 0i128;
    for row in &rows {
        let amount = monthly(row);
        total = dec::add(total, amount).unwrap_or(total);
        items.push(json!({
            "id": cell_i32_at(row, 0),
            "name": cell_str(row, 1),
            "method": cell_str(row, 9),
            "monthly_amount": money(amount),
        }));
    }
    Ok(Json(json!({ "items": items, "total_amount": money(total) })))
}

pub async fn post_dep(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let company = assets::company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let period = form::required(form::str_field(obj, "period")?, "period")?;
    periods::assert_open(&st.db, &period).await?;
    let exp = posting::by_number(&st.db, company, "603")
        .await?
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "липсва сметка 603 (разход за амортизации)"))?;
    let acc = posting::by_number(&st.db, company, "241")
        .await?
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "липсва сметка 241 (амортизация на ДМА)"))?;
    let rows = active_rows(&st, company).await?;
    let mut total = 0i128;
    let mut next = Vec::new();
    for row in &rows {
        let amount = monthly(row);
        total = dec::add(total, amount).unwrap_or(total);
        let old = cell_str(row, 12);
        let accum = dec::add(dec::or0(&old), amount).unwrap_or(0);
        next.push((cell_i32_at(row, 0), money(accum), old));
    }
    let total_s = money(total);
    let entry = posting::write_entry(
        &st.db,
        company,
        &period,
        "depreciation",
        0,
        "",
        "Месечна амортизация на ДМА",
        0,
        "",
        &posting::month_of(&period),
        &[
            Side { account: exp, direction: "debit", amount: total_s.clone() },
            Side { account: acc, direction: "credit", amount: total_s.clone() },
        ],
    )
    .await?;
    for (i, (id, accum, _)) in next.iter().enumerate() {
        if let Err(e) = set_accum(&st, *id, accum).await {
            for (id2, _, old) in next.iter().take(i) {
                let _ = set_accum(&st, *id2, old).await;
            }
            let _ = invoice_read::wipe_journal(&st.db, entry).await;
            return Err(e);
        }
    }
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": entry,
            "document_type": "depreciation",
            "description": "Месечна амортизация на ДМА",
            "total_amount": total_s,
        })),
    ))
}

pub async fn revalue(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = assets::company(&st, &headers).await?;
    assets::need(id)?;
    let cur = assets::load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let new_value = money_of(&form::required(form::str_field(obj, "new_value")?, "new_value")?)?;
    let prev = cur.get("cost").and_then(|v| v.as_str()).unwrap_or("0.00");
    invoice_read::exe(&st.db, &format!("UPDATE fixed_assets SET cost = {} WHERE id = {id}", lit(&new_value))).await?;
    add_event(&st, id, "revaluation", &date_of(obj)?, &new_value, prev, "", &reason(obj)?).await?;
    assets::load(&st, id, company).await.map(Json)
}

pub async fn move_asset(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = assets::company(&st, &headers).await?;
    assets::need(id)?;
    let _ = assets::load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let location_id = form::i32_field(obj, "location_id")?.unwrap_or(0);
    let place = location_name(&st, location_id).await?;
    invoice_read::exe(&st.db, &format!("UPDATE fixed_assets SET location_id = {location_id} WHERE id = {id}")).await?;
    add_event(&st, id, "movement", &date_of(obj)?, "0.00", "", &place, &reason(obj)?).await?;
    assets::load(&st, id, company).await.map(Json)
}

pub async fn conserve(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    flag_event(&st, &headers, id, &body, 1, "conservation_start").await
}

pub async fn end_conserve(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    flag_event(&st, &headers, id, &body, 0, "conservation_end").await
}

pub async fn dispose(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = assets::company(&st, &headers).await?;
    assets::need(id)?;
    let _ = assets::load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let kind = form::str_field(obj, "disposal_type")?.unwrap_or_else(|| "write_off".into());
    let status = if kind == "sale" { "sold" } else { "disposed" };
    let amount = money_of(&form::str_field(obj, "sale_amount")?.unwrap_or_else(|| "0".into()))?;
    invoice_read::exe(&st.db, &format!("UPDATE fixed_assets SET status = {} WHERE id = {id}", lit(status))).await?;
    add_event(&st, id, "disposal", &date_of(obj)?, &amount, "", "", &reason(obj)?).await?;
    assets::load(&st, id, company).await.map(Json)
}

pub async fn events(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let company = assets::company(&st, &headers).await?;
    assets::need(id)?;
    let _ = assets::load(&st, id, company).await?;
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, asset_id, event_type, event_date, amount, prev_value, location, reason \
             FROM fixed_asset_events WHERE asset_id = {id} ORDER BY id"
        ),
    )
    .await?;
    let items = rows
        .iter()
        .map(|r| {
            json!({
                "id": cell_i32_at(r, 0),
                "asset_id": cell_i32_at(r, 1),
                "event_type": cell_str(r, 2),
                "event_date": cell_str(r, 3),
                "amount": cell_str(r, 4),
                "prev_value": cell_str(r, 5),
                "location": cell_str(r, 6),
                "reason": cell_str(r, 7),
            })
        })
        .collect();
    Ok(listed(items))
}

async fn flag_event(
    st: &AppState,
    headers: &HeaderMap,
    id: i32,
    body: &Value,
    flag: i32,
    kind: &str,
) -> Result<Json<Value>, ApiError> {
    let company = assets::company(st, headers).await?;
    assets::need(id)?;
    let _ = assets::load(st, id, company).await?;
    let obj = form::object(body)?;
    invoice_read::exe(&st.db, &format!("UPDATE fixed_assets SET is_conserved = {flag} WHERE id = {id}")).await?;
    add_event(st, id, kind, &date_of(obj)?, "0.00", "", "", &reason(obj)?).await?;
    assets::load(st, id, company).await.map(Json)
}

async fn active_rows(st: &AppState, company: i32) -> Result<Vec<crate::db::Row>, ApiError> {
    let rows = invoice_read::qry(&st.db, &format!("{SEL} WHERE company_id = {company} ORDER BY id")).await?;
    Ok(rows
        .into_iter()
        .filter(|r| cell_str(r, 14) == "active" && cell_i32_at(r, 19) == 0)
        .collect())
}

fn monthly(row: &crate::db::Row) -> i128 {
    let cost = dec::or0(&cell_str(row, 6));
    let salvage = dec::or0(&cell_str(row, 7));
    let accum = dec::or0(&cell_str(row, 12));
    let rate = dec::or0(&cell_str(row, 10));
    let base = if cell_str(row, 9) == "declining" {
        dec::sub(cost, accum).unwrap_or(0)
    } else {
        dec::sub(cost, salvage).unwrap_or(0)
    };
    let base = if base < 0 { 0 } else { base };
    let hundred = dec::parse("100").unwrap_or(1);
    let twelve = dec::parse("12").unwrap_or(1);
    let year = dec::div(dec::mul(base, rate).unwrap_or(0), hundred).unwrap_or(0);
    let raw = dec::div(year, twelve).unwrap_or(0);
    dec::parse(&money(raw)).unwrap_or(0)
}

async fn set_accum(st: &AppState, id: i32, accum: &str) -> Result<(), ApiError> {
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE fixed_assets SET accumulated_depreciation = {}, tax_accumulated_depreciation = {} WHERE id = {id}",
            lit(accum),
            lit(accum)
        ),
    )
    .await
}

async fn add_event(
    st: &AppState,
    asset_id: i32,
    kind: &str,
    date: &str,
    amount: &str,
    prev: &str,
    location: &str,
    reason: &str,
) -> Result<(), ApiError> {
    let id = invoice_read::nid(&st.db, "fixed_asset_events").await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO fixed_asset_events (id, asset_id, event_type, event_date, amount, prev_value, location, reason) \
             VALUES ({id}, {asset_id}, {}, {}, {}, {}, {}, {})",
            lit(kind),
            lit(date),
            lit(amount),
            lit(prev),
            lit(location),
            lit(reason)
        ),
    )
    .await
}

async fn location_name(st: &AppState, id: i32) -> Result<String, ApiError> {
    if id <= 0 {
        return Ok(String::new());
    }
    let rows = invoice_read::qry(&st.db, &format!("SELECT name FROM company_locations WHERE id = {id}")).await?;
    Ok(rows.first().map(|r| cell_str(r, 0)).filter(|s| !s.is_empty()).unwrap_or_else(|| id.to_string()))
}

fn date_of(obj: &serde_json::Map<String, Value>) -> Result<String, ApiError> {
    Ok(form::str_field(obj, "event_date")?.unwrap_or_default())
}

fn reason(obj: &serde_json::Map<String, Value>) -> Result<String, ApiError> {
    Ok(form::str_field(obj, "reason")?.unwrap_or_default())
}

fn money_of(raw: &str) -> Result<String, ApiError> {
    match dec::parse(raw) {
        Some(v) => Ok(money(v)),
        None => Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност")),
    }
}
