//! Правила за банкови транзакции: ключови думи → сметка (и контрагент).
//! Ключовите думи се търсят в описанието, името на контрагента и референцията,
//! без AI. Текстът се нормализира (кирилица → латиница), за да съвпадат
//! „ПОС“/„POS“ и „теглене“/„withdrawal“. При няколко съвпадения печели
//! правилото с по-висок приоритет.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::bank_tx;
use crate::db::{cell_i32_at, cell_str, lit};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoices::err_text;
use crate::invoice_read;
use crate::{api_err, AppState, ApiError};

const CYR: [(&str, &str); 33] = [
    ("а", "a"), ("б", "b"), ("в", "v"), ("г", "g"), ("д", "d"), ("е", "e"),
    ("ж", "zh"), ("з", "z"), ("и", "i"), ("й", "y"), ("к", "k"), ("л", "l"),
    ("м", "m"), ("н", "n"), ("о", "o"), ("п", "p"), ("р", "r"), ("с", "s"),
    ("т", "t"), ("у", "u"), ("ф", "f"), ("х", "h"), ("ц", "ts"), ("ч", "ch"),
    ("ш", "sh"), ("щ", "sht"), ("ъ", "a"), ("ь", ""), ("ю", "yu"), ("я", "ya"),
    ("э", "e"), ("ы", "y"), ("ё", "e"),
];

const LEGAL_FORMS: [&str; 10] = [
    "eood", "ood", "ad", "et", "kd", "kda", "ltd", "llc", "gmbh", "inc",
];

/// Прибира кирилица и латиница до сравним ASCII ключ: малки букви,
/// кирилицата се транслитерира, всичко останало става интервал.
pub fn fold(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    for ch in lower.chars() {
        let c = ch.to_string();
        if let Some((_, lat)) = CYR.iter().find(|(cyr, _)| *cyr == c) {
            out.push_str(lat);
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn company_key(text: &str) -> String {
    let mut s = fold(text);
    for form in LEGAL_FORMS {
        s = s
            .split_whitespace()
            .filter(|w| *w != form)
            .collect::<Vec<_>>()
            .join(" ");
    }
    s
}

fn contains_keyword(haystack: &str, keyword: &str) -> bool {
    let needle = fold(keyword);
    !needle.is_empty() && fold(haystack).contains(&needle)
}

fn matches_keywords(text: &str, keywords: &str) -> bool {
    keywords.split(',').any(|raw| {
        let kw = raw.trim();
        !kw.is_empty() && contains_keyword(text, kw)
    })
}

fn direction_matches(direction: &str, amount: &str) -> bool {
    let v = amount.parse::<f64>().unwrap_or(0.0);
    match direction {
        "debit" => v < 0.0,
        "credit" => v > 0.0,
        _ => true,
    }
}

struct Rule {
    id: i32,
    name: String,
    keywords: String,
    direction: String,
    priority: i32,
    is_active: i32,
    account_id: i32,
    counterpart_id: i32,
}

fn rule_from(r: &crate::db::Row) -> Rule {
    Rule {
        id: cell_i32_at(r, 0),
        name: cell_str(r, 2),
        keywords: cell_str(r, 3),
        direction: cell_str(r, 4),
        priority: cell_i32_at(r, 5),
        is_active: cell_i32_at(r, 6),
        account_id: cell_i32_at(r, 7),
        counterpart_id: cell_i32_at(r, 8),
    }
}

const RULE_SEL: &str = "SELECT id, company_id, name, keywords, direction, priority, is_active, account_id, counterpart_id FROM bank_matching_rules";

fn rule_json(r: &Rule) -> Value {
    json!({
        "id": r.id,
        "name": r.name,
        "keywords": r.keywords,
        "direction": r.direction,
        "priority": r.priority,
        "is_active": r.is_active == 1,
        "account_id": r.account_id,
        "counterpart_id": r.counterpart_id,
    })
}

async fn rules_of(db: &crate::db::Db, company: i32) -> Result<Vec<Rule>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("{RULE_SEL} WHERE company_id = {company} ORDER BY priority DESC, id"),
    )
    .await?;
    Ok(rows.iter().map(rule_from).collect())
}

/// Сред съвпадащите правила печели това с най-висок приоритет,
/// независимо от реда, в който са подадени.
fn find_rule<'a>(rules: &'a [Rule], text: &str, amount: &str) -> Option<&'a Rule> {
    if text.trim().is_empty() {
        return None;
    }
    rules
        .iter()
        .filter(|r| r.is_active == 1)
        .filter(|r| direction_matches(&r.direction, amount) && matches_keywords(text, &r.keywords))
        .max_by_key(|r| r.priority)
}

