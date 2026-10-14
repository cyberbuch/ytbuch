//! Чернова на фактура: създаване, редакция, номер, изтриване.
//! Постът е в invoice_post. Octo не пише вътре в транзакция, затова
//! при грешка след първия INSERT новите редове се трият обратно.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_lines::{self, NewLine};
use crate::invoice_money::{self, amount_for_doc};
use crate::invoice_read::{self, Extra, Inv};
use crate::{api_err, AppState, ApiError};

pub(crate) struct Head {
    pub(crate) direction: String,
    pub(crate) document_type: String,
    pub(crate) number: String,
    pub(crate) issue_date: String,
    pub(crate) due_date: String,
    pub(crate) tax_event_date: String,
    pub(crate) accounting_month: String,
    pub(crate) counterpart_id: i32,
    pub(crate) currency: String,
    pub(crate) currency_rate: String,
    pub(crate) payment_method: String,
    pub(crate) notes: String,
    pub(crate) discount_percent: String,
    pub(crate) discount_amount: String,
    pub(crate) vat_exemption_reason: String,
    pub(crate) original_invoice_id: i32,
    pub(crate) location_id: i32,
}

fn opt_str(obj: &Map<String, Value>, key: &str, default: &str) -> Result<String, ApiError> {
    Ok(form::str_field(obj, key)?.unwrap_or_else(|| default.to_string()))
}

fn money_field(obj: &Map<String, Value>, key: &str) -> Result<String, ApiError> {
    let raw = form::required(form::str_field(obj, key)?, key)?;
    if invoice_money::cents(&raw).is_none() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} е невалидна сума"),
        ));
    }
    Ok(raw)
}

fn month_of(issue_date: &str, given: &str) -> String {
    let g = given.trim();
    if !g.is_empty() {
        return g.to_string();
    }
    if issue_date.len() >= 7 {
        issue_date[..7].to_string()
    } else {
        issue_date.to_string()
    }
}

fn parse_line(obj: &Map<String, Value>, doc_type: &str) -> Result<NewLine, ApiError> {
    let product_id = form::i32_field(obj, "product_id")?.unwrap_or(0);
    let mut unit = opt_str(obj, "unit", "C62")?;
    if unit.trim().is_empty() {
        unit = "C62".into();
    }
    let mut vat_rate = opt_str(obj, "vat_rate", "20")?;
    if vat_rate.trim().is_empty() {
        vat_rate = "20".into();
    }
    Ok(NewLine {
        product_id,
        code: opt_str(obj, "code", "")?,
        description: opt_str(obj, "description", "")?,
        quantity: money_field(obj, "quantity")?,
        unit,
        unit_price: money_field(obj, "unit_price")?,
        vat_rate,
        net_amount: amount_for_doc(doc_type, &money_field(obj, "net_amount")?),
        vat_amount: amount_for_doc(doc_type, &money_field(obj, "vat_amount")?),
        total_amount: amount_for_doc(doc_type, &money_field(obj, "total_amount")?),
    })
}

fn parse_lines(value: &Value, doc_type: &str) -> Result<Vec<NewLine>, ApiError> {
    let Some(arr) = value.as_array() else {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "lines трябва да е масив",
        ));
    };
    let mut lines = Vec::new();
    for el in arr {
        let Some(obj) = el.as_object() else {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "редът трябва да е обект",
            ));
        };
        lines.push(parse_line(obj, doc_type)?);
    }
    Ok(lines)
}

