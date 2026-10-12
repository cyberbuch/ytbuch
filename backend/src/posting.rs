//! Запис в дневника за банки, ДМА и производство.
//! Фактурите си имат собствен запис. При грешка след заглавието редовете се трият.

use axum::http::StatusCode;

use crate::db::{cell_i32_at, cell_str, lit, Db};
use crate::invoice_read;
use crate::{api_err, ApiError};

pub struct Acct {
    pub id: i32,
    pub number: String,
    pub name: String,
}

pub struct Side {
    pub account: Acct,
    pub direction: &'static str,
    pub amount: String,
}

pub async fn by_id(db: &Db, id: i32) -> Result<Acct, ApiError> {
    let rows = invoice_read::qry(db, &format!("SELECT id, number, name FROM accounts WHERE id = {id}")).await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("липсва сметка с id {id}")));
    };
    Ok(acct(row))
}

pub async fn by_number(db: &Db, company: i32, number: &str) -> Result<Option<Acct>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT id, number, name FROM accounts WHERE number = {} AND company_id = {company}",
            lit(number)
        ),
    )
    .await?;
    Ok(rows.into_iter().next().map(acct))
}

fn acct(row: crate::db::Row) -> Acct {
    Acct { id: cell_i32_at(&row, 0), number: cell_str(&row, 1), name: cell_str(&row, 2) }
}

/// `vat_type` за тези записи е `no_vat`. Връща id на заглавието.
pub async fn write_entry(
    db: &Db,
    company: i32,
    date: &str,
    doc_type: &str,
    doc_id: i32,
    doc_number: &str,
    desc: &str,
    counterpart_id: i32,
    counterpart_name: &str,
    vat_period: &str,
    sides: &[Side],
) -> Result<i32, ApiError> {
    let entry_id = invoice_read::nid(db, "journal_entries").await?;
    if let Err(e) = invoice_read::exe(
        db,
        &format!(
            "INSERT INTO journal_entries (id, company_id, entry_date, document_type, document_id, \
             document_number, description, vat_type, counterpart_id, counterpart_name, document_date, \
             vat_period) VALUES ({entry_id}, {company}, {}, {}, {doc_id}, {}, {}, 'no_vat', \
             {counterpart_id}, {}, {}, {})",
            lit(date),
            lit(doc_type),
            lit(doc_number),
            lit(desc),
            lit(counterpart_name),
            lit(date),
            lit(vat_period)
        ),
    )
    .await
    {
        let _ = invoice_read::wipe_journal(db, entry_id).await;
        return Err(e);
    }
    for side in sides {
        let line_id = match invoice_read::nid(db, "journal_lines").await {
            Ok(id) => id,
            Err(e) => {
                let _ = invoice_read::wipe_journal(db, entry_id).await;
                return Err(e);
            }
        };
        if let Err(e) = invoice_read::exe(
            db,
            &format!(
                "INSERT INTO journal_lines (id, entry_id, account_id, account_number, account_name, \
                 direction, amount, vat_amount) VALUES ({line_id}, {entry_id}, {}, {}, {}, {}, {}, '0')",
                side.account.id,
                lit(&side.account.number),
                lit(&side.account.name),
                lit(side.direction),
                lit(&side.amount)
            ),
        )
        .await
        {
            let _ = invoice_read::wipe_journal(db, entry_id).await;
            return Err(e);
        }
    }
    Ok(entry_id)
}

pub fn month_of(date: &str) -> String {
    if date.len() >= 7 { date[..7].to_string() } else { date.to_string() }
}
