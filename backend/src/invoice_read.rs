//! Четене на фактура. Допълнителните колони са в invoice_extra,
//! защото този Octo няма ALTER TABLE.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_i32, cell_i32_at, cell_str, lit, Db, Row};
use crate::form::{active_or_one, auth_sub, db_err};
use crate::pagination::{paginate, PageQuery};
use crate::{api_err, companies, counterparts, AppState, ApiError};

const SEL: &str = "SELECT i.id, i.company_id, i.direction, i.document_type, i.number, i.issue_date, \
i.due_date, i.counterpart_id, i.counterpart_name, i.currency, i.net_amount, i.vat_amount, \
i.total_amount, i.status, i.payment_method, i.notes, e.tax_event_date, e.accounting_month, \
e.currency_rate, e.discount_percent, e.discount_amount, e.vat_exemption_reason, \
e.original_invoice_id, e.location_id, e.journal_entry_id \
FROM invoices i LEFT JOIN invoice_extra e ON i.id = e.id";

pub(crate) struct Inv {
    pub id: i32,
    pub company_id: i32,
    pub direction: String,
    pub document_type: String,
    pub number: String,
    pub issue_date: String,
    pub due_date: String,
    pub counterpart_id: i32,
    pub counterpart_name: String,
    pub currency: String,
    pub net_amount: String,
    pub vat_amount: String,
    pub total_amount: String,
    pub status: String,
    pub payment_method: String,
    pub notes: String,
    pub tax_event_date: String,
    pub accounting_month: String,
    pub currency_rate: String,
    pub discount_percent: String,
    pub discount_amount: String,
    pub vat_exemption_reason: String,
    pub original_invoice_id: i32,
    pub location_id: i32,
    pub journal_entry_id: i32,
}

pub(crate) struct Line {
    pub id: i32,
    pub product_id: i32,
    pub code: String,
    pub description: String,
    pub quantity: String,
    pub unit: String,
    pub unit_price: String,
    pub vat_rate: String,
    pub net_amount: String,
    pub vat_amount: String,
    pub total_amount: String,
}

pub(crate) struct Extra {
    pub tax_event_date: String,
    pub accounting_month: String,
    pub currency_rate: String,
    pub discount_percent: String,
    pub discount_amount: String,
    pub vat_exemption_reason: String,
    pub original_invoice_id: i32,
    pub location_id: i32,
    pub journal_entry_id: i32,
}

pub(crate) async fn qry(db: &Db, sql: &str) -> Result<Vec<Row>, ApiError> {
    db.query(sql).await.map_err(db_err)
}

pub(crate) async fn exe(db: &Db, sql: &str) -> Result<(), ApiError> {
    db.exec(sql).await.map_err(db_err)?;
    Ok(())
}

pub(crate) async fn nid(db: &Db, table: &str) -> Result<i32, ApiError> {
    db.next_id(table).await.map_err(db_err)
}

fn fill(inv: &mut Inv) {
    if inv.tax_event_date.trim().is_empty() {
        inv.tax_event_date = inv.issue_date.clone();
    }
    if inv.accounting_month.trim().is_empty() {
        inv.accounting_month = if inv.issue_date.len() >= 7 {
            inv.issue_date[..7].to_string()
        } else {
            inv.issue_date.clone()
        };
    }
    if inv.currency_rate.trim().is_empty() {
        inv.currency_rate = "1".into();
    }
    if inv.discount_percent.trim().is_empty() {
        inv.discount_percent = "0".into();
    }
    if inv.discount_amount.trim().is_empty() {
        inv.discount_amount = "0".into();
    }
}

fn from_row(r: &Row) -> Inv {
    Inv {
        id: cell_i32_at(r, 0),
        company_id: cell_i32_at(r, 1),
        direction: cell_str(r, 2),
        document_type: cell_str(r, 3),
        number: cell_str(r, 4),
        issue_date: cell_str(r, 5),
        due_date: cell_str(r, 6),
        counterpart_id: cell_i32_at(r, 7),
        counterpart_name: cell_str(r, 8),
        currency: cell_str(r, 9),
        net_amount: cell_str(r, 10),
        vat_amount: cell_str(r, 11),
        total_amount: cell_str(r, 12),
        status: cell_str(r, 13),
        payment_method: cell_str(r, 14),
        notes: cell_str(r, 15),
        tax_event_date: cell_str(r, 16),
        accounting_month: cell_str(r, 17),
        currency_rate: cell_str(r, 18),
        discount_percent: cell_str(r, 19),
        discount_amount: cell_str(r, 20),
        vat_exemption_reason: cell_str(r, 21),
        original_invoice_id: cell_i32_at(r, 22),
        location_id: cell_i32_at(r, 23),
        journal_entry_id: cell_i32_at(r, 24),
    }
}

