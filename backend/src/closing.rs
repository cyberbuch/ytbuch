//! Приключвателни операции в края на месеца.
//!
//! Картата `closing_map` в настройките на фирмата (колоната `settings` на
//! `companies`) казва коя сметка/подсметка (префикс на номера) към коя сметка се
//! приключва: `[{"source": "60201", "target": "612"}, {"source": "60", "target": "611"}]`.
//! При съвпадение печели най-дългият префикс. Без карта по подразбиране
//! група 60 (разходи по икономически елементи) се приключва към 611.
//!
//! `POST /v1/closing-operations/preview` връща редовете без запис.
//! `POST /v1/closing-operations/run` записва една балансирана статия
//! (document_type = 'closing') на последния ден на месеца.

use std::collections::{HashMap, HashSet};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit, Db};
use crate::dec;
use crate::form::{self, auth_sub};
use crate::invoice_read;
use crate::periods;
use crate::posting::{self, Acct, Side};
use crate::{api_err, AppState, ApiError};

pub const CLOSING_TYPE: &str = "closing";

#[derive(Deserialize, Default)]
pub struct MonthQ {
    year: Option<String>,
    month: Option<String>,
}

#[derive(Clone)]
pub struct Rule {
    pub source: String,
    pub target: String,
}

pub struct Acc {
    pub id: i32,
    pub number: String,
    pub name: String,
}

/// Една приключвателна двойка: източник -> цел, с положителна сума.
pub struct Plan {
    pub source: Acc,
    pub target: Acc,
    pub balance: i128,
}

/// Правила от настройките: масив от обекти или обект {"60": "611"}.
/// Резултатът е сортиран по дължина на source низходящо (най-специфичното правило първо).
pub fn parse_rules(settings_raw: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    if let Ok(v) = serde_json::from_str::<Value>(settings_raw) {
        if let Some(arr) = v.get("closing_map").and_then(|m| m.as_array()) {
            for item in arr {
                let source = item.get("source").and_then(|s| s.as_str()).unwrap_or("").trim();
                let target = item.get("target").and_then(|s| s.as_str()).unwrap_or("").trim();
                if !source.is_empty() && !target.is_empty() {
                    rules.push(Rule { source: source.to_string(), target: target.to_string() });
                }
            }
        } else if let Some(obj) = v.get("closing_map").and_then(|m| m.as_object()) {
            for (source, target) in obj {
                let target = target.as_str().unwrap_or("").trim();
                if !source.trim().is_empty() && !target.is_empty() {
                    rules.push(Rule { source: source.trim().to_string(), target: target.to_string() });
                }
            }
        }
    }
    rules.sort_by_key(|r| std::cmp::Reverse(r.source.len()));
    rules
}

pub fn default_rules() -> Vec<Rule> {
    vec![Rule { source: "60".into(), target: "611".into() }]
}

/// Най-дългият префикс печели. Без съвпадение — None.
pub fn match_rule<'a>(rules: &'a [Rule], number: &str) -> Option<&'a Rule> {
    rules.iter().find(|r| number.starts_with(&r.source))
}

fn month_bounds(year: i32, month: i32) -> Result<(String, String, String), ApiError> {
    if !(1..=12).contains(&month) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "month трябва да е 1–12"));
    }
    let (ny, nm) = if month == 12 { (year + 1, 1u32) } else { (year, month as u32 + 1) };
    let next = chrono::NaiveDate::from_ymd_opt(ny, nm, 1)
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалиден месец"))?;
    let last = next - chrono::Duration::days(1);
    let first = chrono::NaiveDate::from_ymd_opt(year, month as u32, 1)
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалиден месец"))?;
    Ok((
        first.format("%Y-%m-%d").to_string(),
        next.format("%Y-%m-%d").to_string(),
        last.format("%Y-%m-%d").to_string(),
    ))
}

fn req_month(year: Option<i32>, month: Option<i32>) -> Result<(i32, i32), ApiError> {
    let y = year.ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "year е задължително"))?;
    let m = month.ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "month е задължително"))?;
    if y < 2000 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "year е невалидна"));
    }
    Ok((y, m))
}

