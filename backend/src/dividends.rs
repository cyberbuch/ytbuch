//! Дивиденти към действителните собственици. Плащането не пише в дневника.
//! Собствениците нямат колона is_active.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, money};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_read;
use crate::{api_err, AppState, ApiError};

const DIST: &str = "SELECT id, year, total_amount, decision_date, decision_number, status, notes, company_id FROM dividend_distributions";

struct Owner {
    id: i32,
    first: String,
    last: String,
    egn: String,
    pct: String,
    share: i128,
}

struct Share {
    owner: Owner,
    gross: i128,
    tax: i128,
    net: i128,
    rate: String,
}

pub async fn index(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let owners = owners_of(&st, company).await?;
    let currency = currency_of(&st, company).await?;
    let rows = invoice_read::qry(&st.db, &format!("{DIST} WHERE company_id = {company} ORDER BY id")).await?;
    let kids = kids_of(&st, company).await?;
    let mut items = Vec::new();
    for row in &rows {
        items.push(pack(row, &kids, false));
    }
    let count = items.len();
    Ok(Json(json!({
        "items": items,
        "count": count,
        "owners": owners.iter().map(owner_json).collect::<Vec<_>>(),
        "currency": currency,
    })))
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
    let year = year_of(form::i32_field(obj, "year")?)?;
    let total_s = form::required(form::str_field(obj, "total_amount")?, "total_amount")?;
    let total = positive(&total_s)?;
    let date = form::required(form::str_field(obj, "decision_date")?, "decision_date")?;
    let rate = rate_of(&form::str_field(obj, "tax_rate")?.unwrap_or_else(|| "5".into()))?;
    let number = form::str_field(obj, "decision_number")?.unwrap_or_default();
    let notes = form::str_field(obj, "notes")?.unwrap_or_default();
    let shares = split(&st, company, total, &rate).await?;
    let id = invoice_read::nid(&st.db, "dividend_distributions").await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO dividend_distributions (id, company_id, year, total_amount, decision_date, decision_number, status, notes) \
             VALUES ({id}, {company}, {year}, {}, {}, {}, 'draft', {})",
            lit(&money(total)), lit(&date), lit(&number), lit(&notes)
        ),
    )
    .await?;
    if let Err(e) = insert_shares(&st, company, id, &date, &shares).await {
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
    let cur = load(&st, id, company).await?;
    let obj = form::object(&body)?;
    if touches(obj) && any_paid(&cur) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Не може да се променя сумата или данъкът при изплатени дивиденти. Първо отменете плащанията.",
        ));
    }
    if let Some(status) = form::str_field(obj, "status")? {
        if !matches!(status.as_str(), "draft" | "approved" | "partially_paid" | "paid") {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалиден статус"));
        }
        invoice_read::exe(&st.db, &format!("UPDATE dividend_distributions SET status = {} WHERE id = {id}", lit(&status))).await?;
    }
    if touches(obj) {
        let year = year_of(Some(pick_i32(obj, &cur, "year")?))?;
        let total = positive(&pick_str(obj, &cur, "total_amount")?)?;
        let date = {
            let d = pick_str(obj, &cur, "decision_date")?;
            if d.trim().is_empty() {
                return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "decision_date е задължително"));
            }
            d
        };
        let rate = rate_of(&form::str_field(obj, "tax_rate")?.unwrap_or_else(|| current_rate(&cur)))?;
        let number = pick_str(obj, &cur, "decision_number")?;
        let notes = pick_str(obj, &cur, "notes")?;
        let shares = split(&st, company, total, &rate).await?;
        invoice_read::exe(
            &st.db,
            &format!(
                "UPDATE dividend_distributions SET year = {year}, total_amount = {}, decision_date = {}, decision_number = {}, notes = {} WHERE id = {id}",
                lit(&money(total)), lit(&date), lit(&number), lit(&notes)
            ),
        )
        .await?;
        invoice_read::exe(&st.db, &format!("DELETE FROM dividends WHERE distribution_id = {id}")).await?;
        if let Err(e) = insert_shares(&st, company, id, &date, &shares).await {
            return Err(e);
        }
    } else {
        if let Some(number) = form::str_field(obj, "decision_number")? {
            invoice_read::exe(&st.db, &format!("UPDATE dividend_distributions SET decision_number = {} WHERE id = {id}", lit(&number))).await?;
        }
        if let Some(notes) = form::str_field(obj, "notes")? {
            invoice_read::exe(&st.db, &format!("UPDATE dividend_distributions SET notes = {} WHERE id = {id}", lit(&notes))).await?;
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
    let cur = load(&st, id, company).await?;
    if any_paid(&cur) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Не може да се изтрие разпределение с изплатени дивиденти."));
    }
    if cur.get("status").and_then(|v| v.as_str()) != Some("draft") {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Може да се изтрие само чернова. Първо върнете разпределението в чернова.",
        ));
    }
    wipe(&st, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn pay(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    if id <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid id"));
    }
    let obj = form::object(&body)?;
    let Some(paid) = form::flag_field(obj, "is_paid")? else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "is_paid е задължително"));
    };
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT distribution_id, decision_date FROM dividends WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Дивидентът не е намерен"));
    };
    let dist_id = cell_i32_at(row, 0);
    let decision = cell_str(row, 1);
    let parent = load(&st, dist_id, company).await?;
    if parent.get("status").and_then(|v| v.as_str()) == Some("draft") {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Одобрете разпределението преди изплащане"));
    }
    let payment_date = if paid == 0 {
        String::new()
    } else {
        let given = form::str_field(obj, "payment_date")?.unwrap_or_default();
        if given.trim().is_empty() { decision } else { given }
    };
    invoice_read::exe(
        &st.db,
        &format!("UPDATE dividends SET is_paid = {paid}, payment_date = {} WHERE id = {id}", lit(&payment_date)),
    )
    .await?;
    refresh_status(&st, dist_id, parent.get("status").and_then(|v| v.as_str()).unwrap_or("approved")).await?;
    Ok(Json(load(&st, dist_id, company).await?))
}