fn check_head(head: &Head) -> Result<(), ApiError> {
    if head.direction != "in" && head.direction != "out" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "direction must be 'in' or 'out'",
        ));
    }
    if head.issue_date.trim().is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "issue_date е задължително",
        ));
    }
    if head.counterpart_id <= 0 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "counterpart_id е задължително",
        ));
    }
    if !invoice_money::valid_for(&head.direction, &head.document_type) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "невалиден вид данъчен документ за тази посока (код по ППДДС)",
        ));
    }
    if invoice_money::requires_original(&head.document_type) && head.original_invoice_id <= 0 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "известието трябва да сочи към първоначална фактура",
        ));
    }
    if head.direction == "in" && head.number.trim().is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "номерът на входящия документ е задължителен",
        ));
    }
    for (key, value) in [
        ("discount_percent", head.discount_percent.as_str()),
        ("discount_amount", head.discount_amount.as_str()),
        ("currency_rate", head.currency_rate.as_str()),
    ] {
        if invoice_money::cents(value).is_none() {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("{key} е невалидна сума"),
            ));
        }
    }
    Ok(())
}

fn head_from_create(obj: &Map<String, Value>) -> Result<(Head, Vec<NewLine>), ApiError> {
    let direction = form::required(form::str_field(obj, "direction")?, "direction")?;
    let issue_date = form::required(form::str_field(obj, "issue_date")?, "issue_date")?;
    let document_type = {
        let t = opt_str(obj, "document_type", "01")?;
        if t.trim().is_empty() { "01".into() } else { t.trim().to_string() }
    };
    let tax = opt_str(obj, "tax_event_date", "")?;
    let month = opt_str(obj, "accounting_month", "")?;
    let head = Head {
        direction: direction.trim().to_string(),
        document_type,
        number: opt_str(obj, "number", "")?.trim().to_string(),
        issue_date: issue_date.trim().to_string(),
        due_date: opt_str(obj, "due_date", "")?,
        tax_event_date: if tax.trim().is_empty() { issue_date.trim().to_string() } else { tax },
        accounting_month: month_of(issue_date.trim(), &month),
        counterpart_id: form::i32_field(obj, "counterpart_id")?.unwrap_or(0),
        currency: {
            let c = opt_str(obj, "currency", "EUR")?;
            if c.trim().is_empty() { "EUR".into() } else { c }
        },
        currency_rate: {
            let c = opt_str(obj, "currency_rate", "1")?;
            if c.trim().is_empty() { "1".into() } else { c }
        },
        payment_method: opt_str(obj, "payment_method", "")?,
        notes: opt_str(obj, "notes", "")?,
        discount_percent: {
            let c = opt_str(obj, "discount_percent", "0")?;
            if c.trim().is_empty() { "0".into() } else { c }
        },
        discount_amount: {
            let c = opt_str(obj, "discount_amount", "0")?;
            if c.trim().is_empty() { "0".into() } else { c }
        },
        vat_exemption_reason: opt_str(obj, "vat_exemption_reason", "")?,
        original_invoice_id: form::i32_field(obj, "original_invoice_id")?.unwrap_or(0),
        location_id: form::i32_field(obj, "location_id")?.unwrap_or(0),
    };
    check_head(&head)?;
    let lines = match obj.get("lines") {
        Some(v) => parse_lines(v, &head.document_type)?,
        None => Vec::new(),
    };
    Ok((head, lines))
}