async fn posted_entry(db: &Db, inv: &Inv) -> Result<i32, ApiError> {
    if inv.journal_entry_id > 0 || inv.status != "posted" {
        return Ok(inv.journal_entry_id);
    }
    let rows = qry(
        db,
        &format!(
            "SELECT id FROM journal_entries WHERE document_id = {} AND document_type = {} ORDER BY id",
            inv.id,
            lit(&inv.document_type)
        ),
    )
    .await?;
    Ok(rows.last().map(cell_i32).unwrap_or(0))
}

pub(crate) async fn load(db: &Db, id: i32) -> Result<Inv, ApiError> {
    let mut rows = qry(db, &format!("{SEL} WHERE i.id = {id}")).await?;
    let Some(row) = rows.pop() else {
        return Err(api_err(StatusCode::NOT_FOUND, "фактурата не е намерена"));
    };
    let mut inv = from_row(&row);
    fill(&mut inv);
    inv.journal_entry_id = posted_entry(db, &inv).await?;
    Ok(inv)
}

pub(crate) fn belongs(inv: &Inv, company_id: i32) -> Result<(), ApiError> {
    if inv.company_id != 0 && inv.company_id != company_id {
        return Err(api_err(StatusCode::NOT_FOUND, "фактурата не е намерена"));
    }
    Ok(())
}

pub(crate) async fn lines_of(db: &Db, invoice_id: i32) -> Result<Vec<Line>, ApiError> {
    let rows = qry(
        db,
        &format!(
            "SELECT l.id, l.code, l.description, l.quantity, l.unit, l.unit_price, l.vat_rate, \
             l.net_amount, l.vat_amount, l.total_amount, e.product_id \
             FROM invoice_lines l LEFT JOIN invoice_line_extra e ON l.id = e.id \
             WHERE l.invoice_id = {invoice_id} ORDER BY l.id"
        ),
    )
    .await?;
    Ok(rows
        .iter()
        .map(|r| Line {
            id: cell_i32_at(r, 0),
            code: cell_str(r, 1),
            description: cell_str(r, 2),
            quantity: cell_str(r, 3),
            unit: cell_str(r, 4),
            unit_price: cell_str(r, 5),
            vat_rate: cell_str(r, 6),
            net_amount: cell_str(r, 7),
            vat_amount: cell_str(r, 8),
            total_amount: cell_str(r, 9),
            product_id: cell_i32_at(r, 10),
        })
        .collect())
}

fn line_json(l: &Line) -> Value {
    let mut v = json!({
        "id": l.id,
        "code": l.code,
        "description": l.description,
        "quantity": l.quantity,
        "unit": l.unit,
        "unit_price": l.unit_price,
        "vat_rate": l.vat_rate,
        "net_amount": l.net_amount,
        "vat_amount": l.vat_amount,
        "total_amount": l.total_amount,
    });
    if l.product_id > 0 {
        v["product_id"] = json!(l.product_id);
    }
    v
}

pub(crate) async fn invoice_value(db: &Db, mut inv: Inv, with_lines: bool) -> Result<Value, ApiError> {
    fill(&mut inv);
    inv.journal_entry_id = posted_entry(db, &inv).await?;
    let mut v = json!({
        "id": inv.id,
        "company_id": inv.company_id,
        "direction": inv.direction,
        "document_type": inv.document_type,
        "number": inv.number,
        "issue_date": inv.issue_date,
        "due_date": inv.due_date,
        "tax_event_date": inv.tax_event_date,
        "accounting_month": inv.accounting_month,
        "counterpart_id": inv.counterpart_id,
        "counterpart_name": inv.counterpart_name,
        "currency": inv.currency,
        "currency_rate": inv.currency_rate,
        "net_amount": inv.net_amount,
        "vat_amount": inv.vat_amount,
        "total_amount": inv.total_amount,
        "status": inv.status,
        "payment_method": inv.payment_method,
        "notes": inv.notes,
        "discount_percent": inv.discount_percent,
        "discount_amount": inv.discount_amount,
        "vat_exemption_reason": inv.vat_exemption_reason,
        "location_id": inv.location_id,
    });
    if inv.original_invoice_id > 0 {
        v["original_invoice_id"] = json!(inv.original_invoice_id);
    }
    if inv.journal_entry_id > 0 {
        v["journal_entry_id"] = json!(inv.journal_entry_id);
    }
    if with_lines {
        let lines = lines_of(db, inv.id).await?;
        v["lines"] = json!(lines.iter().map(line_json).collect::<Vec<_>>());
    }
    Ok(v)
}

pub(crate) async fn detail(st: &AppState, sub: &str, id: i32) -> Result<Value, ApiError> {
    let company = active_or_one(st, sub).await?;
    let inv = load(&st.db, id).await?;
    belongs(&inv, company)?;
    let cp_id = inv.counterpart_id;
    let mut v = invoice_value(&st.db, inv, true).await?;
    if let Some(cp) = counterparts::json_by_id(&st.db, cp_id).await? {
        v["counterpart"] = cp;
    }
    if let Some(co) = companies::json_by_id(&st.db, company).await? {
        v["company"] = co;
    }
    Ok(v)
}

