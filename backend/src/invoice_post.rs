//! Осчетоводяване. Продажба: Дт 411 / Кт приходната сметка на артикула / Кт 4532.
//! Покупка: Дт сметката за покупка на артикула / Дт 4531 / Кт 401.
//! Себестойността не се пише тук. Складът се сглобява наново след записа.
//! Кредитното известие пази същите страни и слага минус.
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
use crate::stock_replay;
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
    name: String,
    revenue_account_id: i32,
    inventory_account_id: i32,
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

async fn acct_id(st: &AppState, id: i32, company: i32) -> Result<Acct, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, number, name FROM accounts WHERE id = {id} AND company_id = {company}"
        ),
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



async fn need_product(st: &AppState, id: i32, company: i32) -> Result<Prod, ApiError> {
    match product(st, id, company).await? {
        Some(p) => Ok(p),
        None => Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "ред без артикул")),
    }
}

async fn product(st: &AppState, id: i32, company: i32) -> Result<Option<Prod>, ApiError> {
    if id <= 0 {
        return Ok(None);
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT name, revenue_account_id, inventory_account_id FROM products WHERE id = {id} AND company_id = {company}"
        ),
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(Prod {
        name: cell_str(&row, 0),
        revenue_account_id: cell_i32_at(&row, 1),
        inventory_account_id: cell_i32_at(&row, 2),
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

async fn acct_or_create(
    st: &AppState,
    company: i32,
    number: &str,
    default_name: &str,
) -> Result<Acct, ApiError> {
    if let Ok(a) = acct_number(st, company, number).await {
        return Ok(a);
    }
    let id = invoice_read::nid(&st.db, "accounts").await?;
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO accounts (id, company_id, number, name, saft_account_type, analytic_type) VALUES ({id}, {company}, {}, {}, {}, 'none')",
            lit(number),
            lit(default_name),
            lit(number)
        ),
    )
    .await?;
    Ok(Acct {
        id,
        number: number.to_string(),
        name: default_name.to_string(),
    })
}

async fn duty_expense_acct(st: &AppState, company: i32, inv: &Inv) -> Result<Acct, ApiError> {
    if inv.original_invoice_id > 0 {
        let orig_lines = invoice_read::lines_of(&st.db, inv.original_invoice_id).await?;
        for l in &orig_lines {
            if let Some(p) = product(st, l.product_id, company).await? {
                if p.inventory_account_id > 0 {
                    if let Ok(a) = acct_id(st, p.inventory_account_id, company).await {
                        return Ok(a);
                    }
                }
            }
        }
    }
    let lines = invoice_read::lines_of(&st.db, inv.id).await?;
    for l in &lines {
        if let Some(p) = product(st, l.product_id, company).await? {
            if p.inventory_account_id > 0 {
                if let Ok(a) = acct_id(st, p.inventory_account_id, company).await {
                    return Ok(a);
                }
            }
        }
    }
    for num in ["304", "302", "609", "602"] {
        if let Ok(a) = acct_number(st, company, num).await {
            return Ok(a);
        }
    }
    acct_or_create(st, company, "609", "Други разходи").await
}

async fn plan_for(st: &AppState, inv: &Inv) -> Result<Vec<Jl>, ApiError> {
    let company = inv.company_id;
    let total = amount_for_doc(&inv.document_type, &inv.total_amount);
    let vat = amount_for_doc(&inv.document_type, &inv.vat_amount);
    let vat_cents = invoice_money::cents(&inv.vat_amount).unwrap_or(0);
    let mut plan = Vec::new();

    if inv.direction == "in" && inv.document_type == "09" {
        if vat_cents == 0 {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "протоколът по чл. 117 трябва да има начислен ДДС",
            ));
        }
        let a4531 = acct_or_create(
            st,
            company,
            "4531",
            "Начислен данък върху добавената стойност за покупките",
        )
        .await?;
        let a4532 = acct_or_create(
            st,
            company,
            "4532",
            "Начислен данък върху добавената стойност за продажбите",
        )
        .await?;
        push(&mut plan, a4531, "debit", vat.clone(), &vat);
        push(&mut plan, a4532, "credit", vat.clone(), &vat);
        return Ok(plan);
    }

    if inv.direction == "in" && inv.document_type == "07" {
        let a458 = acct_or_create(st, company, "458", "Разчети с митници").await?;
        let a4531 = acct_or_create(
            st,
            company,
            "4531",
            "Начислен данък върху добавената стойност за покупките",
        )
        .await?;
        let duty_c = invoice_money::cents(&inv.customs_duty).unwrap_or(0);
        let cvat_c = {
            let c = invoice_money::cents(&inv.customs_vat).unwrap_or(0);
            if c > 0 { c } else { vat_cents }
        };
        if duty_c == 0 && cvat_c == 0 {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "митническата декларация трябва да съдържа мито или ДДС",
            ));
        }
        if duty_c > 0 {
            let duty_str = invoice_money::money(duty_c);
            let exp_acct = duty_expense_acct(st, company, inv).await?;
            push(&mut plan, exp_acct, "debit", duty_str, "0");
        }
        if cvat_c > 0 {
            let vat_str = invoice_money::money(cvat_c);
            push(&mut plan, a4531, "debit", vat_str.clone(), &vat_str);
        }
        let customs_total = invoice_money::money(duty_c + cvat_c);
        push(&mut plan, a458, "credit", customs_total, "0");
        return Ok(plan);
    }

    let a411 = acct_number(st, company, "411").await?;
    let a401 = acct_number(st, company, "401").await?;
    let a4531 = acct_number(st, company, "4531").await?;
    let a4532 = acct_number(st, company, "4532").await?;
    let lines = invoice_read::lines_of(&st.db, inv.id).await?;
    if inv.direction == "out" {
        push(&mut plan, a411, "debit", total, "0");
        for line in &lines {
            let net = amount_for_doc(&inv.document_type, &line.net_amount);
            let prod = need_product(st, line.product_id, company).await?;
            if prod.revenue_account_id <= 0 {
                return Err(api_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("артикулът {} няма приходна сметка", prod.name),
                ));
            }
            let acct = acct_id(st, prod.revenue_account_id, company).await?;
            push(&mut plan, acct, "credit", net, "0");
        }
        if vat_cents != 0 {
            push(&mut plan, a4532, "credit", vat.clone(), &vat);
        }
    } else {
        for line in &lines {
            let net = amount_for_doc(&inv.document_type, &line.net_amount);
            let prod = need_product(st, line.product_id, company).await?;
            if prod.inventory_account_id <= 0 {
                return Err(api_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("артикулът {} няма сметка за покупка", prod.name),
                ));
            }
            let acct = acct_id(st, prod.inventory_account_id, company).await?;
            push(&mut plan, acct, "debit", net, "0");
        }
        if inv.exclude_vat == 0 && vat_cents != 0 {
            push(&mut plan, a4531, "debit", vat.clone(), &vat);
        }
        let credit_amt = if inv.exclude_vat != 0 {
            amount_for_doc(&inv.document_type, &inv.net_amount)
        } else {
            total
        };
        push(&mut plan, a401, "credit", credit_amt, "0");
    }
    Ok(plan)
}

