//! Изписване на продадени стоки и продукция за месец или към дата.
//! Повторно пускане за същия месец подменя записа, не го удвоява.

use std::collections::{HashMap, HashSet};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, money};
use crate::form::{active_or_one, auth_sub};
use crate::invoice_read;
use crate::periods;
use crate::posting::{self, Side};
use crate::stock_replay::{self, SaleHit};
use crate::stock_rules::{self, Rule};
use crate::{api_err, AppState, ApiError};

#[derive(Deserialize, Default)]
pub struct Body {
    month: Option<String>,
    until: Option<String>,
}

pub async fn post(State(st): State<AppState>, headers: HeaderMap, Json(body): Json<Body>) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let res = run(&st.db, company, body.month.as_deref(), body.until.as_deref()).await?;
    let amt = res.get("amount").and_then(|v| v.as_str()).unwrap_or("0.00");
    if let Some(entries) = res.get("entries").and_then(|v| v.as_array()) {
        for eid in entries {
            let entry_id = eid.as_i64().unwrap_or(0) as i32;
            if entry_id > 0 {
                let m = body.month.as_deref().or(body.until.as_deref()).unwrap_or("");
                let summary = format!("Осчетоводено изписване на стоки/продукция ({m}), сума {amt}");
                crate::audit::record(
                    &st.db,
                    company,
                    &sub,
                    "journal",
                    entry_id,
                    m,
                    "create",
                    &summary,
                    "",
                    amt,
                    None,
                    Some(&res),
                )
                .await;
            }
        }
    }
    Ok(Json(res))
}

pub async fn run(db: &crate::db::Db, company: i32, month: Option<&str>, until: Option<&str>) -> Result<Value, ApiError> {
    let rules = stock_rules::load(db, company).await?;
    let months = months(db, company, month, until).await?;
    for m in &months {
        periods::assert_open(db, company, &format!("{m}-01")).await?;
    }
    let built = stock_replay::rebuild(db, company, "9999-12-31").await?;
    if rules.is_empty() && !built.sales.iter().any(|hit| hit.expense_account_id > 0) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "няма правила за склада — добавете ги в настройки, таб Склад",
        ));
    }
    let mut warnings = built.warnings;
    later_months(db, company, &months, &mut warnings).await?;
    let mut entries = Vec::new();
    let mut total = 0i128;
    for m in &months {
        let (id, sum) = one_month(db, company, m, &built.sales, &rules, &mut warnings).await?;
        total = dec::add(total, sum).unwrap_or(total);
        if id > 0 {
            entries.push(id);
        }
    }
    Ok(json!({
        "months": months,
        "entries": entries,
        "amount": money(total),
        "warnings": warnings,
    }))
}

async fn months(db: &crate::db::Db, company: i32, month: Option<&str>, until: Option<&str>) -> Result<Vec<String>, ApiError> {
    let month = month.unwrap_or("").trim().to_string();
    let until = until.unwrap_or("").trim().to_string();
    if !month.is_empty() && !until.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "подайте или месец, или дата"));
    }
    if ym_ok(&month) {
        return Ok(vec![month]);
    }
    if !iso(&until) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "месецът е YYYY-MM или датата е YYYY-MM-DD"));
    }
    let start_month = fy_month(db, company).await?;
    let sm = if (1..=12).contains(&start_month) { start_month } else { 1 };
    let y: i32 = until[0..4].parse().unwrap_or(2026);
    let m: i32 = until[5..7].parse().unwrap_or(1);
    let year = if m >= sm { y } else { y - 1 };
    let mut cy = year;
    let mut cm = sm;
    let mut out = Vec::new();
    while (cy, cm) <= (y, m) && out.len() < 24 {
        out.push(format!("{cy}-{cm:02}"));
        cm += 1;
        if cm == 13 {
            cm = 1;
            cy += 1;
        }
    }
    Ok(out)
}

