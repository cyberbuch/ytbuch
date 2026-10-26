//! Фактура, редове, фирма и контрагент за печат и UBL.

use axum::http::{HeaderMap, StatusCode};
use serde_json::Value;

use crate::db::cell_str;
use crate::form::{active_or_one, auth_sub};
use crate::invoice_read::{self, Inv, Line};
use crate::{api_err, companies, counterparts, AppState, ApiError};

pub struct Loaded {
    pub inv: Inv,
    pub lines: Vec<Line>,
    pub company: Value,
    pub counterpart: Value,
    pub original_number: String,
    pub original_date: String,
}

#[derive(Clone)]
pub struct Party {
    pub name: String,
    pub eik: String,
    pub vat: String,
    pub address: String,
    pub city: String,
    pub post: String,
    pub country: String,
    pub email: String,
    pub phone: String,
    pub mol: String,
    pub contact: String,
    pub iban: String,
    pub bic: String,
}

pub async fn load(st: &AppState, headers: &HeaderMap, id: i32) -> Result<Loaded, ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid id"));
    }
    let sub = auth_sub(st, headers)?;
    let company_id = active_or_one(st, &sub).await?;
    let inv = invoice_read::load(&st.db, id).await?;
    invoice_read::belongs(&inv, company_id)?;
    let lines = invoice_read::lines_of(&st.db, inv.id).await?;
    let firm_id = if inv.company_id > 0 { inv.company_id } else { company_id };
    let company = companies::json_by_id(&st.db, firm_id).await?.unwrap_or(Value::Null);
    let counterpart = if inv.counterpart_id > 0 {
        counterparts::json_by_id(&st.db, inv.counterpart_id, firm_id).await?.unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let (original_number, original_date) = original(&st.db, company_id, inv.original_invoice_id).await?;
    Ok(Loaded { inv, lines, company, counterpart, original_number, original_date })
}

/// Оригиналната фактура е на същата фирма — иначе би изтекъл номер и дата
/// на чужда фирма.
async fn original(db: &crate::db::Db, company: i32, id: i32) -> Result<(String, String), ApiError> {
    if id <= 0 {
        return Ok((String::new(), String::new()));
    }
    let rows = invoice_read::qry(
        db,
        &format!("SELECT number, issue_date FROM invoices WHERE id = {id} AND company_id = {company}"),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok((String::new(), String::new()));
    };
    Ok((cell_str(row, 0), cell_str(row, 1)))
}

pub fn party(v: &Value) -> Party {
    Party {
        name: field(v, "name"),
        eik: field(v, "eik"),
        vat: field(v, "vat_number"),
        address: field(v, "address"),
        city: field(v, "city"),
        post: field(v, "post_code"),
        country: field(v, "country"),
        email: field(v, "email"),
        phone: field(v, "phone"),
        mol: field(v, "mol"),
        contact: field(v, "contact_person"),
        iban: field(v, "iban"),
        bic: field(v, "bic"),
    }
}

pub fn field(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").trim().to_string()
}
