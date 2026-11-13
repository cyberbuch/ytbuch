//! Курсове на ЕЦБ за 1 EUR. Дневният фид и историята за 90 дни.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{self, cell_i32_at, cell_str, lit};
use crate::form::{self, auth_sub, db_err};
use crate::httpx;
use crate::{api_err, AppState, ApiError};

const DAILY: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
const HISTORY: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist-90d.xml";

#[derive(Deserialize)]
pub struct DateQuery {
    date: Option<String>,
}

#[derive(Deserialize)]
pub struct RateQuery {
    currency: Option<String>,
    date: Option<String>,
}

#[derive(Deserialize)]
pub struct CrossQuery {
    from: Option<String>,
    to: Option<String>,
    date: Option<String>,
}

/// Курс за 1 EUR към дадена валута, as-of към най-близката предходна дата.
/// За EUR връща 1. Връща (курс, дата на курса).
async fn per_eur(db: &crate::db::Db, code: &str, date: &str) -> Result<(String, String), ApiError> {
    if code == "EUR" {
        return Ok(("1".to_string(), date.to_string()));
    }
    let rows = db
        .query(&format!(
            "SELECT rate, rate_date FROM exchange_rates WHERE currency_code = {}",
            lit(code)
        ))
        .await
        .map_err(db_err)?;
    let mut best: Option<(String, String)> = None;
    for row in &rows {
        let day = cell_str(row, 1);
        if day.as_str() <= date
            && best.as_ref().map(|(d, _)| day.as_str() >= d.as_str()).unwrap_or(true)
        {
            best = Some((day, cell_str(row, 0)));
        }
    }
    best.ok_or_else(|| api_err(StatusCode::NOT_FOUND, format!("няма курс за {code}")))
}

fn row_json(id: i32, code: &str, rate: &str, date: &str) -> Value {
    json!({
        "id": id,
        "currency_code": code,
        "rate": rate,
        "date": date,
    })
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<DateQuery>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let mut date = q.date.unwrap_or_default().trim().to_string();
    if date.is_empty() {
        let rows = st
            .db
            .query("SELECT rate_date FROM exchange_rates ORDER BY rate_date DESC")
            .await
            .map_err(db_err)?;
        date = rows.first().map(|r| cell_str(r, 0)).unwrap_or_default();
    }
    if date.is_empty() {
        return Ok(Json(json!({ "date": "", "base": "EUR", "items": [] })));
    }
    let rows = st
        .db
        .query(&format!(
            "SELECT id, currency_code, rate, rate_date FROM exchange_rates WHERE rate_date = {} ORDER BY currency_code",
            lit(&date)
        ))
        .await
        .map_err(db_err)?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| row_json(cell_i32_at(r, 0), &cell_str(r, 1), &cell_str(r, 2), &cell_str(r, 3)))
        .collect();
    Ok(Json(json!({ "date": date, "base": "EUR", "items": items })))
}

pub async fn rate(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<RateQuery>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let currency = q.currency.unwrap_or_default().trim().to_ascii_uppercase();
    let date = q.date.unwrap_or_default();
    if currency.is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "currency е задължителен"));
    }
    if date.trim().is_empty() {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            "date е задължителен (YYYY-MM-DD)",
        ));
    }
    let rows = st
        .db
        .query(&format!(
            "SELECT rate, rate_date FROM exchange_rates WHERE currency_code = {}",
            lit(&currency)
        ))
        .await
        .map_err(db_err)?;
    let want = date.trim();
    let mut best: Option<(String, String)> = None;
    for row in &rows {
        let day = cell_str(row, 1);
        if day.as_str() <= want && best.as_ref().map(|(d, _)| day.as_str() >= d.as_str()).unwrap_or(true) {
            best = Some((day, cell_str(row, 0)));
        }
    }
    let Some((day, rate)) = best else {
        return Err(api_err(StatusCode::NOT_FOUND, "няма курс за тази валута"));
    };
    Ok(Json(json!({
        "currency_code": currency,
        "date": day,
        "rate": rate,
        "base": "EUR",
    })))
}