async fn write_journal(st: &AppState, inv: &Inv, plan: &[Jl]) -> Result<Value, ApiError> {
    let vat_type = if inv.exclude_vat != 0 {
        "no_vat"
    } else if inv.direction == "in" && inv.document_type == "09" {
        "both"
    } else if inv.direction == "out" {
        "sales"
    } else {
        "purchase"
    };
    let doc_label = if inv.document_type == "09" && !inv.vat_article.trim().is_empty() {
        format!("Протокол по {} от ЗДДС", inv.vat_article.trim())
    } else {
        invoice_money::title(&inv.document_type).to_string()
    };
    let desc = format!("{doc_label} {} / {}", inv.number, inv.direction);
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
    periods::assert_open(&st.db, company, &inv.accounting_month).await?;
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
    let plan = plan_for(st, &inv).await?;
    let body = write_journal(st, &inv, &plan).await?;
    stock_replay::rebuild(&st.db, company, "9999-12-31").await?;
    Ok(body)
}

pub(crate) async fn unpost_one(st: &AppState, company: i32, id: i32) -> Result<(), ApiError> {
    let inv = invoice_read::load(&st.db, id).await?;
    invoice_read::belongs(&inv, company)?;
    if inv.status != "posted" {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "не е осчетоводена"));
    }
    let ids = invoice_read::journal_ids(&st.db, &inv).await?;
    for entry_id in ids {
        invoice_read::wipe_journal(&st.db, entry_id).await?;
    }
    invoice_read::set_status(&st.db, id, "draft", 0).await?;
    stock_replay::rebuild(&st.db, company, "9999-12-31").await?;
    Ok(())
}

pub async fn post(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let body = post_one(&st, company, id).await?;
    if let Ok(inv) = invoice_read::load(&st.db, id).await {
        let tot = format!("{} {}", inv.total_amount, inv.currency);
        let jid = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let summary = format!(
            "Осчетоводен документ ({}) № {} — {}, сума {tot} (статия #{jid})",
            invoice_money::title(&inv.document_type),
            inv.number,
            inv.counterpart_name
        );
        crate::audit::record(
            &st.db,
            company,
            &sub,
            "invoice",
            id,
            &inv.number,
            "post",
            &summary,
            &tot,
            &tot,
            None,
            Some(&body),
        )
        .await;
        if jid > 0 {
            crate::audit::record(
                &st.db,
                company,
                &sub,
                "journal",
                jid,
                &inv.number,
                "create",
                &summary,
                "",
                &tot,
                None,
                Some(&body),
            )
            .await;
        }
    }
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
            Ok(jbody) => {
                if let Ok(inv) = invoice_read::load(&st.db, id).await {
                    let tot = format!("{} {}", inv.total_amount, inv.currency);
                    let jid = jbody.get("id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let summary = format!(
                        "Осчетоводен документ ({}) № {} — {}, сума {tot} (статия #{jid})",
                        invoice_money::title(&inv.document_type),
                        inv.number,
                        inv.counterpart_name
                    );
                    crate::audit::record(
                        &st.db,
                        company,
                        &sub,
                        "invoice",
                        id,
                        &inv.number,
                        "post",
                        &summary,
                        &tot,
                        &tot,
                        None,
                        Some(&jbody),
                    )
                    .await;
                    if jid > 0 {
                        crate::audit::record(
                            &st.db,
                            company,
                            &sub,
                            "journal",
                            jid,
                            &inv.number,
                            "create",
                            &summary,
                            "",
                            &tot,
                            None,
                            Some(&jbody),
                        )
                        .await;
                    }
                }
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
        let before = invoice_read::load(&st.db, id).await.ok();
        match unpost_one(&st, company, id).await {
            Ok(()) => {
                if let Some(inv) = before {
                    let tot = format!("{} {}", inv.total_amount, inv.currency);
                    let summary = format!(
                        "Върнат в чернова (отпостнат) документ ({}) № {} — {}, сума {tot}",
                        invoice_money::title(&inv.document_type),
                        inv.number,
                        inv.counterpart_name
                    );
                    crate::audit::record(
                        &st.db,
                        company,
                        &sub,
                        "invoice",
                        id,
                        &inv.number,
                        "unpost",
                        &summary,
                        &tot,
                        &tot,
                        None,
                        None,
                    )
                    .await;
                }
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