/// Правилата от настройките; без ключ `closing_map` — по подразбиране 60 → 611.
pub async fn company_rules(db: &Db, company: i32) -> Result<Vec<Rule>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT settings FROM companies WHERE id = {company}"),
    )
    .await?;
    let raw = rows.first().map(|r| cell_str(r, 0)).unwrap_or_default();
    let has_key = serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|v| v.get("closing_map").cloned())
        .is_some();
    if has_key {
        Ok(parse_rules(&raw))
    } else {
        Ok(default_rules())
    }
}

async fn accounts_of(db: &Db, company: i32) -> Result<Vec<Acc>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT id, number, name FROM accounts WHERE company_id = {company} ORDER BY number, id"),
    )
    .await?;
    Ok(rows
        .iter()
        .map(|r| Acc {
            id: cell_i32_at(r, 0),
            number: cell_str(r, 1),
            name: cell_str(r, 2),
        })
        .collect())
}

/// Saldo (Дт-Кт) по сметка за всички записи до края на месеца включително.
async fn balances_until(db: &Db, company: i32, last_day: &str) -> Result<HashMap<i32, i128>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT l.account_id, l.direction, l.amount FROM journal_lines l \
             JOIN journal_entries e ON e.id = l.entry_id \
             WHERE e.company_id = {company} AND e.entry_date <= {} ORDER BY l.id",
            lit(last_day)
        ),
    )
    .await?;
    let mut map: HashMap<i32, i128> = HashMap::new();
    for row in &rows {
        let id = cell_i32_at(row, 0);
        let amount = dec::or0(&cell_str(row, 2));
        let slot = map.entry(id).or_insert(0);
        if cell_str(row, 1) == "debit" {
            *slot += amount;
        } else {
            *slot -= amount;
        }
    }
    Ok(map)
}

/// Строит приключвателните двойки от салдата. Сметките, които са цели на
/// правило, не се приключват сами (избягва се верига в една операция).
pub fn build_plans(
    accounts: &[Acc],
    balances: &HashMap<i32, i128>,
    rules: &[Rule],
) -> Result<Vec<Plan>, ApiError> {
    let targets: HashSet<&str> = rules.iter().map(|r| r.target.as_str()).collect();
    let by_number: HashMap<&str, &Acc> = accounts.iter().map(|a| (a.number.as_str(), a)).collect();
    for rule in rules {
        if !by_number.contains_key(rule.target.as_str()) {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("целева сметка {} от правило {} не съществува в сметкоплана", rule.target, rule.source),
            ));
        }
    }
    let mut plans = Vec::new();
    for acc in accounts {
        let balance = *balances.get(&acc.id).unwrap_or(&0);
        if balance == 0 {
            continue;
        }
        if targets.contains(acc.number.as_str()) {
            continue;
        }
        let Some(rule) = match_rule(rules, &acc.number) else {
            continue;
        };
        let Some(target) = by_number.get(rule.target.as_str()) else {
            continue;
        };
        plans.push(Plan {
            source: Acc { id: acc.id, number: acc.number.clone(), name: acc.name.clone() },
            target: Acc { id: target.id, number: target.number.clone(), name: target.name.clone() },
            balance,
        });
    }
    plans.sort_by(|a, b| a.source.number.cmp(&b.source.number));
    Ok(plans)
}