async fn split(st: &AppState, company: i32, total: i128, rate: &str) -> Result<Vec<Share>, ApiError> {
    let owners: Vec<Owner> = owners_of(st, company).await?.into_iter().filter(|o| o.share > 0).collect();
    if owners.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Няма действителни собственици с процент. Добавете ги в Настройки → SAF-T.",
        ));
    }
    let sum_pct: i128 = owners.iter().map(|o| o.share).sum();
    if sum_pct <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Общият процент на собственост е 0"));
    }
    let rate_v = dec::or0(rate);
    let hundred = dec::parse("100").unwrap_or(1);
    let n = owners.len();
    let mut out = Vec::new();
    let mut assigned = 0i128;
    for (i, owner) in owners.into_iter().enumerate() {
        let gross = if i + 1 == n {
            dec::sub(total, assigned).unwrap_or(0)
        } else {
            let raw = dec::div(dec::mul(total, owner.share).unwrap_or(0), sum_pct).unwrap_or(0);
            let g = dec::parse(&money(raw)).unwrap_or(0);
            assigned = dec::add(assigned, g).unwrap_or(assigned);
            g
        };
        let tax_raw = dec::div(dec::mul(gross, rate_v).unwrap_or(0), hundred).unwrap_or(0);
        let tax = dec::parse(&money(tax_raw)).unwrap_or(0);
        let net = dec::sub(gross, tax).unwrap_or(0);
        out.push(Share { owner, gross, tax, net, rate: rate.to_string() });
    }
    Ok(out)
}

