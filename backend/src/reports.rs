//! Справки от дневника и началните салда. Експортът е PDF, XLSX или ODS.

use std::collections::HashMap;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::db::{cell_str, lit};
use crate::form::{active_or_one, auth_sub};
use crate::reports_data::{self, day_ok, firm_line, kind_ok, Books, Built};
use crate::{api_err, AppState, ApiError};

#[derive(Deserialize)]
pub struct Rq {
    kind: Option<String>,
    from: Option<String>,
    to: Option<String>,
    account: Option<String>,
    counterpart_id: Option<String>,
    format: Option<String>,
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Rq>,
) -> Result<Json<Value>, ApiError> {
    let built = prepare(&st, &headers, &q).await?;
    Ok(Json(built.body))
}

pub async fn export(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Rq>,
) -> Result<Response, ApiError> {
    let format = q.format.as_deref().unwrap_or("").trim();
    if !matches!(format, "pdf" | "xlsx" | "ods") {
        return Err(api_err(StatusCode::BAD_REQUEST, "format трябва да е pdf, xlsx или ods"));
    }
    let built = prepare(&st, &headers, &q).await?;
    let (bytes, mime) = match format {
        "pdf" => (crate::reports_pdf::render(&built.grid)?, "application/pdf"),
        "xlsx" => (
            crate::reports_sheet::xlsx(&built.grid),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
        _ => (crate::reports_sheet::ods(&built.grid), "application/vnd.oasis.opendocument.spreadsheet"),
    };
    let from = q.from.as_deref().unwrap_or("").trim();
    let to = q.to.as_deref().unwrap_or("").trim();
    let filename = format!("{}-{from}-{to}.{format}", built.slug);
    let disp = format!("attachment; filename=\"{filename}\"");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_DISPOSITION, disp)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

async fn prepare(st: &AppState, headers: &HeaderMap, q: &Rq) -> Result<Built, ApiError> {
    let sub = auth_sub(st, headers)?;
    let company = active_or_one(st, &sub).await?;
    let kind = q.kind.as_deref().unwrap_or("").trim();
    let kind = if kind.is_empty() { "trial_balance" } else { kind };
    if !kind_ok(kind) {
        return Err(api_err(StatusCode::BAD_REQUEST, "непознат kind"));
    }
    let from = q.from.as_deref().unwrap_or("").trim();
    let to = q.to.as_deref().unwrap_or("").trim();
    if !day_ok(from) || !day_ok(to) {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            "from и to са задължителни (YYYY-MM-DD)",
        ));
    }
    if from > to {
        return Err(api_err(StatusCode::BAD_REQUEST, "from трябва да е преди to"));
    }
    let account = q.account.as_deref().unwrap_or("").trim();
    let raw_cp = q.counterpart_id.as_deref().unwrap_or("").trim();
    let cpid: i32 = if raw_cp.is_empty() {
        0
    } else {
        raw_cp.parse().unwrap_or(-1)
    };
    let wants_cp = kind == "counterpart_trial" || kind == "counterpart_chrono";
    if wants_cp && cpid <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "counterpart_id е задължителен"));
    }
    let filter_cp = if wants_cp { cpid } else { 0 };
    let books = reports_data::load(&st.db, company, from, to, account, filter_cp).await?;
    let mut built = dispatch(st, kind, &books).await?;
    let line = firm_line(&books.firm_name, &books.firm_eik);
    if !line.is_empty() {
        built.grid.insert(0, vec![line]);
    }
    Ok(built)
}

async fn dispatch(st: &AppState, kind: &str, books: &Books) -> Result<Built, ApiError> {
    if let Some(statement) = reports_data::fs_statement(kind) {
        let formulas = formula_map(st, books, statement).await?;
        return Ok(crate::reports_fs::build(kind, books, &formulas));
    }
    Ok(match kind {
        "chronological" | "counterpart_chrono" => crate::reports_book::chrono(kind, books),
        "general_ledger" => crate::reports_book::ledger(kind, books),
        "correspondence_ledger" => crate::reports_corr::build(kind, books),
        _ => crate::reports_trial::build(kind, books),
    })
}

async fn formula_map(st: &AppState, books: &Books, statement: &str) -> Result<HashMap<String, String>, ApiError> {
    let rows = crate::invoice_read::qry(
        &st.db,
        &format!(
            "SELECT line_code, formula FROM fs_line_formulas WHERE company_id = {} AND statement = {}",
            books.company_id,
            lit(statement)
        ),
    )
    .await?;
    let mut map = HashMap::new();
    for row in rows {
        map.insert(cell_str(&row, 0), cell_str(&row, 1));
    }
    Ok(map)
}
