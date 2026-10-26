//! Проверка на интегритета на базата от „Глобални настройки“.
//! Octo не поддържа LIMIT, затова редовете се четат и броят в паметта.
//! Проверява връзката, основните таблици, сираци по фирма/документ и
//! баланса на счетоводните статии (дебит = кредит).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str};
use crate::form::auth_sub;
use crate::{AppState, ApiError};

const TABLES: &[&str] = &[
    "companies",
    "accounts",
    "counterparts",
    "products",
    "invoices",
    "invoice_lines",
    "journal_entries",
    "journal_lines",
    "users",
    "roles",
    "user_companies",
    "bank_accounts",
    "bank_transactions",
    "fixed_assets",
    "material_issues",
    "internal_docs",
    "production_orders",
    "recipes",
    "dividends",
    "opening_balances",
    "accounting_periods",
    "audit_logs",
    "document_series",
    "settings",
];

fn check(id: &str, ok: bool, detail: impl Into<String>) -> Value {
    json!({ "id": id, "ok": ok, "detail": detail.into() })
}

async fn ids(st: &AppState, sql: &str) -> HashSet<i32> {
    match st.db.query(sql).await {
        Ok(rows) => rows.iter().map(|r| cell_i32_at(r, 0)).collect(),
        Err(_) => HashSet::new(),
    }
}

/// Сираци по фирма: ред с company_id, който не е сред фирмите.
async fn orphans(st: &AppState, id: &str, sql: &str, companies: &HashSet<i32>) -> Value {
    match st.db.query(sql).await {
        Ok(rows) => {
            let bad = rows
                .iter()
                .filter(|r| !companies.contains(&cell_i32_at(r, 1)))
                .count();
            check(id, bad == 0, bad.to_string())
        }
        Err(e) => check(id, false, e.to_string()),
    }
}

/// Висящи редове: стойност в първата колона, която не е сред родителите.
async fn dangling(st: &AppState, id: &str, sql: &str, parents: &HashSet<i32>) -> Value {
    match st.db.query(sql).await {
        Ok(rows) => {
            let bad = rows
                .iter()
                .filter(|r| !parents.contains(&cell_i32_at(r, 0)))
                .count();
            check(id, bad == 0, bad.to_string())
        }
        Err(e) => check(id, false, e.to_string()),
    }
}

pub async fn integrity(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let started = Instant::now();
    let mut checks: Vec<Value> = Vec::new();
    let mut tables: Vec<Value> = Vec::new();

    let mut ok_tables = 0;
    let mut table_errors: Vec<String> = Vec::new();
    for name in TABLES {
        match st.db.query(&format!("SELECT id FROM {name}")).await {
            Ok(rows) => {
                ok_tables += 1;
                tables.push(json!({ "name": name, "rows": rows.len() }));
            }
            Err(e) => table_errors.push(format!("{name}: {e}")),
        }
    }
    let tables_ok = table_errors.is_empty();
    checks.push(check("connection", true, String::new()));
    checks.push(check(
        "tables",
        tables_ok,
        if tables_ok {
            format!("{ok_tables}/{}", TABLES.len())
        } else {
            table_errors.join("; ")
        },
    ));

    let companies = ids(&st, "SELECT id FROM companies").await;
    checks.push(orphans(&st, "orphan_invoices", "SELECT id, company_id FROM invoices", &companies).await);
    checks.push(
        orphans(&st, "orphan_journal_entries", "SELECT id, company_id FROM journal_entries", &companies)
            .await,
    );
    checks.push(orphans(&st, "orphan_accounts", "SELECT id, company_id FROM accounts", &companies).await);
    checks.push(orphans(&st, "orphan_products", "SELECT id, company_id FROM products", &companies).await);

    let invoices = ids(&st, "SELECT id FROM invoices").await;
    checks.push(dangling(&st, "orphan_invoice_lines", "SELECT invoice_id FROM invoice_lines", &invoices).await);
    let entries = ids(&st, "SELECT id FROM journal_entries").await;
    checks.push(dangling(&st, "orphan_journal_lines", "SELECT entry_id FROM journal_lines", &entries).await);

    let balance = match st.db.query("SELECT entry_id, direction, amount FROM journal_lines").await {
        Ok(rows) => {
            let mut debit: HashMap<i32, i128> = HashMap::new();
            let mut credit: HashMap<i32, i128> = HashMap::new();
            for r in &rows {
                let entry = cell_i32_at(r, 0);
                let is_debit = cell_str(r, 1).trim().to_ascii_lowercase().starts_with('d');
                let amount = crate::dec::or0(&cell_str(r, 2));
                let side = if is_debit { &mut debit } else { &mut credit };
                *side.entry(entry).or_insert(0) += amount;
            }
            let mut keys: HashSet<i32> = debit.keys().copied().collect();
            keys.extend(credit.keys().copied());
            let bad = keys
                .iter()
                .filter(|e| debit.get(e).copied().unwrap_or(0) != credit.get(e).copied().unwrap_or(0))
                .count();
            check("journal_balance", bad == 0, bad.to_string())
        }
        Err(e) => check("journal_balance", false, e.to_string()),
    };
    checks.push(balance);

    let ok = checks.iter().all(|c| c["ok"].as_bool().unwrap_or(false));
    Ok(Json(json!({
        "ok": ok,
        "checked_at": chrono::Utc::now().to_rfc3339(),
        "duration_ms": started.elapsed().as_millis() as u64,
        "checks": checks,
        "tables": tables,
    })))
}
