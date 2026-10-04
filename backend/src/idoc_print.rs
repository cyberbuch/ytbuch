//! Печат на приемо-предавателен протокол. Потвърждението не се пипа.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;

use crate::db::cell_str;
use crate::form::{active_or_one, auth_sub};
use crate::invoice_read;
use crate::print_load;
use crate::print_sheet::{self, Block};
use crate::{api_err, companies, idocs, AppState, ApiError};

#[derive(Deserialize)]
pub struct Q {
    format: Option<String>,
}

pub async fn print(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Query(q): Query<Q>,
) -> Result<Response, ApiError> {
    let format = q.format.as_deref().unwrap_or("").trim();
    let format = if format.is_empty() { "pdf" } else { format };
    if !matches!(format, "pdf" | "docx" | "odt") {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "format трябва да е pdf/docx/odt"));
    }
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let doc = idocs::load(&st, id, company).await?;
    let firm = companies::json_by_id(&st.db, company).await?.unwrap_or(Value::Null);
    let from_name = place(&st, company, int_of(&doc, "from_location_id")).await?;
    let to_name = place(&st, company, int_of(&doc, "to_location_id")).await?;
    let blocks = blocks(&doc, &print_load::field(&firm, "name"), &from_name, &to_name);
    let bytes = print_sheet::render(&blocks, format)?;
    let number = print_load::field(&doc, "number");
    let filename = format!("protokol-{}.{}", print_sheet::safe_name(&number), format);
    Ok(print_sheet::attachment(bytes, print_sheet::mime(format), &filename))
}

fn blocks(doc: &Value, company: &str, from_name: &str, to_name: &str) -> Vec<Block> {
    let mut out = Vec::new();
    if !company.is_empty() {
        out.push(Block::Head(company.to_string()));
    }
    out.push(Block::Head("ПРИЕМО-ПРЕДАВАТЕЛЕН ПРОТОКОЛ".into()));
    out.push(Block::Line(format!(
        "№ {} · дата {}",
        print_load::field(doc, "number"),
        print_load::field(doc, "doc_date")
    )));
    out.push(Block::Line(format!("От обект: {from_name}")));
    out.push(Block::Line(format!("Към обект: {to_name}")));
    let mut rows = vec![vec![
        "№".into(),
        "Вид".into(),
        "Наименование".into(),
        "К-во".into(),
        "Себестойност".into(),
        "Сума".into(),
    ]];
    let empty = Vec::new();
    let lines = doc.get("lines").and_then(|v| v.as_array()).unwrap_or(&empty);
    for (i, line) in lines.iter().enumerate() {
        let kind = print_load::field(line, "line_kind");
        let label = if kind == "fixed_asset" { "ДМА" } else { "Артикул" };
        rows.push(vec![
            (i + 1).to_string(),
            label.into(),
            print_load::field(line, "description"),
            print_load::field(line, "quantity"),
            print_load::field(line, "unit_cost"),
            print_load::field(line, "amount"),
        ]);
    }
    out.push(Block::Table(rows));
    let notes = print_load::field(doc, "notes");
    if !notes.is_empty() {
        out.push(Block::Line(format!("Забележка: {notes}")));
    }
    out.push(Block::Line(format!("Предал: {} _______________", print_load::field(doc, "handed_by"))));
    out.push(Block::Line(format!("Приел: {} _______________", print_load::field(doc, "received_by"))));
    out
}

async fn place(st: &AppState, company: i32, id: i32) -> Result<String, ApiError> {
    if id <= 0 {
        return Ok(String::new());
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT name FROM company_locations WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    Ok(rows.first().map(|r| cell_str(r, 0)).unwrap_or_default())
}

fn int_of(v: &Value, key: &str) -> i32 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0) as i32
}