async fn insert_shares(st: &AppState, company: i32, dist_id: i32, date: &str, shares: &[Share]) -> Result<(), ApiError> {
    for share in shares {
        let id = invoice_read::nid(&st.db, "dividends").await?;
        let name = format!("{} {}", share.owner.first, share.owner.last).trim().to_string();
        invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO dividends (id, company_id, distribution_id, beneficial_owner_id, owner_name, owner_egn, ownership_percentage, gross_amount, tax_rate, tax_amount, net_amount, decision_date, payment_date, is_paid, notes) \
                 VALUES ({id}, {company}, {dist_id}, {}, {}, {}, {}, {}, {}, {}, {}, {}, '', 0, '')",
                share.owner.id,
                lit(&name),
                lit(&share.owner.egn),
                lit(&share.owner.pct),
                lit(&money(share.gross)),
                lit(&share.rate),
                lit(&money(share.tax)),
                lit(&money(share.net)),
                lit(date)
            ),
        )
        .await?;
    }
    Ok(())
}

async fn owners_of(st: &AppState, company: i32) -> Result<Vec<Owner>, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, first_name_bg, last_name_bg, egn, ownership_percentage FROM beneficial_owners WHERE company_id = {company} ORDER BY id"
        ),
    )
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let pct = cell_str(r, 4);
            Owner {
                id: cell_i32_at(r, 0),
                first: cell_str(r, 1),
                last: cell_str(r, 2),
                egn: cell_str(r, 3),
                share: dec::or0(&pct),
                pct,
            }
        })
        .collect())
}

fn owner_json(o: &Owner) -> Value {
    json!({
        "id": o.id,
        "first_name_bg": o.first,
        "last_name_bg": o.last,
        "egn": o.egn,
        "ownership_percentage": o.pct,
    })
}

async fn currency_of(st: &AppState, company: i32) -> Result<String, ApiError> {
    let rows = invoice_read::qry(&st.db, &format!("SELECT currency FROM companies WHERE id = {company}")).await?;
    let c = rows.first().map(|r| cell_str(r, 0)).unwrap_or_default();
    Ok(if c.trim().is_empty() { "BGN".into() } else { c })
}

async fn kids_of(st: &AppState, company: i32) -> Result<Vec<crate::db::Row>, ApiError> {
    invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, distribution_id, beneficial_owner_id, owner_name, owner_egn, ownership_percentage, gross_amount, tax_rate, tax_amount, net_amount, is_paid, payment_date \
             FROM dividends WHERE company_id = {company} ORDER BY id"
        ),
    )
    .await
}

fn pack(row: &crate::db::Row, kids: &[crate::db::Row], with_divs: bool) -> Value {
    let id = cell_i32_at(row, 0);
    let mine: Vec<&crate::db::Row> = kids.iter().filter(|r| cell_i32_at(r, 1) == id).collect();
    let mut total_net = 0i128;
    let mut total_tax = 0i128;
    let mut total_paid = 0i128;
    for kid in &mine {
        let net = dec::or0(&cell_str(kid, 9));
        total_net = dec::add(total_net, net).unwrap_or(total_net);
        total_tax = dec::add(total_tax, dec::or0(&cell_str(kid, 8))).unwrap_or(total_tax);
        if cell_i32_at(kid, 10) != 0 {
            total_paid = dec::add(total_paid, net).unwrap_or(total_paid);
        }
    }
    let mut item = json!({
        "id": id,
        "year": cell_i32_at(row, 1),
        "total_amount": cell_str(row, 2),
        "decision_date": cell_str(row, 3),
        "decision_number": cell_str(row, 4),
        "status": cell_str(row, 5),
        "notes": cell_str(row, 6),
        "dividends_count": mine.len(),
        "total_net": money(total_net),
        "total_paid": money(total_paid),
        "total_tax": money(total_tax),
    });
    if with_divs {
        item["dividends"] = json!(mine.iter().map(|r| json!({
            "id": cell_i32_at(r, 0),
            "beneficial_owner_id": cell_i32_at(r, 2),
            "owner_name": cell_str(r, 3),
            "owner_egn": cell_str(r, 4),
            "ownership_percentage": cell_str(r, 5),
            "gross_amount": cell_str(r, 6),
            "tax_rate": cell_str(r, 7),
            "tax_amount": cell_str(r, 8),
            "net_amount": cell_str(r, 9),
            "is_paid": cell_i32_at(r, 10),
            "payment_date": cell_str(r, 11),
        })).collect::<Vec<_>>());
    }
    item
}