/// Кръстосан курс: ?from=USD&to=BGN&date=… → единици `to` за 1 `from`,
/// през EUR (ЕЦБ курсовете са за 1 EUR), as-of към предходна дата.
pub async fn cross(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<CrossQuery>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let from = q.from.unwrap_or_default().trim().to_ascii_uppercase();
    let to = q.to.unwrap_or_default().trim().to_ascii_uppercase();
    let date = q.date.unwrap_or_default().trim().to_string();
    if from.is_empty() || to.is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "from и to са задължителни"));
    }
    if date.is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "date е задължителен (YYYY-MM-DD)"));
    }
    let (rf, _) = per_eur(&st.db, &from, &date).await?;
    let (rt, rate_date) = per_eur(&st.db, &to, &date).await?;
    let f: f64 = rf.parse().map_err(|_| api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалиден курс"))?;
    let t: f64 = rt.parse().map_err(|_| api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалиден курс"))?;
    if f == 0.0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "курсът е 0"));
    }
    Ok(Json(json!({
        "from": from,
        "to": to,
        "rate": format!("{:.6}", t / f),
        "rate_date": rate_date,
        "requested_date": date,
        "base": "EUR",
    })))
}

fn quoted(tag: &str, key: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{key}={quote}");
        let Some(pos) = tag.find(&needle) else { continue };
        let rest = &tag[pos + needle.len()..];
        let Some(stop) = rest.find(quote) else { continue };
        return Some(rest[..stop].to_string());
    }
    None
}

fn cubes(xml: &str) -> Result<Vec<(String, String, String)>, String> {
    let mut out = Vec::new();
    let mut date = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if rest.starts_with('/') || rest.starts_with('!') || rest.starts_with('?') {
            continue;
        }
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        if let Some(day) = quoted(tag, "time") {
            date = day;
            continue;
        }
        let (Some(code), Some(rate)) = (quoted(tag, "currency"), quoted(tag, "rate")) else {
            continue;
        };
        if date.is_empty() {
            continue;
        }
        out.push((code, rate, date.clone()));
    }
    if out.is_empty() && xml.contains('<') {
        return Err("невалиден XML от ЕЦБ".into());
    }
    Ok(out)
}

pub async fn import(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let mut mode = "daily".to_string();
    if let Some(Json(value)) = body {
        if let Some(obj) = value.as_object() {
            if let Some(v) = form::str_field(obj, "mode")? {
                if !v.trim().is_empty() {
                    mode = v.trim().to_string();
                }
            }
        }
    }
    let url = if mode == "history" { HISTORY } else { DAILY };
    if mode != "daily" && mode != "history" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "mode трябва да е daily или history",
        ));
    }
    let (status, xml) = httpx::get_text(url, 40)
        .await
        .map_err(|_| api_err(StatusCode::BAD_GATEWAY, "ЕЦБ фидът е недостъпен"))?;
    if status != 200 {
        return Err(api_err(
            StatusCode::BAD_GATEWAY,
            format!("ЕЦБ фидът върна статус {status} — опитайте пак"),
        ));
    }
    let parsed = cubes(&xml).map_err(|e| api_err(StatusCode::BAD_GATEWAY, e))?;
    let existing = st
        .db
        .query("SELECT id, currency_code, rate, rate_date FROM exchange_rates")
        .await
        .map_err(db_err)?;
    let mut known: std::collections::HashMap<(String, String), (i32, String)> = std::collections::HashMap::new();
    let mut next = 0;
    for row in &existing {
        let id = cell_i32_at(row, 0);
        if id > next {
            next = id;
        }
        known.insert((cell_str(row, 1), cell_str(row, 3)), (id, cell_str(row, 2)));
    }
    let mut inserted = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let mut first = String::new();
    let mut last = String::new();
    let stamp = db::now_stamp();
    for (code, rate, date) in parsed {
        if first.is_empty() || date < first {
            first = date.clone();
        }
        if last.is_empty() || date > last {
            last = date.clone();
        }
        let key = (code.clone(), date.clone());
        if let Some((id, old)) = known.get(&key) {
            if old == &rate {
                unchanged += 1;
                continue;
            }
            st.db
                .exec(&format!(
                    "UPDATE exchange_rates SET rate = {} WHERE id = {id}",
                    lit(&rate)
                ))
                .await
                .map_err(db_err)?;
            updated += 1;
            continue;
        }
        next += 1;
        st.db
            .exec(&format!(
                "INSERT INTO exchange_rates (id, currency_code, rate, rate_date, created_at) VALUES ({next}, {}, {}, {}, {})",
                lit(&code),
                lit(&rate),
                lit(&date),
                lit(&stamp)
            ))
            .await
            .map_err(db_err)?;
        known.insert(key, (next, rate));
        inserted += 1;
    }
    Ok(Json(json!({
        "mode": mode,
        "inserted": inserted,
        "updated": updated,
        "unchanged": unchanged,
        "from_date": first,
        "to_date": last,
    })))
}
