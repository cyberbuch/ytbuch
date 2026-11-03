//! Ръчен запис в дневника: създаване, редактиране и изтриване.
//! Само записи без фактура (document_type = 'manual') могат да се променят.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_read;
use crate::invoices::{bulk_body, bulk_ids, err_text};
use crate::periods;
use crate::{api_err, AppState, ApiError};

const MANUAL_TYPE: &str = "manual";

fn is_manual(row_type: &str) -> bool {
    row_type.is_empty() || row_type == MANUAL_TYPE
}

async fn resolve_counterpart_name(
    db: &crate::db::Db,
    counterpart_id: i32,
    given: &str,
    company: i32,
) -> Result<String, ApiError> {
    if !given.trim().is_empty() {
        return Ok(given.trim().to_string());
    }
    if counterpart_id <= 0 {
        return Ok(String::new());
    }
    let rows = invoice_read::qry(
        db,
        &format!("SELECT name FROM counterparts WHERE id = {counterpart_id} AND company_id = {company}"),
    )
    .await?;
    Ok(rows.first().map(|r| cell_str(r, 0)).unwrap_or_default())
}

fn parse_vat_type(raw: Option<String>) -> String {
    match raw.as_deref().map(str::trim).unwrap_or("no_vat") {
        "purchase" => "purchase".into(),
        "sales" => "sales".into(),
        _ => "no_vat".into(),
    }
}

async fn entry_type(db: &crate::db::Db, id: i32, company: i32) -> Result<String, ApiError> {
    let rows = invoice_read::qry(db, &format!("SELECT document_type FROM journal_entries WHERE id = {id} AND company_id = {company}")).await?;
    Ok(cell_str(rows.first().ok_or_else(|| {
        api_err(StatusCode::NOT_FOUND, "записът не е намерен")
    })?, 0))
}

fn parse_lines(obj: &Map<String, Value>) -> Result<Vec<Line>, ApiError> {
    let arr = obj.get("lines").and_then(|v| v.as_array()).ok_or_else(|| {
        api_err(StatusCode::UNPROCESSABLE_ENTITY, "lines е задължително")
    })?;
    if arr.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "lines не може да е празен"));
    }
    let mut lines = Vec::new();
    for (i, item) in arr.iter().enumerate() {
        let o = item.as_object().ok_or_else(|| {
            api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("lines[{i}] трябва да е обект"))
        })?;
        let account_id = form::i32_field(o, "account_id")?.unwrap_or(0);
        if account_id <= 0 {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("lines[{i}].account_id е задължителен")));
        }
        let direction = form::str_field(o, "direction")?.unwrap_or_default();
        let direction = direction.trim().to_string();
        if direction != "debit" && direction != "credit" {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("lines[{i}].direction трябва да е debit или credit")));
        }
        let amount = form::str_field(o, "amount")?.unwrap_or_default();
        let amount = amount.trim().to_string();
        if amount.is_empty() || amount == "0" || amount == "0.00" {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("lines[{i}].amount не може да е 0")));
        }
        let vat_amount = form::str_field(o, "vat_amount")?.unwrap_or_else(|| "0".to_string());
        lines.push(Line { account_id, direction, amount, vat_amount: vat_amount.trim().to_string() });
    }
    Ok(lines)
}

struct Line {
    account_id: i32,
    direction: String,
    amount: String,
    vat_amount: String,
}

fn check_balanced(lines: &[Line]) -> Result<(), ApiError> {
    let mut debit = 0i64;
    let mut credit = 0i64;
    for l in lines {
        let cents = crate::invoice_money::cents(&l.amount).ok_or_else(|| {
            api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("невалидна сума: {}", l.amount))
        })?;
        if l.direction == "debit" {
            debit += cents;
        } else {
            credit += cents;
        }
    }
    if debit != credit {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("записът не е балансиран: Дт={} Кт={}", crate::invoice_money::money(debit), crate::invoice_money::money(credit)),
        ));
    }
    Ok(())
}

fn entry_debit_total(v: &Value) -> String {
    let mut debit = 0i64;
    if let Some(arr) = v.get("lines").and_then(|x| x.as_array()) {
        for l in arr {
            if l.get("direction").and_then(|d| d.as_str()) == Some("debit") {
                let raw = l.get("amount").and_then(|a| a.as_str()).unwrap_or("0");
                debit += crate::invoice_money::cents(raw).unwrap_or(0);
            }
        }
    }
    crate::invoice_money::money(debit)
}

