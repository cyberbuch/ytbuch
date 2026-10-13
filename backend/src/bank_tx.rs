//! Банкови транзакции: преглед, импорт, ръчно осчетоводяване и разнасяне.
//! Автоматичният запис е Дт ГЛ / Кт буфер при плюс и обратно при минус.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::bank_csv::{self, Tx};
use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, money};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_read;
use crate::pagination::{paginate, PageQuery};
use crate::periods;
use crate::posting::{self, Side};
use crate::{api_err, AppState, ApiError};

const UNKNOWN: &str = "Непознат формат на файла. Поддържаните формати са: OBB XML, ISO camt.053, PostBank XML, MT-940, CSV.";

#[derive(Deserialize, Default)]
pub(crate) struct TxQ {
    bank_account_id: Option<String>,
    status: Option<String>,
    #[serde(flatten)]
    page: PageQuery,
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<TxQ>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let account = q.bank_account_id.unwrap_or_default().trim().parse::<i32>().unwrap_or(0);
    let sql = if account > 0 {
        format!("SELECT id, bank_account_id, txn_date, amount, currency, counterpart_name, counterpart_iban, description, reference, transaction_type, is_booked, is_allocated, journal_entry_id, company_id FROM bank_transactions WHERE company_id = {company} AND bank_account_id = {account} ORDER BY id")
    } else {
        format!("SELECT id, bank_account_id, txn_date, amount, currency, counterpart_name, counterpart_iban, description, reference, transaction_type, is_booked, is_allocated, journal_entry_id, company_id FROM bank_transactions WHERE company_id = {company} ORDER BY id")
    };
    let rows = invoice_read::qry(&st.db, &sql).await?;
    let status = q.status.unwrap_or_default();
    let mut items = Vec::new();
    for row in &rows {
        let booked = cell_i32_at(row, 10) != 0;
        let allocated = cell_i32_at(row, 11) != 0;
        let keep = match status.as_str() {
            "new" => !booked,
            "booked" => booked && !allocated,
            "allocated" => allocated,
            _ => true,
        };
        if keep {
            items.push(tx_json(row));
        }
    }
    Ok(paginate(items, &q.page))
}

pub async fn preview(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let ba_id = req_i32(obj, "bank_account_id")?;
    let content = form::required(form::str_field(obj, "file_content")?, "file_content")?;
    let mut parsed = bank_csv::detect(&content).ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, UNKNOWN))?;
    bank(&st, ba_id, company).await?;
    let refs = refs_of(&st, ba_id).await?;
    let mut dup = 0;
    for tx in &mut parsed.txs {
        if !tx.reference.is_empty() && refs.iter().any(|r| r == &tx.reference) {
            tx.is_duplicate = true;
            dup += 1;
        }
    }
    Ok(Json(pack(&parsed, dup)))
}

pub async fn import(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let ba_id = req_i32(obj, "bank_account_id")?;
    let content = form::required(form::str_field(obj, "file_content")?, "file_content")?;
    let ba = bank(&st, ba_id, company).await?;
    let parsed = bank_csv::detect(&content).ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, UNKNOWN))?;
    let refs = refs_of(&st, ba_id).await?;
    let auto = ba.gl > 0 && ba.buf > 0;
    let mut imported = 0;
    let mut journal_count = 0;
    let mut made: Vec<(i32, i32)> = Vec::new();
    for tx in &parsed.txs {
        if !tx.reference.is_empty() && refs.iter().any(|r| r == &tx.reference) {
            continue;
        }
        let Some(amount) = dec::parse(&tx.amount) else { continue };
        let tx_type = if amount < 0 { "WITHDRAWAL" } else { "DEPOSIT" };
        if auto {
            if let Err(e) = periods::assert_open(&st.db, company, &tx.date).await {
                let _ = rollback(&st, &made).await;
                return Err(e);
            }
        }
        let id = match invoice_read::nid(&st.db, "bank_transactions").await {
            Ok(id) => id,
            Err(e) => { let _ = rollback(&st, &made).await; return Err(e); }
        };
        let currency = if tx.currency.is_empty() { ba.currency.clone() } else { tx.currency.clone() };
        if let Err(e) = invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO bank_transactions (id, company_id, bank_account_id, txn_date, amount, currency, counterpart_name, counterpart_iban, description, reference, transaction_type, is_booked, is_allocated, journal_entry_id) \
                 VALUES ({id}, {company}, {ba_id}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 0, 0)",
                lit(&tx.date), lit(&money(amount)), lit(&currency), lit(&tx.contra_name), lit(&tx.contra_iban),
                lit(&tx.description), lit(&tx.reference), lit(tx_type), i32::from(auto)
            ),
        )
        .await
        {
            let _ = rollback(&st, &made).await;
            return Err(e);
        }
        made.push((id, 0));
        imported += 1;
        if auto {
            match auto_journal(&st, company, &ba, tx, amount, tx_type).await {
                Ok(entry) => {
                    if let Err(e) = invoice_read::exe(&st.db, &format!("UPDATE bank_transactions SET journal_entry_id = {entry} WHERE id = {id}")).await {
                        let _ = rollback(&st, &made).await;
                        return Err(e);
                    }
                    if let Some(last) = made.last_mut() { last.1 = entry; }
                    journal_count += 1;
                }
                Err(e) => {
                    let _ = rollback(&st, &made).await;
                    return Err(e);
                }
            }
        }
    }
    Ok(Json(json!({ "imported": imported, "journal_count": journal_count })))
}

