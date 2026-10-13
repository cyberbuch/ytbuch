//! SAF-T номенклатури на НАП и зареждане на сметкоплана от тях.
//! Каталогът е същият като в bagabuch (Structure Definition V1.0.1).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::db::{self, lit};
use crate::form;
use crate::{api_err, auth, AppState, ApiError};

const CATALOG_JSON: &str = include_str!("../data/saft_noms.json");

const KINDS: &[(&str, &str)] = &[
    ("stock_movements", "Движения запаси"),
    ("asset_movements", "Движения активи"),
    ("tax_regimes", "Данъчни режими"),
    ("payment_methods", "Начини на плащане"),
    ("invoice_types", "Видове фактури"),
    ("tax_types", "Видове данъци"),
    ("tax_codes", "Данъчни кодове"),
    ("product_types", "Видове запаси"),
    ("units", "Мерни единици"),
    ("regions", "Области"),
    ("accounts", "Сметки НАП"),
];

pub(crate) fn catalog() -> &'static HashMap<String, Vec<String>> {
    static CATALOG: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(CATALOG_JSON).expect("saft_noms.json"))
}

fn fields(kind: &str) -> &'static [&'static str] {
    match kind {
        "payment_methods" => &["method_code", "mechanism_code", "name", "name_en"],
        "tax_codes" => &["tax_type", "code", "description", "rate", "note"],
        "accounts" => &[
            "code",
            "name",
            "section",
            "section_name",
            "group",
            "group_name",
        ],
        "units" => &["code", "name", "name_en", "symbol", "category", "source"],
        "regions" | "tax_types" => &["code", "name"],
        _ => &["code", "name", "name_en"],
    }
}

fn parse_line(kind: &str, line: &str) -> Value {
    let parts: Vec<&str> = line.split('|').collect();
    let mut obj = Map::new();
    for (i, key) in fields(kind).iter().enumerate() {
        obj.insert((*key).to_string(), json!(parts.get(i).copied().unwrap_or("")));
    }
    Value::Object(obj)
}

#[derive(Deserialize)]
pub struct NomQuery {
    kind: Option<String>,
    search: Option<String>,
}

pub async fn nomenclatures(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<NomQuery>,
) -> Result<Json<Value>, ApiError> {
    auth::subject(&st.jwt_secret, &headers)?;
    let kind = q.kind.unwrap_or_default().trim().to_string();
    if kind.is_empty() {
        let kinds: Vec<Value> = KINDS
            .iter()
            .map(|(id, name)| json!({ "id": id, "name": name }))
            .collect();
        return Ok(Json(json!({ "kinds": kinds, "count": kinds.len() })));
    }
    let Some(rows) = catalog().get(&kind) else {
        return Err(api_err(StatusCode::BAD_REQUEST, "непозната номенклатура (kind)"));
    };
    let needle = q.search.unwrap_or_default().trim().to_lowercase();
    let mut items = Vec::new();
    for line in rows {
        if !needle.is_empty() && !line.to_lowercase().contains(&needle) {
            continue;
        }
        items.push(parse_line(&kind, line));
    }
    let count = items.len();
    Ok(Json(json!({ "kind": kind, "items": items, "count": count })))
}

/// Липсващите кодове от NRA_Nom_Accounts влизат в сметкоплана.
/// Съществуващите номера се пропускат.
pub async fn fill_chart(db: &db::Db, company: i32) -> Result<(i32, i32), tokio_postgres::Error> {
    let existing = db
        .query(&format!("SELECT number FROM accounts WHERE company_id = {company}"))
        .await?;
    let mut have: HashSet<String> = existing.iter().map(|r| db::cell_str(r, 0)).collect();
    let mut id = db.next_id("accounts").await?;
    let mut created = 0;
    let mut skipped = 0;
    let mut batch: Vec<String> = Vec::new();
    for line in catalog().get("accounts").map(|v| v.as_slice()).unwrap_or(&[]) {
        let mut parts = line.split('|');
        let code = parts.next().unwrap_or("").trim();
        let name = parts.next().unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }
        if have.contains(code) {
            skipped += 1;
            continue;
        }
        batch.push(format!(
            "({id}, {company}, {}, {}, {}, 'none')",
            lit(code),
            lit(name),
            lit(code)
        ));
        have.insert(code.to_string());
        id += 1;
        created += 1;
        if batch.len() == 40 {
            let sql = format!(
                "INSERT INTO accounts (id, company_id, number, name, saft_account_type, analytic_type) VALUES {}",
                batch.join(", ")
            );
            batch.clear();
            db.exec(&sql).await?;
        }
    }
    if !batch.is_empty() {
        let sql = format!(
            "INSERT INTO accounts (id, company_id, number, name, saft_account_type, analytic_type) VALUES {}",
            batch.join(", ")
        );
        db.exec(&sql).await?;
    }
    Ok((created, skipped))
}

pub async fn ensure_chart(db: &db::Db) -> Result<(), tokio_postgres::Error> {
    let companies = db.query("SELECT id FROM companies ORDER BY id").await?;
    for row in &companies {
        fill_chart(db, db::cell_i32(row)).await?;
    }
    let marked = db
        .query("SELECT id FROM schema_migrations WHERE id = 3")
        .await?;
    if marked.is_empty() {
        db.exec("INSERT INTO schema_migrations (id, name) VALUES (3, 'saft_accounts')")
            .await?;
    }
    Ok(())
}

/// Празен низ минава. BG код или име от каталога се нормализира.
/// Чужда държава пази свободния текст до 35 знака.
pub fn region_code(raw: &str, country: &str) -> Result<String, ()> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(String::new());
    }
    let key = region_key(t);
    if let Some(rows) = catalog().get("regions") {
        for line in rows {
            let mut parts = line.splitn(2, '|');
            let code = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            if key == code || t == name {
                return Ok(code.to_string());
            }
        }
    }
    if key.starts_with("BG-") {
        return Err(());
    }
    let cc = country.trim().to_ascii_uppercase();
    if cc.is_empty() || cc == "BG" {
        return Err(());
    }
    Ok(t.chars().take(35).collect())
}

fn region_key(raw: &str) -> String {
    let up = raw.trim().to_ascii_uppercase();
    let b = up.as_bytes();
    if b.len() == 4
        && b[0] == b'B'
        && b[1] == b'G'
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
    {
        return format!("BG-{}", &up[2..]);
    }
    up
}

pub async fn units(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<NomQuery>,
) -> Result<Json<Value>, ApiError> {
    auth::subject(&st.jwt_secret, &headers)?;
    let needle = q.search.unwrap_or_default().trim().to_lowercase();
    let mut items = Vec::new();
    for line in catalog().get("units").map(|v| v.as_slice()).unwrap_or(&[]) {
        if !needle.is_empty() && !line.to_lowercase().contains(&needle) {
            continue;
        }
        items.push(parse_line("units", line));
    }
    let count = items.len();
    Ok(Json(json!({ "items": items, "count": count, "limit": 50, "offset": 0 })))
}

pub async fn seed_saft(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let sub = form::auth_sub(&st, &headers)?;
    let company = form::active_or_one(&st, &sub).await?;
    let (created, skipped) = fill_chart(&st.db, company)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({
        "created": created,
        "skipped": skipped,
        "total": created + skipped,
    })))
}