fn overlay(head: &mut Head, obj: &Map<String, Value>) -> Result<Option<Vec<NewLine>>, ApiError> {
    if let Some(v) = form::str_field(obj, "direction")? {
        head.direction = v.trim().to_string();
    }
    if let Some(v) = form::str_field(obj, "document_type")? {
        if !v.trim().is_empty() {
            head.document_type = v.trim().to_string();
        }
    }
    if let Some(v) = form::str_field(obj, "number")? {
        head.number = v.trim().to_string();
    }
    if let Some(v) = form::str_field(obj, "issue_date")? {
        head.issue_date = v.trim().to_string();
    }
    if let Some(v) = form::str_field(obj, "due_date")? {
        head.due_date = v;
    }
    if let Some(v) = form::str_field(obj, "tax_event_date")? {
        head.tax_event_date = v;
    }
    if let Some(v) = form::str_field(obj, "accounting_month")? {
        head.accounting_month = v;
    }
    if let Some(v) = form::i32_field(obj, "counterpart_id")? {
        head.counterpart_id = v;
    }
    if let Some(v) = form::str_field(obj, "currency")? {
        head.currency = v;
    }
    if let Some(v) = form::str_field(obj, "currency_rate")? {
        head.currency_rate = v;
    }
    if let Some(v) = form::str_field(obj, "payment_method")? {
        head.payment_method = v;
    }
    if let Some(v) = form::str_field(obj, "notes")? {
        head.notes = v;
    }
    if let Some(v) = form::str_field(obj, "discount_percent")? {
        head.discount_percent = v;
    }
    if let Some(v) = form::str_field(obj, "discount_amount")? {
        head.discount_amount = v;
    }
    if let Some(v) = form::str_field(obj, "vat_exemption_reason")? {
        head.vat_exemption_reason = v;
    }
    if let Some(v) = form::i32_field(obj, "original_invoice_id")? {
        head.original_invoice_id = v;
    }
    if let Some(v) = form::i32_field(obj, "location_id")? {
        head.location_id = v;
    }
    if head.tax_event_date.trim().is_empty() {
        head.tax_event_date = head.issue_date.clone();
    }
    head.accounting_month = month_of(&head.issue_date, &head.accounting_month);
    check_head(head)?;
    match obj.get("lines") {
        Some(v) => Ok(Some(parse_lines(v, &head.document_type)?)),
        None => Ok(None),
    }
}

fn head_of(inv: &Inv) -> Head {
    Head {
        direction: inv.direction.clone(),
        document_type: inv.document_type.clone(),
        number: inv.number.clone(),
        issue_date: inv.issue_date.clone(),
        due_date: inv.due_date.clone(),
        tax_event_date: inv.tax_event_date.clone(),
        accounting_month: inv.accounting_month.clone(),
        counterpart_id: inv.counterpart_id,
        currency: inv.currency.clone(),
        currency_rate: inv.currency_rate.clone(),
        payment_method: inv.payment_method.clone(),
        notes: inv.notes.clone(),
        discount_percent: inv.discount_percent.clone(),
        discount_amount: inv.discount_amount.clone(),
        vat_exemption_reason: inv.vat_exemption_reason.clone(),
        original_invoice_id: inv.original_invoice_id,
        location_id: inv.location_id,
    }
}

fn extra_of(head: &Head, journal_id: i32) -> Extra {
    Extra {
        tax_event_date: head.tax_event_date.clone(),
        accounting_month: head.accounting_month.clone(),
        currency_rate: head.currency_rate.clone(),
        discount_percent: head.discount_percent.clone(),
        discount_amount: head.discount_amount.clone(),
        vat_exemption_reason: head.vat_exemption_reason.clone(),
        original_invoice_id: head.original_invoice_id,
        location_id: head.location_id,
        journal_entry_id: journal_id,
    }
}

fn pad_num(n: i32, width: i32) -> String {
    let w = width.clamp(1, 10) as usize;
    let s = n.max(0).to_string();
    if s.len() >= w {
        s
    } else {
        format!("{s:0>w$}")
    }
}

fn csv_has(csv: &str, needle: &str) -> bool {
    csv.split(',').map(str::trim).any(|p| p == needle)
}