fn entry_accounts_summary(v: &Value) -> String {
    let mut debits = Vec::new();
    let mut credits = Vec::new();
    if let Some(arr) = v.get("lines").and_then(|x| x.as_array()) {
        for l in arr {
            let num = l.get("account_number").and_then(|a| a.as_str()).unwrap_or("");
            if num.is_empty() {
                continue;
            }
            if l.get("direction").and_then(|d| d.as_str()) == Some("debit") {
                if !debits.contains(&num.to_string()) {
                    debits.push(num.to_string());
                }
            } else if !credits.contains(&num.to_string()) {
                credits.push(num.to_string());
            }
        }
    }
    format!("Дт {} / Кт {}", debits.join(","), credits.join(","))
}

async fn write_lines(db: &crate::db::Db, entry_id: i32, lines: &[Line], company: i32) -> Result<(), ApiError> {
    for l in lines {
        form::ensure_row(db, "accounts", l.account_id, company, "сметката").await?;
        let acct = invoice_read::qry(
            db,
            &format!("SELECT id, number, name FROM accounts WHERE id = {} AND company_id = {company}", l.account_id),
        ).await?;
        let Some(row) = acct.first() else {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("липсва сметка с id {}", l.account_id)));
        };
        let line_id = invoice_read::nid(db, "journal_lines").await?;
        invoice_read::exe(
            db,
            &format!(
                "INSERT INTO journal_lines (id, entry_id, account_id, account_number, account_name, direction, amount, vat_amount) VALUES ({line_id}, {entry_id}, {}, {}, {}, {}, {}, {})",
                l.account_id,
                lit(&cell_str(row, 1)),
                lit(&cell_str(row, 2)),
                lit(&l.direction),
                lit(&l.amount),
                lit(&l.vat_amount),
            ),
        ).await?;
    }
    Ok(())
}