fn plan_lines(plans: &[Plan]) -> (Vec<Side>, i128) {
    let mut sides: Vec<Side> = Vec::new();
    // натрупваме знаковата сума по целева сметка: салдо Дт > 0 -> Дт на целта, Кт < 0 -> Кт на целта
    let mut totals: HashMap<i32, (Acct, i128)> = HashMap::new();
    let mut total = 0i128;
    for p in plans {
        total += dec::abs(p.balance);
        let src_acct = Acct { id: p.source.id, number: p.source.number.clone(), name: p.source.name.clone() };
        let dst_acct = Acct { id: p.target.id, number: p.target.number.clone(), name: p.target.name.clone() };
        if p.balance > 0 {
            sides.push(Side { account: src_acct, direction: "credit", amount: dec::money(dec::abs(p.balance)) });
        } else {
            sides.push(Side { account: src_acct, direction: "debit", amount: dec::money(dec::abs(p.balance)) });
        }
        let slot = totals.entry(p.target.id).or_insert((dst_acct, 0));
        slot.1 += p.balance;
    }
    let mut by_target: Vec<(i32, (Acct, i128))> = totals.into_iter().collect();
    by_target.sort_by_key(|(id, _)| *id);
    for (_, (acct, signed)) in by_target {
        if signed == 0 {
            continue;
        }
        let direction = if signed > 0 { "debit" } else { "credit" };
        sides.push(Side { account: acct, direction, amount: dec::money(dec::abs(signed)) });
    }
    (sides, total)
}

fn plans_json(plans: &[Plan], total: i128) -> Value {
    json!({
        "lines": plans.iter().map(|p| json!({
            "source_number": p.source.number,
            "source_name": p.source.name,
            "target_number": p.target.number,
            "target_name": p.target.name,
            "balance": dec::money(p.balance),
        })).collect::<Vec<_>>(),
        "total": dec::money(total),
        "count": plans.len(),
    })
}

async fn existing_closing(db: &Db, company: i32, year: i32, month: i32) -> Result<Option<(i32, String)>, ApiError> {
    let period = format!("{year}-{month:02}");
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT id, document_number FROM journal_entries WHERE company_id = {company} AND document_type = {} AND vat_period = {}",
            lit(CLOSING_TYPE),
            lit(&period)
        ),
    )
    .await?;
    Ok(rows.first().map(|r| (cell_i32_at(r, 0), cell_str(r, 1))))
}

async fn compute(
    db: &Db,
    company: i32,
    year: i32,
    month: i32,
) -> Result<(String, String, Vec<Plan>, i128), ApiError> {
    let (_, _, last_day) = month_bounds(year, month)?;
    let rules = company_rules(db, company).await?;
    let accounts = accounts_of(db, company).await?;
    let balances = balances_until(db, company, &last_day).await?;
    let plans = build_plans(&accounts, &balances, &rules)?;
    let (_, total) = plan_lines(&plans);
    Ok((last_day, format!("{year}-{month:02}"), plans, total))
}

pub async fn preview(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<MonthQ>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = form::active_or_one(&st, &sub).await?;
    let year = q.year.as_deref().unwrap_or("").trim().parse::<i32>().ok();
    let month = q.month.as_deref().unwrap_or("").trim().parse::<i32>().ok();
    let (year, month) = req_month(year, month)?;
    let (last_day, period, plans, total) = compute(&st.db, company, year, month).await?;
    let existing = existing_closing(&st.db, company, year, month).await?;
    let mut body = plans_json(&plans, total);
    body["year"] = json!(year);
    body["month"] = json!(month);
    body["entry_date"] = json!(last_day);
    body["period"] = json!(period);
    body["document_number"] = json!(format!("CLOSE-{period}"));
    body["already_done"] = json!(existing.is_some());
    if let Some((id, number)) = existing {
        body["existing_entry_id"] = json!(id);
        body["existing_document_number"] = json!(number);
    }
    Ok(Json(body))
}