fn find_counterpart_id(db_rows: &[(i32, String)], contra_name: &str) -> Option<i32> {
    let key = company_key(contra_name);
    if key.chars().count() < 3 {
        return None;
    }
    for (id, name) in db_rows {
        let cp_key = company_key(name);
        if cp_key.is_empty() {
            continue;
        }
        if cp_key == key {
            return Some(*id);
        }
        if cp_key.chars().count() >= 5 && key.chars().count() >= 5
            && (key.contains(&cp_key) || cp_key.contains(&key))
        {
            return Some(*id);
        }
    }
    None
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let rules = rules_of(&st.db, company).await?;
    Ok(Json(json!({ "items": rules.iter().map(rule_json).collect::<Vec<_>>() })))
}

fn parse_rule_body(obj: &serde_json::Map<String, Value>) -> Result<(String, String, String, i32, i32, i32, i32), ApiError> {
    let name = form::required(form::str_field(obj, "name")?, "name")?.trim().to_string();
    let keywords = form::required(form::str_field(obj, "keywords")?, "keywords")?.trim().to_string();
    let direction = form::str_field(obj, "direction")?.unwrap_or_else(|| "any".into()).trim().to_string();
    if !matches!(direction.as_str(), "any" | "debit" | "credit") {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "direction е any, debit или credit"));
    }
    let priority = form::i32_field(obj, "priority")?.unwrap_or(0);
    let is_active = obj.get("is_active").and_then(|v| v.as_bool()).map(|b| i32::from(b)).unwrap_or(1);
    let account_id = form::i32_field(obj, "account_id")?.unwrap_or(0);
    if account_id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "account_id е задължителен"));
    }
    let counterpart_id = form::i32_field(obj, "counterpart_id")?.unwrap_or(0);
    Ok((name, keywords, direction, priority, is_active, account_id, counterpart_id))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let (name, keywords, direction, priority, is_active, account_id, counterpart_id) =
        parse_rule_body(&obj)?;
    form::ensure_ref(&st.db, "accounts", account_id, company, "сметката").await?;
    if counterpart_id > 0 {
        form::ensure_ref(&st.db, "counterparts", counterpart_id, company, "контрагентът").await?;
    }
    let id = st.db.next_id("bank_matching_rules").await.map_err(crate::form::db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO bank_matching_rules (id, company_id, name, keywords, direction, priority, is_active, account_id, counterpart_id) \
             VALUES ({id}, {company}, {}, {}, {}, {priority}, {is_active}, {account_id}, {counterpart_id})",
            lit(&name), lit(&keywords), lit(&direction),
        ))
        .await
        .map_err(crate::form::db_err)?;
    let rules = rules_of(&st.db, company).await?;
    let made = rules.into_iter().find(|r| r.id == id).map(|r| rule_json(&r)).unwrap_or(json!({}));
    Ok((StatusCode::CREATED, Json(made)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let (name, keywords, direction, priority, is_active, account_id, counterpart_id) =
        parse_rule_body(&obj)?;
    form::ensure_ref(&st.db, "accounts", account_id, company, "сметката").await?;
    if counterpart_id > 0 {
        form::ensure_ref(&st.db, "counterparts", counterpart_id, company, "контрагентът").await?;
    }
    let n = st
        .db
        .exec(&format!(
            "UPDATE bank_matching_rules SET name = {}, keywords = {}, direction = {}, priority = {priority}, is_active = {is_active}, account_id = {account_id}, counterpart_id = {counterpart_id} \
             WHERE id = {id} AND company_id = {company}",
            lit(&name), lit(&keywords), lit(&direction),
        ))
        .await
        .map_err(crate::form::db_err)?;
    if n == 0 {
        return Err(api_err(StatusCode::NOT_FOUND, "правилото не е намерено"));
    }
    let rules = rules_of(&st.db, company).await?;
    Ok(Json(rules.into_iter().find(|r| r.id == id).map(|r| rule_json(&r)).unwrap_or(json!({}))))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    st.db
        .exec(&format!("DELETE FROM bank_matching_rules WHERE id = {id} AND company_id = {company}"))
        .await
        .map_err(crate::form::db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Шаблони по подразбиране (като baraba): име, ключови думи, посока,
/// сметки-кандидати (първата съществуваща), приоритет.
const DEFAULTS: [(&str, &str, &str, &[&str], i32); 6] = [
    ("POS покупки", "POS,ПОС,pos трансакция,покупка на пос,pos purchase,card purchase,покупка с карта,покупка с дк", "debit", &["447", "446", "50902"], 100),
    ("ATM теглене", "ATM,АТМ,atm трансакция,теглене,withdrawal,cash withdrawal,теглене на каса,теглене на пари", "debit", &["501"], 90),
    ("Банкови такси", "автоматична такса,такса теглене,такса за поддръжка,комисионна,bank fee,commission,месечна такса,мес. такса,monthly fee", "debit", &["629"], 80),
    ("Получени преводи от клиенти", "получен директен превод,получен превод,sct instant получен,incoming credit,credit transfer received", "credit", &["411"], 70),
    ("Издадени преводи към доставчици", "издаден превод,sct instant издаден,изходящ превод,outgoing credit,issued transfer", "debit", &["401"], 60),
    ("Аванси", "аванс,avans,advance", "debit", &["402"], 50),
];

async fn account_id_by_number(db: &crate::db::Db, company: i32, number: &str) -> Option<i32> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT id FROM accounts WHERE company_id = {company} AND number = {}", lit(number)),
    )
    .await
    .ok()?;
    rows.first().map(|r| cell_i32_at(r, 0))
}

pub async fn defaults(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let existing: Vec<String> = rules_of(&st.db, company).await?.into_iter().map(|r| r.name).collect();
    let mut created = 0;
    for (name, keywords, direction, codes, priority) in DEFAULTS {
        if existing.iter().any(|n| n == name) {
            continue;
        }
        let mut account_id = 0;
        for code in codes {
            if let Some(id) = account_id_by_number(&st.db, company, code).await {
                account_id = id;
                break;
            }
        }
        if account_id <= 0 {
            continue;
        }
        let id = st.db.next_id("bank_matching_rules").await.map_err(crate::form::db_err)?;
        st.db
            .exec(&format!(
                "INSERT INTO bank_matching_rules (id, company_id, name, keywords, direction, priority, is_active, account_id, counterpart_id) \
                 VALUES ({id}, {company}, {}, {}, {}, {priority}, 1, {account_id}, 0)",
                lit(name), lit(keywords), lit(direction),
            ))
            .await
            .map_err(crate::form::db_err)?;
        created += 1;
    }
    Ok(Json(json!({ "created": created })))
}

/// Преглед преди осчетоводяване: показва кои транзакции към кое правило/сметка
/// биха се разнесли, без да записва нищо.
pub async fn preview(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    Ok(Json(allocate(&st, company, &sub, false).await?))
}

/// Автоматично разнасяне: за всяка неразнесена транзакция на фирмата намира
/// правило и я осчетоводява (или пренасочва буферната) към сметката му.
pub async fn auto_allocate(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    Ok(Json(allocate(&st, company, &sub, true).await?))
}

async fn allocate(st: &AppState, company: i32, sub: &str, apply: bool) -> Result<Value, ApiError> {
    let rules = rules_of(&st.db, company).await?;
    let cp_rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id, name FROM counterparts WHERE company_id = {company}"),
    )
    .await?;
    let counterparts: Vec<(i32, String)> = cp_rows.iter().map(|r| (cell_i32_at(r, 0), cell_str(r, 1))).collect();
    let acc_rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id, number, name FROM accounts WHERE company_id = {company}"),
    )
    .await?;
    let account_label = |id: i32| -> String {
        acc_rows
            .iter()
            .find(|r| cell_i32_at(r, 0) == id)
            .map(|r| format!("{} {}", cell_str(r, 1), cell_str(r, 2)))
            .unwrap_or_else(|| format!("#{id}"))
    };

    let tx_rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, amount, description, counterpart_name, reference, is_booked FROM bank_transactions \
             WHERE company_id = {company} AND is_allocated = 0 ORDER BY id"
        ),
    )
    .await?;

    let mut matched = 0;
    let mut unmatched = 0;
    let mut errors = 0;
    let mut counterpart_hits = 0;
    let mut details: Vec<Value> = Vec::new();

    for row in &tx_rows {
        let id = cell_i32_at(row, 0);
        let amount = cell_str(row, 1);
        let text = format!("{} {} {}", cell_str(row, 2), cell_str(row, 3), cell_str(row, 4));
        let Some(rule) = find_rule(&rules, &text, &amount) else {
            unmatched += 1;
            if details.len() < 50 {
                details.push(json!({ "id": id, "status": "unmatched", "text": text.chars().take(80).collect::<String>() }));
            }
            continue;
        };
        let cp = if rule.counterpart_id > 0 {
            Some(rule.counterpart_id)
        } else {
            find_counterpart_id(&counterparts, &cell_str(row, 3))
        };
        if cp.is_some() {
            counterpart_hits += 1;
        }
        let cp_id = cp.unwrap_or(0);
        let cp_name = counterparts.iter().find(|(i, _)| *i == cp_id).map(|(_, n)| n.clone()).unwrap_or_default();

        if !apply {
            matched += 1;
            if details.len() < 50 {
                details.push(json!({
                    "id": id, "status": "matched", "rule": rule.name,
                    "account": account_label(rule.account_id),
                    "counterpart": cp_name, "amount": amount,
                    "text": text.chars().take(80).collect::<String>(),
                }));
            }
            continue;
        }

        let res = if cell_i32_at(row, 5) == 0 {
            bank_tx::book_tx(st, company, sub, id, rule.account_id, cp_id, None, None).await.map(|_| ())
        } else {
            bank_tx::reallocate_tx(st, company, sub, id, rule.account_id, cp_id).await.map(|_| ())
        };
        match res {
            Ok(()) => {
                matched += 1;
                if details.len() < 50 {
                    details.push(json!({ "id": id, "status": "matched", "rule": rule.name, "account": account_label(rule.account_id) }));
                }
            }
            Err(e) => {
                errors += 1;
                if details.len() < 50 {
                    details.push(json!({ "id": id, "status": "error", "error": err_text(&e) }));
                }
            }
        }
    }

    Ok(json!({
        "matched": matched,
        "unmatched": unmatched,
        "errors": errors,
        "counterpart_hits": counterpart_hits,
        "details": details,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_transliterates_cyrillic() {
        assert_eq!(fold("ПОС трансакция"), "pos transaktsiya");
        assert_eq!(fold("POS purchase"), "pos purchase");
        assert_eq!(fold("Теглене на каса"), "teglene na kasa");
    }

    #[test]
    fn keyword_matches_across_scripts() {
        assert!(contains_keyword("ПОС трансакция 123", "pos"));
        assert!(contains_keyword("POS PURCHASE CARD", "ПОС"));
        assert!(contains_keyword("теглене от АТМ", "withdrawal") == false);
        assert!(contains_keyword("cash withdrawal", "теглене") == false);
        assert!(contains_keyword("НЕВЕЛ 25 ЕООД", "Nevel 25 EOOD"));
    }

    #[test]
    fn higher_priority_wins() {
        let rules = vec![
            Rule { id: 1, name: "нисък".into(), keywords: "такса".into(), direction: "any".into(), priority: 10, is_active: 1, account_id: 1, counterpart_id: 0 },
            Rule { id: 2, name: "висок".into(), keywords: "такса,fee".into(), direction: "debit".into(), priority: 90, is_active: 1, account_id: 2, counterpart_id: 0 },
        ];
        let hit = find_rule(&rules, "месечна такса поддръжка", "-12.00").unwrap();
        assert_eq!(hit.id, 2);
    }

    #[test]
    fn direction_filters() {
        assert!(direction_matches("debit", "-5"));
        assert!(!direction_matches("debit", "5"));
        assert!(direction_matches("credit", "5"));
        assert!(direction_matches("any", "5"));
    }

    #[test]
    fn company_key_ignores_legal_form() {
        assert_eq!(company_key("НЕВЕЛ 25 ЕООД"), company_key("Nevel 25"));
    }
}