async fn entry_json_full(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id, company_id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period FROM journal_entries WHERE id = {id} AND company_id = {company}"),
    ).await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "записът не е намерен"));
    };
    let mut value = json!({
        "id": cell_i32_at(row, 0),
        "company_id": cell_i32_at(row, 1),
        "entry_date": cell_str(row, 2),
        "document_type": cell_str(row, 3),
        "document_id": cell_i32_at(row, 4),
        "document_number": cell_str(row, 5),
        "description": cell_str(row, 6),
        "vat_type": cell_str(row, 7),
        "counterpart_id": cell_i32_at(row, 8),
        "counterpart_name": cell_str(row, 9),
        "document_date": cell_str(row, 10),
        "vat_period": cell_str(row, 11),
    });
    let lines = invoice_read::qry(
        &st.db,
        &format!("SELECT id, account_id, direction, amount, vat_amount, account_number, account_name FROM journal_lines WHERE entry_id = {id} ORDER BY id"),
    ).await?;
    value["lines"] = json!(lines.iter().map(|r| json!({
        "id": cell_i32_at(r, 0),
        "account_id": cell_i32_at(r, 1),
        "direction": cell_str(r, 2),
        "amount": cell_str(r, 3),
        "vat_amount": cell_str(r, 4),
        "account_number": cell_str(r, 5),
        "account_name": cell_str(r, 6),
    })).collect::<Vec<_>>());
    let stamps = crate::audit::stamps_for(&st.db, company, "journal").await?;
    crate::audit::attach_stamp(&mut value, stamps.get(&id));
    value["audit_history"] = json!(crate::audit::history_for(&st.db, company, "journal", id).await?);
    Ok(value)
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;

    let entry_date = form::required(form::str_field(obj, "entry_date")?, "entry_date")?;
    let description = form::str_field(obj, "description")?.unwrap_or_default();
    let vat_type = parse_vat_type(form::str_field(obj, "vat_type")?);
    let counterpart_id = form::i32_field(obj, "counterpart_id")?.unwrap_or(0);
    form::ensure_ref(&st.db, "counterparts", counterpart_id, company, "контрагентът").await?;
    let raw_cp_name = form::str_field(obj, "counterpart_name")?.unwrap_or_default();
    let counterpart_name = resolve_counterpart_name(&st.db, counterpart_id, &raw_cp_name, company).await?;
    let given_doc_no = form::str_field(obj, "document_number")?.unwrap_or_default();
    let given_doc_date = form::str_field(obj, "document_date")?.unwrap_or_default();
    let document_date = if given_doc_date.trim().is_empty() {
        entry_date.clone()
    } else {
        given_doc_date.trim().to_string()
    };
    let given_vat_period = form::str_field(obj, "vat_period")?.unwrap_or_default();
    let vat_period = if !given_vat_period.trim().is_empty() {
        given_vat_period.trim().to_string()
    } else if entry_date.len() >= 7 {
        entry_date[..7].to_string()
    } else {
        entry_date.clone()
    };

    let lines = parse_lines(obj)?;
    check_balanced(&lines)?;
    for l in &lines {
        form::ensure_row(&st.db, "accounts", l.account_id, company, "сметката").await?;
    }

    periods::assert_open(&st.db, company, &entry_date).await?;

    let entry_id = invoice_read::nid(&st.db, "journal_entries").await?;
    let document_number = if given_doc_no.trim().is_empty() {
        format!("MANUAL-{entry_id}")
    } else {
        given_doc_no.trim().to_string()
    };
    if let Err(e) = invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO journal_entries (id, company_id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period) VALUES ({entry_id}, {company}, {}, '{}', 0, {}, {}, {}, {counterpart_id}, {}, {}, {})",
            lit(&entry_date),
            MANUAL_TYPE,
            lit(&document_number),
            lit(&description),
            lit(&vat_type),
            lit(&counterpart_name),
            lit(&document_date),
            lit(&vat_period),
        ),
    ).await
    {
        let _ = invoice_read::wipe_journal(&st.db, entry_id).await;
        return Err(e);
    }

    if let Err(e) = write_lines(&st.db, entry_id, &lines, company).await {
        let _ = invoice_read::wipe_journal(&st.db, entry_id).await;
        return Err(e);
    }

    let value = entry_json_full(&st, entry_id, company).await?;
    let amt = entry_debit_total(&value);
    let accts = entry_accounts_summary(&value);
    let summary = format!(
        "Създадена ръчна статия {document_number} ({entry_date}) — {description} [{accts}], оборот {amt}"
    );
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "journal",
        entry_id,
        &document_number,
        "create",
        &summary,
        "",
        &amt,
        None,
        Some(&value),
    )
    .await;
    let value = entry_json_full(&st, entry_id, company).await?;
    Ok((StatusCode::CREATED, Json(value)))
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

    let doc_type = entry_type(&st.db, id, company).await?;
    if !is_manual(&doc_type) {
        return Err(api_err(StatusCode::FORBIDDEN, "само ръчни записи могат да се редактират"));
    }
    let before = entry_json_full(&st, id, company).await?;

    let entry_date = form::required(form::str_field(obj, "entry_date")?, "entry_date")?;
    periods::assert_open(&st.db, company, &entry_date).await?;

    let description = form::str_field(obj, "description")?.unwrap_or_default();
    let vat_type = parse_vat_type(form::str_field(obj, "vat_type")?);
    let counterpart_id = form::i32_field(obj, "counterpart_id")?.unwrap_or(0);
    form::ensure_ref(&st.db, "counterparts", counterpart_id, company, "контрагентът").await?;
    let raw_cp_name = form::str_field(obj, "counterpart_name")?.unwrap_or_default();
    let counterpart_name = resolve_counterpart_name(&st.db, counterpart_id, &raw_cp_name, company).await?;
    let given_doc_no = form::str_field(obj, "document_number")?.unwrap_or_default();
    let document_number = if given_doc_no.trim().is_empty() {
        format!("MANUAL-{id}")
    } else {
        given_doc_no.trim().to_string()
    };
    let given_doc_date = form::str_field(obj, "document_date")?.unwrap_or_default();
    let document_date = if given_doc_date.trim().is_empty() {
        entry_date.clone()
    } else {
        given_doc_date.trim().to_string()
    };
    let given_vat_period = form::str_field(obj, "vat_period")?.unwrap_or_default();
    let vat_period = if !given_vat_period.trim().is_empty() {
        given_vat_period.trim().to_string()
    } else if entry_date.len() >= 7 {
        entry_date[..7].to_string()
    } else {
        entry_date.clone()
    };

    let lines = parse_lines(obj)?;
    check_balanced(&lines)?;
    for l in &lines {
        form::ensure_row(&st.db, "accounts", l.account_id, company, "сметката").await?;
    }

    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE journal_entries SET entry_date = {}, document_type = '{}', document_number = {}, description = {}, vat_type = {}, counterpart_id = {counterpart_id}, counterpart_name = {}, document_date = {}, vat_period = {} WHERE id = {id} AND company_id = {company}",
            lit(&entry_date),
            MANUAL_TYPE,
            lit(&document_number),
            lit(&description),
            lit(&vat_type),
            lit(&counterpart_name),
            lit(&document_date),
            lit(&vat_period),
        ),
    ).await?;

    invoice_read::exe(&st.db, &format!("DELETE FROM journal_lines WHERE entry_id = {id}")).await?;
    write_lines(&st.db, id, &lines, company).await?;

    let value = entry_json_full(&st, id, company).await?;
    let old_amt = entry_debit_total(&before);
    let new_amt = entry_debit_total(&value);
    let old_accts = entry_accounts_summary(&before);
    let new_accts = entry_accounts_summary(&value);
    let mut changes = Vec::new();
    if old_amt != new_amt {
        changes.push(format!("оборот: {old_amt} → {new_amt}"));
    }
    if old_accts != new_accts {
        changes.push(format!("сметки: {old_accts} → {new_accts}"));
    }
    let old_desc = before.get("description").and_then(|v| v.as_str()).unwrap_or("");
    if old_desc != description {
        changes.push(format!("описание: {old_desc} → {description}"));
    }
    let diff = if changes.is_empty() {
        format!("{new_accts}, оборот {new_amt}")
    } else {
        changes.join(", ")
    };
    let summary = format!(
        "Редактирана счетоводна статия {document_number} ({entry_date}) — {description} ({diff})"
    );
    crate::audit::record(
        &st.db,
        company,
        &sub,
        "journal",
        id,
        &document_number,
        "update",
        &summary,
        &old_amt,
        &new_amt,
        Some(&before),
        Some(&value),
    )
    .await;
    let value = entry_json_full(&st, id, company).await?;
    Ok(Json(value))
}