pub async fn run(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = form::active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let (year, month) = req_month(form::i32_field(obj, "year")?, form::i32_field(obj, "month")?)?;
    let (last_day, period, plans, _total) = compute(&st.db, company, year, month).await?;

    periods::assert_open(&st.db, company, &last_day).await?;

    if let Some((id, number)) = existing_closing(&st.db, company, year, month).await? {
        return Err(api_err(
            StatusCode::CONFLICT,
            format!("приключването за {period} вече е записано ({number}, запис №{id})"),
        ));
    }
    if plans.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("няма салда за приключване за {period} по зададената карта"),
        ));
    }

    let (sides, total) = plan_lines(&plans);
    let doc_number = format!("CLOSE-{period}");
    let desc = format!("Приключвателни операции за {period}");
    let entry_id = posting::write_entry(
        &st.db,
        company,
        &last_day,
        CLOSING_TYPE,
        0,
        &doc_number,
        &desc,
        0,
        "",
        &period,
        &sides,
    )
    .await?;

    let mut out = plans_json(&plans, total);
    out["year"] = json!(year);
    out["month"] = json!(month);
    out["entry_date"] = json!(last_day);
    out["period"] = json!(period);
    out["document_number"] = json!(doc_number);
    out["entry_id"] = json!(entry_id);
    let summary = format!(
        "Приключвателни операции за {period}: {} сметки, оборот {}",
        plans.len(),
        dec::money(total)
    );
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "journal",
        entry_id,
        &doc_number,
        "create",
        &summary,
        "",
        &dec::money(total),
        None,
        Some(&out),
    )
    .await;
    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(id: i32, number: &str) -> Acc {
        Acc { id, number: number.into(), name: format!("acc {number}") }
    }

    #[test]
    fn rules_from_array_and_object() {
        let arr = r#"{"closing_map":[{"source":"60201","target":"612"},{"source":"60","target":"611"}]}"#;
        let rules = parse_rules(arr);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].source, "60201");
        assert_eq!(rules[1].source, "60");

        let obj = r#"{"closing_map":{"60":"611","602":"612"}}"#;
        let rules = parse_rules(obj);
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().any(|r| r.source == "60" && r.target == "611"));

        assert!(parse_rules("{}").is_empty());
        assert!(parse_rules("not json").is_empty());
        assert_eq!(default_rules()[0].target, "611");
    }

    #[test]
    fn longest_prefix_wins() {
        let rules = parse_rules(r#"{"closing_map":[{"source":"60","target":"611"},{"source":"602","target":"612"}]}"#);
        assert_eq!(match_rule(&rules, "60201").unwrap().target, "612");
        assert_eq!(match_rule(&rules, "60101").unwrap().target, "611");
        assert!(match_rule(&rules, "702").is_none());
    }

    #[test]
    fn plans_debit_and_credit_balances() {
        let accounts = vec![
            acc(1, "60101"),
            acc(2, "60201"),
            acc(3, "611"),
            acc(4, "612"),
            acc(5, "702"),
        ];
        let mut balances = HashMap::new();
        balances.insert(1, dec::parse("100.00").unwrap());
        balances.insert(2, dec::parse("-25.50").unwrap());
        balances.insert(3, dec::parse("10.00").unwrap());
        let rules = parse_rules(r#"{"closing_map":[{"source":"602","target":"612"},{"source":"60","target":"611"}]}"#);
        let plans = build_plans(&accounts, &balances, &rules).unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].source.number, "60101");
        assert_eq!(plans[0].target.number, "611");
        assert_eq!(plans[1].source.number, "60201");
        assert_eq!(plans[1].target.number, "612");

        let (sides, total) = plan_lines(&plans);
        // 60101: Кт 100.00; 60201: Дт 25.50; цели: Дт 611 100.00, Дт 612 25.50
        assert_eq!(sides.len(), 4);
        assert_eq!(sides[0].direction, "credit");
        assert_eq!(sides[0].amount, "100.00");
        assert_eq!(sides[1].direction, "debit");
        assert_eq!(sides[1].amount, "25.50");
        assert_eq!(total, dec::parse("125.50").unwrap());
        let td: i128 = sides.iter().filter(|s| s.direction == "debit").map(|s| dec::or0(&s.amount)).sum();
        let tc: i128 = sides.iter().filter(|s| s.direction == "credit").map(|s| dec::or0(&s.amount)).sum();
        assert_eq!(td, tc);
    }

    #[test]
    fn target_accounts_are_not_closed() {
        let accounts = vec![acc(1, "60"), acc(2, "611")];
        let mut balances = HashMap::new();
        balances.insert(1, dec::parse("10").unwrap());
        balances.insert(2, dec::parse("10").unwrap());
        let plans = build_plans(&accounts, &balances, &default_rules()).unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].source.number, "60");
    }

    #[test]
    fn missing_target_is_rejected() {
        let accounts = vec![acc(1, "60101")];
        let mut balances = HashMap::new();
        balances.insert(1, dec::parse("5").unwrap());
        let rules = vec![Rule { source: "60".into(), target: "999".into() }];
        assert!(build_plans(&accounts, &balances, &rules).is_err());
    }

    #[test]
    fn month_bounds_ok() {
        let (first, next, last) = month_bounds(2026, 12).unwrap();
        assert_eq!(first, "2026-12-01");
        assert_eq!(next, "2027-01-01");
        assert_eq!(last, "2026-12-31");
        let (_, _, last) = month_bounds(2026, 2).unwrap();
        assert_eq!(last, "2026-02-28");
        assert!(month_bounds(2026, 13).is_err());
    }

    /// Интеграционен тест срещу Octo: отделна тестова фирма, без да се пипа демото.
    /// Без база тестът се прескача.
    static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn test_db() -> Option<Db> {
        let url = std::env::var("YTBUCH_DATABASE_URL").unwrap_or_else(|_| {
            "host=127.0.0.1 port=1337 user=ydb password=ydbrocks dbname=ydb sslmode=disable".into()
        });
        match tokio::time::timeout(std::time::Duration::from_secs(5), Db::connect(&url, 2)).await {
            Ok(Ok(db)) => Some(db),
            _ => {
                eprintln!("SKIP: няма Octo за интеграционния тест");
                None
            }
        }
    }

    #[tokio::test]
    async fn closing_run_posts_balanced_entry_and_is_idempotent() {
        let _lock = DB_LOCK.lock().await;
        let Some(db) = test_db().await else { return };
        db.migrate().await.expect("migrate");

        // остатък от прекъснат тест
        invoice_read::exe(&db, "DELETE FROM journal_lines WHERE entry_id IN (SELECT id FROM journal_entries WHERE company_id IN (SELECT id FROM companies WHERE eik = '900000001'))").await.unwrap();
        invoice_read::exe(&db, "DELETE FROM journal_entries WHERE company_id IN (SELECT id FROM companies WHERE eik = '900000001')").await.unwrap();
        invoice_read::exe(&db, "DELETE FROM accounts WHERE company_id IN (SELECT id FROM companies WHERE eik = '900000001')").await.unwrap();
        invoice_read::exe(&db, "DELETE FROM audit_logs WHERE company_id IN (SELECT id FROM companies WHERE eik = '900000001')").await.unwrap();
        invoice_read::exe(&db, "DELETE FROM companies WHERE eik = '900000001'").await.unwrap();

        let company = invoice_read::nid(&db, "companies").await.unwrap();
        let settings = r#"{"closing_map":[{"source":"602","target":"612"},{"source":"60","target":"611"}]}"#;
        invoice_read::exe(
            &db,
            &format!(
                "INSERT INTO companies (id, name, eik, settings, created_at) VALUES ({company}, 'Closing Test', '900000001', {}, {})",
                lit(settings),
                lit(&crate::db::now_stamp())
            ),
        )
        .await
        .unwrap();
        crate::saft::fill_chart(&db, company).await.unwrap();

        async fn acc(db: &Db, company: i32, number: &str) -> i32 {
            let rows = invoice_read::qry(db, &format!("SELECT id FROM accounts WHERE company_id = {company} AND number = {}", lit(number))).await.unwrap();
            cell_i32_at(rows.first().unwrap(), 0)
        }
        let a60101 = acc(&db, company, "60101").await;
        let a60212 = acc(&db, company, "60212").await;
        let a401 = acc(&db, company, "401").await;

        let entry = invoice_read::nid(&db, "journal_entries").await.unwrap();
        invoice_read::exe(
            &db,
            &format!(
                "INSERT INTO journal_entries (id, company_id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period) VALUES ({entry}, {company}, '2031-05-10', 'manual', 0, 'TEST-CLOSE', 'тест', 'no_vat', 0, '', '2031-05-10', '2031-05')"
            ),
        )
        .await
        .unwrap();
        let mut line = invoice_read::nid(&db, "journal_lines").await.unwrap();
        for (account_id, direction, amount) in [
            (a60101, "debit", "100.00"),
            (a60212, "debit", "25.50"),
            (a401, "credit", "125.50"),
        ] {
            invoice_read::exe(
                &db,
                &format!(
                    "INSERT INTO journal_lines (id, entry_id, account_id, account_number, account_name, direction, amount, vat_amount) VALUES ({line}, {entry}, {account_id}, '', '', '{direction}', '{amount}', '0')"
                ),
            )
            .await
            .unwrap();
            line += 1;
        }

        // preview: 601 -> 611 (100.00), 60212 -> 612 (25.50)
        let (last_day, period, plans, total) = compute(&db, company, 2031, 5).await.unwrap();
        assert_eq!(last_day, "2031-05-31");
        assert_eq!(period, "2031-05");
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].source.number, "60101");
        assert_eq!(plans[0].target.number, "611");
        assert_eq!(plans[1].source.number, "60212");
        assert_eq!(plans[1].target.number, "612");
        assert_eq!(total, dec::parse("125.50").unwrap());
        assert!(existing_closing(&db, company, 2031, 5).await.unwrap().is_none());

        // run: балансирана статия
        let (sides, total) = plan_lines(&plans);
        let entry_id = posting::write_entry(
            &db,
            company,
            &last_day,
            CLOSING_TYPE,
            0,
            "CLOSE-2031-05",
            "Приключвателни операции за 2031-05",
            0,
            "",
            &period,
            &sides,
        )
        .await
        .unwrap();
        assert_eq!(dec::or0(&dec::money(total)), total);
        assert!(existing_closing(&db, company, 2031, 5).await.unwrap().is_some());

        // след приключването 60101 и 60212 са на нула, 611/612 са с оборота
        let balances = balances_until(&db, company, "2031-05-31").await.unwrap();
        assert_eq!(balances.get(&a60101), Some(&0));
        assert_eq!(balances.get(&a60212), Some(&0));
        let a611 = acc(&db, company, "611").await;
        let a612 = acc(&db, company, "612").await;
        assert_eq!(balances.get(&a611), Some(&dec::parse("100.00").unwrap()));
        assert_eq!(balances.get(&a612), Some(&dec::parse("25.50").unwrap()));

        // записът е балансиран
        let rows = invoice_read::qry(
            &db,
            &format!("SELECT direction, amount FROM journal_lines WHERE entry_id = {entry_id} ORDER BY id"),
        )
        .await
        .unwrap();
        let d: i128 = rows.iter().filter(|r| cell_str(r, 0) == "debit").map(|r| dec::or0(&cell_str(r, 1))).sum();
        let c: i128 = rows.iter().filter(|r| cell_str(r, 0) == "credit").map(|r| dec::or0(&cell_str(r, 1))).sum();
        assert_eq!(d, c);
        assert_eq!(d, dec::parse("125.50").unwrap());

        // почистване
        invoice_read::exe(&db, &format!("DELETE FROM journal_lines WHERE entry_id IN (SELECT id FROM journal_entries WHERE company_id = {company})")).await.unwrap();
        invoice_read::exe(&db, &format!("DELETE FROM journal_entries WHERE company_id = {company}")).await.unwrap();
        invoice_read::exe(&db, &format!("DELETE FROM accounts WHERE company_id = {company}")).await.unwrap();
        invoice_read::exe(&db, &format!("DELETE FROM audit_logs WHERE company_id = {company}")).await.unwrap();
        invoice_read::exe(&db, &format!("DELETE FROM companies WHERE id = {company}")).await.unwrap();
    }
}
