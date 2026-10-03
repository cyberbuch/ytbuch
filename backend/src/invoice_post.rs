//! Осчетоводяване. Продажба: Дт 411 / Кт приход (702) / Кт 4532.
//! Покупка: Дт 304 или 602 / Дт 4531 / Кт 401.
//! Кредитното известие пази същите страни и слага минус.
//! Типове 01 и 11 движат склада. При грешка след движението наличността се връща.
//! Затворен счетоводен месец спира поста.
//!
//! Octo не записва вътре в BEGIN, а процесът на хоста не отваря YottaDB
//! вътре в контейнера. Записите са поредни. Ако ред след заглавния запис
//! в дневника се провали, новите редове се трият. Прекъсване по средата
//! не е атомарно.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::form::{active_or_one, auth_sub};
use crate::invoice_money::{self, amount_for_doc};
use crate::invoice_read::{self, Inv};
use crate::invoices::{bulk_body, bulk_ids, err_text};
use crate::periods;
use crate::stock;
use crate::{api_err, AppState, ApiError};

struct Acct {
    id: i32,
    number: String,
    name: String,
}

struct Jl {
    account: Acct,
    direction: &'static str,
    amount: String,
    vat_amount: String,
}

struct Prod {
    revenue_account_id: i32,
    inventory_account_id: i32,
    is_inventory: i32,
}

async fn acct_number(st: &AppState, company: i32, number: &str) -> Result<Acct, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, number, name FROM accounts WHERE number = {} AND company_id = {company}",
            lit(number)
        ),
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("липсва сметка с номер {number}"),
        ));
    };
    Ok(Acct {
        id: cell_i32_at(&row, 0),
        number: cell_str(&row, 1),
        name: cell_str(&row, 2),
    })
}

async fn acct_id(st: &AppState, id: i32) -> Result<Acct, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id, number, name FROM accounts WHERE id = {id}"),
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("липсва сметка с id {id}"),
        ));
    };
    Ok(Acct {
        id: cell_i32_at(&row, 0),
        number: cell_str(&row, 1),
        name: cell_str(&row, 2),
    })
}

async fn acct_pref(st: &AppState, company: i32, pref: i32, fallback: &str, missing: &str) -> Result<Acct, ApiError> {
    if pref > 0 {
        return acct_id(st, pref).await;
    }
    if let Ok(a) = acct_number(st, company, fallback).await {
        return Ok(a);
    }
    // В плана на НАП няма сметка 602, а 60201…60212. 60212 е „други външни услуги“.
    if fallback == "602" {
        if let Ok(a) = acct_number(st, company, "60212").await {
            return Ok(a);
        }
    }
    Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, missing))
}

async fn product(st: &AppState, id: i32) -> Result<Option<Prod>, ApiError> {
    if id <= 0 {
        return Ok(None);
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT revenue_account_id, inventory_account_id, is_inventory FROM products WHERE id = {id}"
        ),
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(Prod {
        revenue_account_id: cell_i32_at(&row, 0),
        inventory_account_id: cell_i32_at(&row, 1),
        is_inventory: cell_i32_at(&row, 2),
    }))
}

fn push(plan: &mut Vec<Jl>, account: Acct, direction: &'static str, amount: String, vat_amount: &str) {
    plan.push(Jl {
        account,
        direction,
        amount,
        vat_amount: vat_amount.to_string(),
    });
}

async fn plan_for(st: &AppState, inv: &Inv) -> Result<Vec<Jl>, ApiError> {
    let company = inv.company_id;
    let a411 = acct_number(st, company, "411").await?;
    let a401 = acct_number(st, company, "401").await?;
    let a4531 = acct_number(st, company, "4531").await?;
    let a4532 = acct_number(st, company, "4532").await?;
    let total = amount_for_doc(&inv.document_type, &inv.total_amount);
    let vat = amount_for_doc(&inv.document_type, &inv.vat_amount);
    let lines = invoice_read::lines_of(&st.db, inv.id).await?;
    let mut plan = Vec::new();
    if inv.direction == "out" {
        push(&mut plan, a411, "debit", total, "0");
        for line in &lines {
            let net = amount_for_doc(&inv.document_type, &line.net_amount);
            let prod = product(st, line.product_id).await?;
            let pref = prod.as_ref().map(|p| p.revenue_account_id).unwrap_or(0);
            let acct = acct_pref(st, company, pref, "702", "липсва приходна сметка (702)").await?;
            push(&mut plan, acct, "credit", net, "0");
        }
        push(&mut plan, a4532, "credit", vat.clone(), &vat);
    } else {
        for line in &lines {
            let net = amount_for_doc(&inv.document_type, &line.net_amount);
            let prod = product(st, line.product_id).await?;
            let (pref, fallback, missing) = match prod.as_ref() {
                Some(p) if p.is_inventory == 1 => {
                    (p.inventory_account_id, "304", "липсва складова сметка (304)")
                }
                Some(p) => (p.inventory_account_id, "602", "липсва сметка 602"),
                None => (0, "602", "липсва сметка 602"),
            };
            let acct = acct_pref(st, company, pref, fallback, missing).await?;
            push(&mut plan, acct, "debit", net, "0");
        }
        push(&mut plan, a4531, "debit", vat.clone(), &vat);
        push(&mut plan, a401, "credit", total, "0");
    }
    Ok(plan)
}