async fn delete_one(st: &AppState, sub: &str, id: i32, company: i32) -> Result<(), ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT document_type, entry_date, document_number, description FROM journal_entries WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "записът не е намерен"));
    };
    let doc_type = cell_str(row, 0);
    if !is_manual(&doc_type) && doc_type != crate::closing::CLOSING_TYPE {
        return Err(api_err(StatusCode::FORBIDDEN, "само ръчни и приключвателни записи могат да се изтриват"));
    }
    let entry_date = cell_str(row, 1);
    let doc_no = cell_str(row, 2);
    let desc = cell_str(row, 3);
    periods::assert_open(&st.db, company, &entry_date).await?;
    let before = entry_json_full(st, id, company).await.ok();
    let old_amt = before.as_ref().map(entry_debit_total).unwrap_or_default();
    invoice_read::wipe_journal(&st.db, id).await?;
    let summary = format!("Изтрита счетоводна статия {doc_no} ({entry_date}) — {desc}, оборот {old_amt}");
    crate::audit::record(
        &st.db,
        company,
        sub,
        "journal",
        id,
        &doc_no,
        "delete",
        &summary,
        &old_amt,
        "",
        before.as_ref(),
        None,
    )
    .await;
    Ok(())
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    delete_one(&st, &sub, id, company).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn bulk_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let ids = bulk_ids(&body)?;
    let mut done = 0;
    let mut failed = 0;
    let mut results = Vec::new();
    for id in ids {
        match delete_one(&st, &sub, id, company).await {
            Ok(()) => {
                done += 1;
                results.push(json!({ "id": id, "ok": 1 }));
            }
            Err(e) => {
                failed += 1;
                results.push(json!({ "id": id, "ok": 0, "error": err_text(&e) }));
            }
        }
    }
    Ok(Json(bulk_body("deleted", done, failed, results)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_type_check() {
        assert!(is_manual("manual"));
        assert!(is_manual(""));
        assert!(!is_manual("01"));
        assert!(!is_manual("bank"));
        assert!(!is_manual("depreciation"));
        assert!(!is_manual("stock_writeoff"));
    }

    #[test]
    fn vat_type_defaults_and_preserves() {
        assert_eq!(parse_vat_type(None), "no_vat");
        assert_eq!(parse_vat_type(Some("".into())), "no_vat");
        assert_eq!(parse_vat_type(Some("purchase".into())), "purchase");
        assert_eq!(parse_vat_type(Some("sales".into())), "sales");
        assert_eq!(parse_vat_type(Some("no_vat".into())), "no_vat");
        assert_eq!(parse_vat_type(Some("other".into())), "no_vat");
    }
}
