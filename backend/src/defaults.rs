//! Default сметки за автоматизация на фирмата (settings.default_accounts).
//! Двойки роля → номер на сметка; липсваща роля ползва стандартния номер от
//! сметкоплана. Постът и плащанията ги ползват за клиент/доставчик, ДДС, каса,
//! банка и буферни сметки за карта; приход/разход/запаси остават от артикула.

use crate::db::{cell_i32_at, cell_str, lit};
use crate::invoice_read;
use crate::posting::Acct;
use crate::{api_err, AppState, ApiError};
use axum::http::StatusCode;

pub const ROLES: [&str; 9] = [
    "customers", "suppliers", "vat_purchase", "vat_sale", "cash",
    "card_purchase", "card_sale", "revenues", "expenses",
];

fn fallback(role: &str) -> &'static str {
    match role {
        "customers" => "411",
        "suppliers" => "401",
        "vat_purchase" => "4531",
        "vat_sale" => "4532",
        "cash" => "501",
        "revenues" => "702",
        "expenses" => "602",
        _ => "",
    }
}

/// Картата default_accounts от настройките на фирмата.
async fn map(st: &AppState, company: i32) -> serde_json::Map<String, serde_json::Value> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT settings FROM companies WHERE id = {company}"),
    )
    .await
    .unwrap_or_default();
    let raw = rows.first().map(|r| cell_str(r, 0)).unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| v.get("default_accounts").cloned())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// Номерът на сметката за роля: зададеният или стандартният.
pub async fn number(st: &AppState, company: i32, role: &str) -> String {
    let m = map(st, company).await;
    let set = m.get(role).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if set.is_empty() { fallback(role).to_string() } else { set }
}

/// Сметката за роля в рамките на фирмата; липсваща сметка е 422.
pub async fn resolve(st: &AppState, company: i32, role: &str) -> Result<Acct, ApiError> {
    let num = number(st, company, role).await;
    if num.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("няма default сметка за „{role}“ — задайте я в настройките на фирмата"),
        ));
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, number, name FROM accounts WHERE number = {} AND company_id = {company}",
            lit(&num)
        ),
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("липсва сметка с номер {num}")));
    };
    Ok(Acct {
        id: cell_i32_at(&row, 0),
        number: cell_str(&row, 1),
        name: cell_str(&row, 2),
    })
}