async fn fy_month(db: &crate::db::Db, company: i32) -> Result<i32, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT fiscal_year_start_month FROM companies WHERE id = {company}"),
    )
    .await?;
    Ok(rows.first().map(|r| cell_i32_at(r, 0)).unwrap_or(1))
}

fn ym_ok(s: &str) -> bool {
    s.len() == 7 && s.as_bytes().get(4) == Some(&b'-') && s.chars().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
}

fn iso(s: &str) -> bool {
    s.len() == 10 && s.as_bytes().get(4) == Some(&b'-') && s.as_bytes().get(7) == Some(&b'-') && s.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

async fn later_months(db: &crate::db::Db, company: i32, months: &[String], warnings: &mut Vec<String>) -> Result<(), ApiError> {
    let last = months.last().cloned().unwrap_or_default();
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT document_number FROM journal_entries WHERE company_id = {company} AND document_type = 'stock_writeoff'"
        ),
    )
    .await?;
    let mut seen = HashSet::new();
    for row in &rows {
        let n = cell_str(row, 0);
        if n.as_str() > last.as_str() && seen.insert(n.clone()) {
            warnings.push(format!("има изписване за {n} след този период — пуснете и него"));
        }
    }
    Ok(())
}

async fn one_month(db: &crate::db::Db, company: i32, month: &str, sales: &[SaleHit], rules: &[Rule], warnings: &mut Vec<String>) -> Result<(i32, i128), ApiError> {
    let mut by: HashMap<i32, (String, i128, i32, i32, i32)> = HashMap::new();
    for hit in sales {
        if !hit.date.starts_with(month) || hit.cost == 0 {
            continue;
        }
        let e = by.entry(hit.product_id).or_insert((hit.product_name.clone(), 0, hit.inventory_account_id, hit.revenue_account_id, hit.expense_account_id));
        e.1 = dec::add(e.1, hit.cost).unwrap_or(e.1);
    }
    let mut sides = Vec::new();
    let mut total = 0i128;
    for (name, cost, inventory, revenue, expense) in by.values() {
        let (debit_id, credit_id) = if *expense > 0 {
            (*expense, *inventory)
        } else {
            let rule = stock_rules::pair(rules, "goods", *inventory, *revenue)
                .or_else(|| stock_rules::pair(rules, "production", *inventory, *revenue));
            let Some(rule) = rule else {
                warnings.push(format!("няма правило за {name}"));
                continue;
            };
            (rule.offset_account_id, rule.inventory_account_id)
        };
        let debit = posting::by_id(db, company, debit_id).await?;
        let credit = posting::by_id(db, company, credit_id).await?;
        let amount = money(*cost);
        sides.push(Side { account: debit, direction: "debit", amount: amount.clone() });
        sides.push(Side { account: credit, direction: "credit", amount });
        total = dec::add(total, *cost).unwrap_or(total);
    }
    let old = invoice_read::qry(
        db,
        &format!(
            "SELECT id FROM journal_entries WHERE company_id = {company} AND document_type = 'stock_writeoff' AND document_number = {}",
            lit(month)
        ),
    )
    .await?;
    for row in &old {
        invoice_read::wipe_journal(db, cell_i32_at(row, 0)).await?;
    }
    if sides.is_empty() {
        return Ok((0, 0));
    }
    let id_num: i32 = month.replace('-', "").parse().unwrap_or(0);
    let day = format!("{month}-01");
    let id = posting::write_entry(
        db,
        company,
        &day,
        "stock_writeoff",
        id_num,
        month,
        &format!("Изписване {month}"),
        0,
        "",
        month,
        &sides,
    )
    .await?;
    Ok((id, total))
}

pub async fn rebuild(State(st): State<AppState>, headers: HeaderMap, Json(body): Json<Value>) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let until = body.get("until").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let until = if until.is_empty() { "9999-12-31".to_string() } else { until };
    if until != "9999-12-31" && !iso(&until) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "датата е YYYY-MM-DD"));
    }
    let out = stock_replay::rebuild(&st.db, company, &until).await?;
    Ok(Json(json!({ "warnings": out.warnings, "balances": out.balances.len() })))
}