pub async fn book(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    need(id)?;
    let obj = form::object(&body)?;
    let cp = form::i32_field(obj, "counterpart_id")?.unwrap_or(0);
    let contra = form::i32_field(obj, "contra_account_id")?.unwrap_or(0);
    form::ensure_ref(&st.db, "counterparts", cp, company, "контрагентът").await?;
    form::ensure_ref(&st.db, "accounts", contra, company, "сметката").await?;
    let row = tx_row(&st, id, company).await?;
    if cell_i32_at(&row, 10) != 0 {
        return Err(api_err(StatusCode::CONFLICT, "транзакцията вече е осчетоводена"));
    }
    let amount = dec::parse(&cell_str(&row, 3)).ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна сума на транзакцията"))?;
    let deposit = amount >= 0;
    let abs_s = money(dec::abs(amount));
    let ba = bank(&st, cell_i32_at(&row, 1), company).await?;
    let (debit_id, credit_id) = if contra > 0 {
        if ba.gl <= 0 {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Банковата сметка няма свързана GL сметка"));
        }
        if deposit { (ba.gl, contra) } else { (contra, ba.gl) }
    } else {
        let d = form::i32_field(obj, "debit_account_id")?.ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "debit_account_id е задължително"))?;
        let c = form::i32_field(obj, "credit_account_id")?.ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "credit_account_id е задължително"))?;
        if d <= 0 || c <= 0 {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "debit_account_id е задължително"));
        }
        form::ensure_ref(&st.db, "accounts", d, company, "сметката").await?;
        form::ensure_ref(&st.db, "accounts", c, company, "сметката").await?;
        (d, c)
    };
    let (cp_name, _) = counterpart(&st, cp, company).await?;
    let tx_contra = cell_str(&row, 5);
    let shown = if cp > 0 { cp_name.clone() } else { tx_contra.clone() };
    periods::assert_open(&st.db, company, &cell_str(&row, 2)).await?;
    let mut desc = if cell_str(&row, 7).is_empty() { "Ръчно осчетоводяване".into() } else { cell_str(&row, 7) };
    if !tx_contra.is_empty() {
        desc = format!("{desc} - {tx_contra}");
    }
    let debit = posting::by_id(&st.db, debit_id).await?;
    let credit = posting::by_id(&st.db, credit_id).await?;
    let entry = posting::write_entry(
        &st.db, company, &cell_str(&row, 2), "bank", 0, &cell_str(&row, 8), &desc, cp, &shown,
        &posting::month_of(&cell_str(&row, 2)),
        &[
            Side { account: debit, direction: "debit", amount: abs_s.clone() },
            Side { account: credit, direction: "credit", amount: abs_s },
        ],
    )
    .await?;
    if let Err(e) = invoice_read::exe(
        &st.db,
        &format!("UPDATE bank_transactions SET is_booked = 1, is_allocated = 1, journal_entry_id = {entry} WHERE id = {id}"),
    )
    .await
    {
        let _ = invoice_read::wipe_journal(&st.db, entry).await;
        return Err(e);
    }
    let mut o = json!({ "message": "транзакцията е осчетоводена", "journal_entry_id": entry });
    if cp > 0 { o["counterpart_id"] = json!(cp); }
    Ok(Json(o))
}

