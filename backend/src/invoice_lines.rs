//! Редове на фактура и проверки на препратките към фирмата.

use axum::http::StatusCode;

use crate::db::{cell_i32_at, cell_str, lit};
use crate::invoice_money;
use crate::invoice_read;
use crate::invoices::Head;
use crate::{api_err, AppState, ApiError};

pub(crate) struct NewLine {
    pub(crate) product_id: i32,
    pub(crate) code: String,
    pub(crate) description: String,
    pub(crate) quantity: String,
    pub(crate) unit: String,
    pub(crate) unit_price: String,
    pub(crate) vat_rate: String,
    pub(crate) net_amount: String,
    pub(crate) vat_amount: String,
    pub(crate) total_amount: String,
}

pub(crate) async fn counterpart_name(
    st: &AppState,
    id: i32,
    company_id: i32,
) -> Result<String, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT name FROM counterparts WHERE id = {id} AND company_id = {company_id}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "контрагентът не е намерен",
        ));
    };
    Ok(cell_str(row, 0))
}

pub(crate) async fn product_code(
    st: &AppState,
    id: i32,
    company_id: i32,
) -> Result<String, ApiError> {
    if id <= 0 {
        return Ok(String::new());
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT code FROM products WHERE id = {id} AND company_id = {company_id}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "стоката не е намерена"));
    };
    Ok(cell_str(row, 0))
}

pub(crate) async fn ensure_location(
    st: &AppState,
    company_id: i32,
    location_id: i32,
) -> Result<(), ApiError> {
    if location_id <= 0 {
        return Ok(());
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id FROM company_locations WHERE id = {location_id} AND company_id = {company_id}"
        ),
    )
    .await?;
    if rows.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "обектът не е намерен"));
    }
    Ok(())
}

pub(crate) async fn ensure_original(
    st: &AppState,
    company_id: i32,
    original_id: i32,
    self_id: i32,
) -> Result<(), ApiError> {
    if original_id <= 0 {
        return Ok(());
    }
    if original_id == self_id {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "известието не може да сочи към себе си",
        ));
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id FROM invoices WHERE id = {original_id} AND company_id = {company_id}"),
    )
    .await?;
    if rows.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "първоначалната фактура не е намерена",
        ));
    }
    Ok(())
}

pub(crate) async fn insert_line(
    st: &AppState,
    invoice_id: i32,
    company_id: i32,
    line: &NewLine,
) -> Result<i32, ApiError> {
    let mut code = line.code.clone();
    let looked = product_code(st, line.product_id, company_id).await?;
    if code.trim().is_empty() {
        code = looked;
    }
    let id = invoice_read::nid(&st.db, "invoice_lines").await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO invoice_lines (id, invoice_id, code, description, quantity, unit, unit_price, \
             vat_rate, net_amount, vat_amount, total_amount) VALUES ({id}, {invoice_id}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            lit(code.trim()),
            lit(&line.description),
            lit(&line.quantity),
            lit(line.unit.trim()),
            lit(&line.unit_price),
            lit(line.vat_rate.trim()),
            lit(&line.net_amount),
            lit(&line.vat_amount),
            lit(&line.total_amount)
        ),
    )
    .await?;
    if line.product_id > 0 {
        invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO invoice_line_extra (id, product_id) VALUES ({id}, {})",
                line.product_id
            ),
        )
        .await?;
    }
    Ok(id)
}

pub(crate) async fn drop_line(st: &AppState, line_id: i32) -> Result<(), ApiError> {
    invoice_read::exe(
        &st.db,
        &format!("DELETE FROM invoice_line_extra WHERE id = {line_id}"),
    )
    .await?;
    invoice_read::exe(
        &st.db,
        &format!("DELETE FROM invoice_lines WHERE id = {line_id}"),
    )
    .await
}

pub(crate) async fn replace_lines(
    st: &AppState,
    invoice_id: i32,
    company_id: i32,
    lines: &[NewLine],
) -> Result<(), ApiError> {
    let old = invoice_read::qry(
        &st.db,
        &format!("SELECT id FROM invoice_lines WHERE invoice_id = {invoice_id}"),
    )
    .await?;
    let old_ids: Vec<i32> = old.iter().map(|r| cell_i32_at(r, 0)).collect();
    let mut made = Vec::new();
    for line in lines {
        match insert_line(st, invoice_id, company_id, line).await {
            Ok(id) => made.push(id),
            Err(e) => {
                for id in made {
                    let _ = drop_line(st, id).await;
                }
                return Err(e);
            }
        }
    }
    for id in old_ids {
        drop_line(st, id).await?;
    }
    Ok(())
}

pub(crate) async fn write_totals(
    st: &AppState,
    invoice_id: i32,
    lines: &[NewLine],
) -> Result<(), ApiError> {
    let net = invoice_money::sum_amounts(&lines.iter().map(|l| l.net_amount.clone()).collect::<Vec<_>>());
    let vat = invoice_money::sum_amounts(&lines.iter().map(|l| l.vat_amount.clone()).collect::<Vec<_>>());
    let total = invoice_money::sum_amounts(&lines.iter().map(|l| l.total_amount.clone()).collect::<Vec<_>>());
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE invoices SET net_amount = {}, vat_amount = {}, total_amount = {} WHERE id = {invoice_id}",
            lit(&net),
            lit(&vat),
            lit(&total)
        ),
    )
    .await
}

pub(crate) async fn prepare(
    st: &AppState,
    company_id: i32,
    head: &Head,
    self_id: i32,
) -> Result<String, ApiError> {
    ensure_location(st, company_id, head.location_id).await?;
    ensure_original(st, company_id, head.original_invoice_id, self_id).await?;
    counterpart_name(st, head.counterpart_id, company_id).await
}