async fn load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid id"));
    }
    let rows = invoice_read::qry(&st.db, &format!("{DIST} WHERE id = {id}")).await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Разпределението не е намерено"));
    };
    if cell_i32_at(row, 7) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "Разпределението не е намерено"));
    }
    let kids = kids_of(st, company).await?;
    Ok(pack(row, &kids, true))
}

async fn refresh_status(st: &AppState, id: i32, old: &str) -> Result<(), ApiError> {
    let rows = invoice_read::qry(&st.db, &format!("SELECT is_paid FROM dividends WHERE distribution_id = {id}")).await?;
    let n = rows.len();
    let paid = rows.iter().filter(|r| cell_i32_at(r, 0) != 0).count();
    let status = if n > 0 && paid == n {
        "paid"
    } else if paid > 0 {
        "partially_paid"
    } else if old == "paid" || old == "partially_paid" {
        "approved"
    } else {
        old
    };
    invoice_read::exe(&st.db, &format!("UPDATE dividend_distributions SET status = {} WHERE id = {id}", lit(status))).await
}

async fn wipe(st: &AppState, id: i32) -> Result<(), ApiError> {
    invoice_read::exe(&st.db, &format!("DELETE FROM dividends WHERE distribution_id = {id}")).await?;
    invoice_read::exe(&st.db, &format!("DELETE FROM dividend_distributions WHERE id = {id}")).await
}

fn touches(obj: &Map<String, Value>) -> bool {
    ["year", "total_amount", "tax_rate", "decision_date"].iter().any(|k| obj.contains_key(*k))
}

fn any_paid(cur: &Value) -> bool {
    cur.get("dividends")
        .and_then(|v| v.as_array())
        .map(|rows| rows.iter().any(|r| r.get("is_paid").and_then(|v| v.as_i64()).unwrap_or(0) != 0))
        .unwrap_or(false)
}

fn current_rate(cur: &Value) -> String {
    cur.get("dividends")
        .and_then(|v| v.as_array())
        .and_then(|rows| rows.first())
        .and_then(|r| r.get("tax_rate"))
        .and_then(|v| v.as_str())
        .unwrap_or("5")
        .to_string()
}

fn year_of(value: Option<i32>) -> Result<i32, ApiError> {
    let year = value.unwrap_or(0);
    if !(1990..=2100).contains(&year) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна финансова година"));
    }
    Ok(year)
}

fn positive(raw: &str) -> Result<i128, ApiError> {
    let Some(v) = dec::parse(raw) else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност"));
    };
    if v <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "сумата за разпределяне трябва да е по-голяма от 0"));
    }
    Ok(dec::parse(&money(v)).unwrap_or(v))
}

fn rate_of(raw: &str) -> Result<String, ApiError> {
    let Some(v) = dec::parse(raw) else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност"));
    };
    if v < 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "данъчната ставка не може да е отрицателна"));
    }
    Ok(dec::text(v, 2))
}

fn pick_str(obj: &Map<String, Value>, cur: &Value, key: &str) -> Result<String, ApiError> {
    if let Some(v) = form::str_field(obj, key)? {
        return Ok(v);
    }
    Ok(cur.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string())
}

fn pick_i32(obj: &Map<String, Value>, cur: &Value, key: &str) -> Result<i32, ApiError> {
    if let Some(v) = form::i32_field(obj, key)? {
        return Ok(v);
    }
    Ok(cur.get(key).and_then(|v| v.as_i64()).unwrap_or(0) as i32)
}

async fn company(st: &AppState, headers: &HeaderMap) -> Result<i32, ApiError> {
    let sub = auth_sub(st, headers)?;
    let id = active_or_one(st, &sub).await?;
    if id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "няма избрана активна фирма"));
    }
    Ok(id)
}