pub(crate) async fn next_out(st: &AppState, company_id: i32, doc_type: &str) -> Result<String, ApiError> {
    let series = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT document_types, pad_width, start_number FROM document_series WHERE company_id = {company_id} ORDER BY id"
        ),
    )
    .await?;
    let mut types = doc_type.to_string();
    let mut pad = 10;
    let mut start = 1;
    for row in &series {
        let csv = cell_str(row, 0);
        if csv_has(&csv, doc_type) {
            types = csv;
            pad = cell_i32_at(row, 1);
            start = cell_i32_at(row, 2);
            break;
        }
    }
    if pad <= 0 {
        pad = 10;
    }
    if start <= 0 {
        start = 1;
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT document_type, number FROM invoices WHERE company_id = {company_id} AND direction = 'out'"
        ),
    )
    .await?;
    let mut base = start - 1;
    if base < 0 {
        base = 0;
    }
    for row in &rows {
        if !csv_has(&types, &cell_str(row, 0)) {
            continue;
        }
        let n: i32 = cell_str(row, 1).trim().parse().unwrap_or(0);
        if n > base {
            base = n;
        }
    }
    Ok(pad_num(base.saturating_add(1), pad))
}

async fn insert_header(
    st: &AppState,
    company_id: i32,
    head: &Head,
    name: &str,
    lines: &[NewLine],
) -> Result<i32, ApiError> {
    let net = invoice_money::sum_amounts(&lines.iter().map(|l| l.net_amount.clone()).collect::<Vec<_>>());
    let vat = invoice_money::sum_amounts(&lines.iter().map(|l| l.vat_amount.clone()).collect::<Vec<_>>());
    let total = invoice_money::sum_amounts(&lines.iter().map(|l| l.total_amount.clone()).collect::<Vec<_>>());
    let id = invoice_read::nid(&st.db, "invoices").await?;
    let inserted = invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO invoices (id, company_id, direction, document_type, number, issue_date, due_date, \
             counterpart_id, counterpart_name, currency, net_amount, vat_amount, total_amount, status, \
             payment_method, notes) VALUES ({id}, {company_id}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'draft', {}, {})",
            lit(&head.direction),
            lit(&head.document_type),
            lit(&head.number),
            lit(&head.issue_date),
            lit(&head.due_date),
            head.counterpart_id,
            lit(name),
            lit(&head.currency),
            lit(&net),
            lit(&vat),
            lit(&total),
            lit(&head.payment_method),
            lit(&head.notes)
        ),
    )
    .await;
    inserted?;
    if let Err(e) = invoice_read::save_extra(&st.db, id, &extra_of(head, 0)).await {
        let _ = invoice_read::wipe_invoice(&st.db, id).await;
        return Err(e);
    }
    for line in lines {
        if let Err(e) = invoice_lines::insert_line(st, id, company_id, line).await {
            let _ = invoice_read::wipe_invoice(&st.db, id).await;
            return Err(e);
        }
    }
    Ok(id)
}