pub async fn invoices(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let rows = qry(&st.db, &format!("{SEL} WHERE i.company_id = {company} ORDER BY i.id")).await?;
    let mut items = Vec::new();
    for row in &rows {
        items.push(invoice_value(&st.db, from_row(row), false).await?);
    }
    Ok(paginate(items, &q))
}

pub async fn invoice_show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    Ok(Json(detail(&st, &sub, id).await?))
}

pub(crate) async fn save_extra(db: &Db, id: i32, e: &Extra) -> Result<(), ApiError> {
    let rows = qry(db, &format!("SELECT id FROM invoice_extra WHERE id = {id}")).await?;
    if rows.is_empty() {
        exe(
            db,
            &format!(
                "INSERT INTO invoice_extra (id, tax_event_date, accounting_month, currency_rate, \
                 discount_percent, discount_amount, vat_exemption_reason, original_invoice_id, \
                 location_id, journal_entry_id) VALUES ({id}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                lit(&e.tax_event_date),
                lit(&e.accounting_month),
                lit(&e.currency_rate),
                lit(&e.discount_percent),
                lit(&e.discount_amount),
                lit(&e.vat_exemption_reason),
                e.original_invoice_id,
                e.location_id,
                e.journal_entry_id
            ),
        )
        .await?;
    } else {
        exe(
            db,
            &format!(
                "UPDATE invoice_extra SET tax_event_date = {}, accounting_month = {}, currency_rate = {}, \
                 discount_percent = {}, discount_amount = {}, vat_exemption_reason = {}, \
                 original_invoice_id = {}, location_id = {}, journal_entry_id = {} WHERE id = {id}",
                lit(&e.tax_event_date),
                lit(&e.accounting_month),
                lit(&e.currency_rate),
                lit(&e.discount_percent),
                lit(&e.discount_amount),
                lit(&e.vat_exemption_reason),
                e.original_invoice_id,
                e.location_id,
                e.journal_entry_id
            ),
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn set_status(db: &Db, id: i32, status: &str, journal_id: i32) -> Result<(), ApiError> {
    exe(
        db,
        &format!(
            "UPDATE invoices SET status = {} WHERE id = {id}",
            lit(status)
        ),
    )
    .await?;
    let rows = qry(db, &format!("SELECT id FROM invoice_extra WHERE id = {id}")).await?;
    if rows.is_empty() {
        exe(
            db,
            &format!(
                "INSERT INTO invoice_extra (id, tax_event_date, accounting_month, currency_rate, \
                 discount_percent, discount_amount, vat_exemption_reason, original_invoice_id, \
                 location_id, journal_entry_id) VALUES ({id}, '', '', '1', '0', '0', '', 0, 0, {journal_id})"
            ),
        )
        .await?;
    } else {
        exe(
            db,
            &format!("UPDATE invoice_extra SET journal_entry_id = {journal_id} WHERE id = {id}"),
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn journal_ids(db: &Db, inv: &Inv) -> Result<Vec<i32>, ApiError> {
    let mut ids = Vec::new();
    if inv.journal_entry_id > 0 {
        ids.push(inv.journal_entry_id);
    }
    let rows = qry(
        db,
        &format!(
            "SELECT id FROM journal_entries WHERE document_id = {} AND document_type = {}",
            inv.id,
            lit(&inv.document_type)
        ),
    )
    .await?;
    for row in &rows {
        let id = cell_i32(row);
        if id > 0 && !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

pub(crate) async fn wipe_journal(db: &Db, entry_id: i32) -> Result<(), ApiError> {
    if entry_id <= 0 {
        return Ok(());
    }
    exe(db, &format!("DELETE FROM journal_lines WHERE entry_id = {entry_id}")).await?;
    exe(db, &format!("DELETE FROM journal_entries WHERE id = {entry_id}")).await?;
    Ok(())
}

pub(crate) async fn wipe_invoice(db: &Db, id: i32) -> Result<(), ApiError> {
    let rows = qry(db, &format!("SELECT id FROM invoice_lines WHERE invoice_id = {id}")).await?;
    for row in &rows {
        let line_id = cell_i32(row);
        exe(db, &format!("DELETE FROM invoice_line_extra WHERE id = {line_id}")).await?;
    }
    exe(db, &format!("DELETE FROM invoice_lines WHERE invoice_id = {id}")).await?;
    exe(db, &format!("DELETE FROM invoice_extra WHERE id = {id}")).await?;
    exe(db, &format!("DELETE FROM invoices WHERE id = {id}")).await?;
    Ok(())
}

/// Маха дневника, ако има такъв, после редовете и самата фактура.
pub(crate) async fn delete_one(db: &Db, id: i32) -> Result<(), ApiError> {
    let inv = load(db, id).await?;
    let posted = inv.status == "posted";
    let company = inv.company_id;
    for entry_id in journal_ids(db, &inv).await? {
        wipe_journal(db, entry_id).await?;
    }
    wipe_invoice(db, id).await?;
    if posted {
        crate::stock_replay::rebuild(db, company, "9999-12-31").await?;
    }
    Ok(())
}