pub async fn reallocate(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    need(id)?;
    let obj = form::object(&body)?;
    let new_acc = req_i32(obj, "account_id")?;
    let cp = form::i32_field(obj, "counterpart_id")?.unwrap_or(0);
    form::ensure_ref(&st.db, "accounts", new_acc, company, "сметката").await?;
    form::ensure_ref(&st.db, "counterparts", cp, company, "контрагентът").await?;
    let row = tx_row(&st, id, company).await?;
    let entry = cell_i32_at(&row, 12);
    if entry <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Транзакцията няма счетоводен запис"));
    }
    let ba = bank(&st, cell_i32_at(&row, 1), company).await?;
    if ba.buf <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Няма буферна сметка"));
    }
    let (cp_name, _) = counterpart(&st, cp, company).await?;
    let acct = posting::by_id(&st.db, new_acc).await?;
    let lines = invoice_read::qry(&st.db, &format!("SELECT id, account_id FROM journal_lines WHERE entry_id = {entry}")).await?;
    let mut replaced = 0;
    for line in &lines {
        if cell_i32_at(line, 1) == ba.buf {
            invoice_read::exe(
                &st.db,
                &format!(
                    "UPDATE journal_lines SET account_id = {}, account_number = {}, account_name = {} WHERE id = {}",
                    acct.id, lit(&acct.number), lit(&acct.name), cell_i32_at(line, 0)
                ),
            )
            .await?;
            replaced += 1;
        }
    }
    if replaced == 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Не е намерен ред с буферна сметка"));
    }
    if cp > 0 {
        invoice_read::exe(
            &st.db,
            &format!(
                "UPDATE journal_entries SET counterpart_id = {cp}, counterpart_name = {} WHERE id = {entry}",
                lit(&cp_name)
            ),
        )
        .await?;
    }
    invoice_read::exe(&st.db, &format!("UPDATE bank_transactions SET is_allocated = 1 WHERE id = {id}")).await?;
    let acc_name = if acct.name.is_empty() { acct.number.clone() } else { format!("{} {}", acct.number, acct.name) };
    Ok(Json(json!({
        "message": format!("Транзакцията е разнесена към {acc_name}"),
        "account_name": acc_name,
        "replaced_lines": replaced,
    })))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let company = company(&st, &headers).await?;
    need(id)?;
    let row = tx_row(&st, id, company).await?;
    let entry = cell_i32_at(&row, 12);
    if entry > 0 {
        invoice_read::wipe_journal(&st.db, entry).await?;
    }
    invoice_read::exe(&st.db, &format!("DELETE FROM bank_transactions WHERE id = {id}")).await?;
    Ok(StatusCode::NO_CONTENT)
}

struct Ba {
    name: String,
    currency: String,
    gl: i32,
    buf: i32,
}

async fn bank(st: &AppState, id: i32, company: i32) -> Result<Ba, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT name, currency, gl_account_id, buffer_account_id, company_id FROM bank_accounts WHERE id = {id}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Банковата сметка не е намерена"));
    };
    if cell_i32_at(row, 4) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "Банковата сметка не е намерена"));
    }
    Ok(Ba { name: cell_str(row, 0), currency: cell_str(row, 1), gl: cell_i32_at(row, 2), buf: cell_i32_at(row, 3) })
}

async fn auto_journal(st: &AppState, company: i32, ba: &Ba, tx: &Tx, amount: i128, tx_type: &str) -> Result<i32, ApiError> {
    let abs_s = money(dec::abs(amount));
    let (debit_id, credit_id) = if tx_type == "DEPOSIT" { (ba.gl, ba.buf) } else { (ba.buf, ba.gl) };
    let debit = posting::by_id(&st.db, debit_id).await?;
    let credit = posting::by_id(&st.db, credit_id).await?;
    let mut desc = if ba.name.is_empty() { "Банково извлечение".into() } else { format!("Банково извлечение {}", ba.name) };
    if !tx.description.is_empty() {
        desc = format!("{desc} — {}", tx.description);
    }
    posting::write_entry(
        &st.db, company, &tx.date, "bank", 0, &tx.reference, &desc, 0, &tx.contra_name,
        &posting::month_of(&tx.date),
        &[
            Side { account: debit, direction: "debit", amount: abs_s.clone() },
            Side { account: credit, direction: "credit", amount: abs_s },
        ],
    )
    .await
}