async fn write_journal(st: &AppState, inv: &Inv, plan: &[Jl]) -> Result<Value, ApiError> {
    let vat_type = if inv.direction == "out" { "sales" } else { "purchase" };
    let desc = format!(
        "{} {} / {}",
        invoice_money::title(&inv.document_type),
        inv.number,
        inv.direction
    );
    let entry_id = invoice_read::nid(&st.db, "journal_entries").await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO journal_entries (id, company_id, entry_date, document_type, document_id, \
             document_number, description, vat_type, counterpart_id, counterpart_name, document_date, \
             vat_period) VALUES ({entry_id}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            inv.company_id,
            lit(&inv.issue_date),
            lit(&inv.document_type),
            inv.id,
            lit(&inv.number),
            lit(&desc),
            lit(vat_type),
            inv.counterpart_id,
            lit(&inv.counterpart_name),
            lit(&inv.issue_date),
            lit(&inv.accounting_month)
        ),
    )
    .await?;
    let mut out_lines = Vec::new();
    for jl in plan {
        let line_id = match invoice_read::nid(&st.db, "journal_lines").await {
            Ok(id) => id,
            Err(e) => {
                let _ = invoice_read::wipe_journal(&st.db, entry_id).await;
                return Err(e);
            }
        };
        if let Err(e) = invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO journal_lines (id, entry_id, account_id, account_number, account_name, \
                 direction, amount, vat_amount) VALUES ({line_id}, {entry_id}, {}, {}, {}, {}, {}, {})",
                jl.account.id,
                lit(&jl.account.number),
                lit(&jl.account.name),
                lit(jl.direction),
                lit(&jl.amount),
                lit(&jl.vat_amount)
            ),
        )
        .await
        {
            let _ = invoice_read::wipe_journal(&st.db, entry_id).await;
            return Err(e);
        }
        out_lines.push(json!({
            "id": line_id,
            "account_id": jl.account.id,
            "account_number": jl.account.number,
            "account_name": jl.account.name,
            "direction": jl.direction,
            "amount": jl.amount,
            "vat_amount": jl.vat_amount,
        }));
    }
    if let Err(e) = invoice_read::set_status(&st.db, inv.id, "posted", entry_id).await {
        let _ = invoice_read::wipe_journal(&st.db, entry_id).await;
        let _ = invoice_read::exe(
            &st.db,
            &format!("UPDATE invoices SET status = {} WHERE id = {}", lit(&inv.status), inv.id),
        )
        .await;
        return Err(e);
    }
    Ok(json!({
        "id": entry_id,
        "company_id": inv.company_id,
        "entry_date": inv.issue_date,
        "document_type": inv.document_type,
        "document_id": inv.id,
        "document_number": inv.number,
        "description": desc,
        "vat_type": vat_type,
        "counterpart_id": inv.counterpart_id,
        "counterpart_name": inv.counterpart_name,
        "document_date": inv.issue_date,
        "vat_period": inv.accounting_month,
        "lines": out_lines,
    }))
}

pub(crate) async fn post_one(st: &AppState, company: i32, id: i32) -> Result<Value, ApiError> {
    let inv = invoice_read::load(&st.db, id).await?;
    invoice_read::belongs(&inv, company)?;
    periods::assert_open(&st.db, &inv.accounting_month).await?;
    if inv.status == "posted" {
        return Err(api_err(StatusCode::CONFLICT, "фактурата е вече постната"));
    }
    if invoice_money::is_proforma(&inv.document_type) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "проформата не е данъчен документ и не се осчетоводява",
        ));
    }
    if inv.direction != "in" && inv.direction != "out" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "direction must be 'in' or 'out'",
        ));
    }
    let mut plan = plan_for(st, &inv).await?;
    let (sides, snaps) = stock::apply_invoice(&st.db, &inv).await?;
    plan.extend(sides.into_iter().map(from_side));
    match write_journal(st, &inv, &plan).await {
        Ok(body) => Ok(body),
        Err(e) => {
            let _ = stock::undo(&st.db, &snaps).await;
            Err(e)
        }
    }
}

fn from_side(side: crate::posting::Side) -> Jl {
    Jl {
        account: Acct {
            id: side.account.id,
            number: side.account.number,
            name: side.account.name,
        },
        direction: side.direction,
        amount: side.amount,
        vat_amount: "0".into(),
    }
}

pub(crate) async fn unpost_one(st: &AppState, company: i32, id: i32) -> Result<(), ApiError> {
    let inv = invoice_read::load(&st.db, id).await?;
    invoice_read::belongs(&inv, company)?;
    if inv.status != "posted" {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "не е осчетоводена"));
    }
    stock::restore_posted(&st.db, &inv).await?;
    let ids = invoice_read::journal_ids(&st.db, &inv).await?;
    for entry_id in ids {
        invoice_read::wipe_journal(&st.db, entry_id).await?;
    }
    invoice_read::set_status(&st.db, id, "draft", 0).await
}

pub async fn post(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let body = post_one(&st, company, id).await?;
    Ok((StatusCode::CREATED, Json(body)))
}

pub async fn bulk_post(
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
        match post_one(&st, company, id).await {
            Ok(_) => {
                done += 1;
                results.push(json!({ "id": id, "ok": 1 }));
            }
            Err(e) => {
                failed += 1;
                results.push(json!({ "id": id, "ok": 0, "error": err_text(&e) }));
            }
        }
    }
    Ok(Json(bulk_body("posted", done, failed, results)))
}

pub async fn bulk_unpost(
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
        match unpost_one(&st, company, id).await {
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
    Ok(Json(bulk_body("unposted", done, failed, results)))
}