async fn check_in_duplicate(
    st: &AppState,
    company_id: i32,
    head: &Head,
    exclude_id: i32,
) -> Result<(), ApiError> {
    if head.direction != "in" || head.number.trim().is_empty() || head.counterpart_id <= 0 {
        return Ok(());
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, number FROM invoices WHERE company_id = {company_id} AND direction = 'in' AND counterpart_id = {} AND id != {exclude_id}",
            head.counterpart_id
        ),
    )
    .await?;
    let want = head.number.trim();
    if rows.iter().any(|r| cell_str(r, 1).trim().eq_ignore_ascii_case(want)) {
        return Err(api_err(
            StatusCode::CONFLICT,
            "входящ документ с този номер от този доставчик вече съществува",
        ));
    }
    Ok(())
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let (mut head, lines) = head_from_create(obj)?;
    if head.number.is_empty() {
        head.number = next_out(&st, company, &head.document_type).await?;
    }
    check_in_duplicate(&st, company, &head, 0).await?;
    let name = invoice_lines::prepare(&st, company, &head, 0).await?;
    let id = insert_header(&st, company, &head, &name, &lines).await?;
    Ok((StatusCode::CREATED, Json(invoice_read::detail(&st, &sub, id).await?)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let inv = invoice_read::load(&st.db, id).await?;
    invoice_read::belongs(&inv, company)?;
    if inv.status == "posted" {
        return Err(api_err(
            StatusCode::CONFLICT,
            "фактурата е осчетоводена — първо я върни към чернова",
        ));
    }
    let obj = form::object(&body)?;
    let mut head = head_of(&inv);
    let lines = overlay(&mut head, obj)?;
    if head.direction == "out" && head.number.is_empty() {
        head.number = next_out(&st, company, &head.document_type).await?;
    }
    check_in_duplicate(&st, company, &head, id).await?;
    let name = invoice_lines::prepare(&st, company, &head, id).await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE invoices SET direction = {}, document_type = {}, number = {}, issue_date = {}, \
             due_date = {}, counterpart_id = {}, counterpart_name = {}, currency = {}, \
             payment_method = {}, notes = {} WHERE id = {id}",
            lit(&head.direction),
            lit(&head.document_type),
            lit(&head.number),
            lit(&head.issue_date),
            lit(&head.due_date),
            head.counterpart_id,
            lit(&name),
            lit(&head.currency),
            lit(&head.payment_method),
            lit(&head.notes)
        ),
    )
    .await?;
    invoice_read::save_extra(&st.db, id, &extra_of(&head, inv.journal_entry_id)).await?;
    if let Some(lines) = lines {
        invoice_lines::replace_lines(&st, id, company, &lines).await?;
        invoice_lines::write_totals(&st, id, &lines).await?;
    }
    Ok(Json(invoice_read::detail(&st, &sub, id).await?))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let inv = invoice_read::load(&st.db, id).await?;
    invoice_read::belongs(&inv, company)?;
    invoice_read::delete_one(&st.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct NextQuery {
    document_type: Option<String>,
    direction: Option<String>,
}

pub async fn next_number(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<NextQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let direction = q.direction.unwrap_or_else(|| "out".into());
    let direction = direction.trim();
    if direction != "in" && direction != "out" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "direction must be 'in' or 'out'",
        ));
    }
    let document_type = q.document_type.unwrap_or_else(|| "01".into());
    let document_type = document_type.trim();
    let document_type = if document_type.is_empty() { "01" } else { document_type };
    let number = if direction == "out" {
        next_out(&st, company, document_type).await?
    } else {
        String::new()
    };
    Ok(Json(json!({
        "number": number,
        "document_type": document_type,
        "direction": direction,
    })))
}

pub(crate) fn bulk_ids(body: &Value) -> Result<Vec<i32>, ApiError> {
    let obj = form::object(body)?;
    let Some(raw) = obj.get("ids") else {
        return Err(api_err(StatusCode::BAD_REQUEST, "липсва ids"));
    };
    let Some(arr) = raw.as_array() else {
        return Err(api_err(StatusCode::BAD_REQUEST, "ids трябва да е масив"));
    };
    if arr.is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "ids е празен"));
    }
    if arr.len() > 500 {
        return Err(api_err(StatusCode::BAD_REQUEST, "ids е твърде дълъг (макс 500)"));
    }
    let mut ids = Vec::new();
    for (i, el) in arr.iter().enumerate() {
        let n = match el {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        };
        let Some(n) = n else {
            return Err(api_err(StatusCode::BAD_REQUEST, format!("ids[{i}] не е число")));
        };
        if n <= 0 || n > i32::MAX as i64 {
            return Err(api_err(StatusCode::BAD_REQUEST, format!("ids[{i}] не е число")));
        }
        ids.push(n as i32);
    }
    Ok(ids)
}

pub(crate) fn err_text(err: &ApiError) -> String {
    err.1
        .0
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("грешка")
        .to_string()
}

pub(crate) fn bulk_body(key: &str, done: i32, failed: i32, results: Vec<Value>) -> Value {
    json!({ key: done, "failed": failed, "results": results })
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
        let res = async {
            let inv = invoice_read::load(&st.db, id).await?;
            invoice_read::belongs(&inv, company)?;
            invoice_read::delete_one(&st.db, id).await
        }
        .await;
        match res {
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