async fn rollback(st: &AppState, made: &[(i32, i32)]) -> Result<(), ApiError> {
    for (tx, entry) in made.iter().rev() {
        if *entry > 0 {
            invoice_read::wipe_journal(&st.db, *entry).await?;
        }
        invoice_read::exe(&st.db, &format!("DELETE FROM bank_transactions WHERE id = {tx}")).await?;
    }
    Ok(())
}

async fn refs_of(st: &AppState, account: i32) -> Result<Vec<String>, ApiError> {
    let rows = invoice_read::qry(&st.db, &format!("SELECT reference FROM bank_transactions WHERE bank_account_id = {account}")).await?;
    Ok(rows.iter().map(|r| cell_str(r, 0)).filter(|s| !s.is_empty()).collect())
}

fn pack(parsed: &bank_csv::Parsed, dup: i32) -> Value {
    let total = parsed.txs.len() as i32;
    json!({
        "format_name": parsed.format_name,
        "account_iban": parsed.account_iban,
        "account_currency": parsed.account_currency,
        "transactions": parsed.txs.iter().map(|tx| json!({
            "date": tx.date,
            "amount": tx.amount,
            "currency": tx.currency,
            "description": tx.description,
            "contra_name": tx.contra_name,
            "contra_iban": tx.contra_iban,
            "reference": tx.reference,
            "is_duplicate": tx.is_duplicate,
        })).collect::<Vec<_>>(),
        "total_count": total,
        "duplicate_count": dup,
        "new_count": total - dup,
    })
}

fn tx_json(row: &crate::db::Row) -> Value {
    json!({
        "id": cell_i32_at(row, 0),
        "bank_account_id": cell_i32_at(row, 1),
        "transaction_date": cell_str(row, 2),
        "amount": cell_str(row, 3),
        "currency": cell_str(row, 4),
        "counterpart_name": cell_str(row, 5),
        "counterpart_iban": cell_str(row, 6),
        "description": cell_str(row, 7),
        "reference": cell_str(row, 8),
        "transaction_type": cell_str(row, 9),
        "is_booked": cell_i32_at(row, 10) != 0,
        "is_allocated": cell_i32_at(row, 11) != 0,
        "journal_entry_id": cell_i32_at(row, 12),
        "company_id": cell_i32_at(row, 13),
    })
}

async fn tx_row(st: &AppState, id: i32, company: i32) -> Result<crate::db::Row, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id, bank_account_id, txn_date, amount, currency, counterpart_name, counterpart_iban, description, reference, transaction_type, is_booked, is_allocated, journal_entry_id, company_id FROM bank_transactions WHERE id = {id}"),
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Транзакцията не е намерена"));
    };
    if cell_i32_at(&row, 13) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "Транзакцията не е намерена"));
    }
    Ok(row)
}

async fn counterpart(st: &AppState, id: i32, company: i32) -> Result<(String, String), ApiError> {
    if id <= 0 {
        return Ok((String::new(), String::new()));
    }
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT name, vat_number FROM counterparts WHERE id = {id} AND company_id = {company}"
        ),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Контрагентът не е намерен"));
    };
    Ok((cell_str(row, 0), cell_str(row, 1)))
}

fn req_i32(obj: &serde_json::Map<String, Value>, key: &str) -> Result<i32, ApiError> {
    let v = form::i32_field(obj, key)?.ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("{key} е задължително")))?;
    if v <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("{key} е задължително")));
    }
    Ok(v)
}

fn need(id: i32) -> Result<(), ApiError> {
    if id <= 0 { Err(api_err(StatusCode::BAD_REQUEST, "invalid id")) } else { Ok(()) }
}

async fn company(st: &AppState, headers: &HeaderMap) -> Result<i32, ApiError> {
    let sub = auth_sub(st, headers)?;
    let id = active_or_one(st, &sub).await?;
    if id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "няма избрана активна фирма"));
    }
    Ok(id)
}
