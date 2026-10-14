//! Име от сканиран документ към наш артикул. Търсенето по име е в приложението.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{self, cell_i32_at, cell_str, lit, Db};
use crate::form::{self, auth_sub, db_err};
use crate::pagination::{paginate, PageQuery};
use crate::{api_err, AppState, ApiError};

#[derive(Deserialize)]
pub struct ListQuery {
    counterpart_id: Option<i32>,
    #[serde(flatten)]
    page: PageQuery,
}

pub fn norm(raw: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for ch in raw.trim().chars() {
        if ch.is_whitespace() {
            if !space && !out.is_empty() {
                out.push(' ');
                space = true;
            }
        } else {
            space = false;
            out.push(ch.to_lowercase().next().unwrap_or(ch));
        }
    }
    out.trim().to_string()
}

fn row_json(row: &crate::db::Row) -> Value {
    json!({
        "id": cell_i32_at(row, 0),
        "counterpart_id": cell_i32_at(row, 1),
        "scanned_name": cell_str(row, 2),
        "product_id": cell_i32_at(row, 3),
    })
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = form::active_or_one(&st, &sub).await?;
    let id = q.counterpart_id.unwrap_or(0);
    form::ensure_ref(&st.db, "counterparts", id, company, "контрагентът").await?;
    let sql = if id > 0 {
        format!(
            "SELECT id, counterpart_id, scanned_name, product_id FROM product_name_mappings WHERE company_id = {company} AND counterpart_id = {id}"
        )
    } else {
        format!("SELECT id, counterpart_id, scanned_name, product_id FROM product_name_mappings WHERE company_id = {company}")
    };
    let rows = st.db.query(&sql).await.map_err(db_err)?;
    let items: Vec<Value> = rows.iter().map(row_json).collect();
    Ok(paginate(items, &q.page))
}

pub async fn save(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = form::active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let counterpart = form::i32_field(obj, "counterpart_id")?.unwrap_or(0);
    if counterpart <= 0 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "counterpart_id е задължителен",
        ));
    }
    form::ensure_row(&st.db, "counterparts", counterpart, company, "контрагентът").await?;
    let items = obj.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let existing = st
        .db
        .query(&format!(
            "SELECT id, scanned_name FROM product_name_mappings WHERE counterpart_id = {counterpart} AND company_id = {company}"
        ))
        .await
        .map_err(db_err)?;
    let mut by_name: std::collections::HashMap<String, i32> = existing
        .iter()
        .map(|r| (cell_str(r, 1), cell_i32_at(r, 0)))
        .collect();
    let stamp = db::now_stamp();
    for item in items {
        let Some(row) = item.as_object() else { continue };
        let name = norm(&form::str_field(row, "scanned_name")?.unwrap_or_default());
        let product = form::i32_field(row, "product_id")?.unwrap_or(0);
        if name.is_empty() || product <= 0 {
            continue;
        }
        form::ensure_ref(&st.db, "products", product, company, "артикулът").await?;
        if let Some(id) = by_name.get(&name) {
            st.db
                .exec(&format!(
                    "UPDATE product_name_mappings SET product_id = {product} WHERE id = {id} AND company_id = {company}"
                ))
                .await
                .map_err(db_err)?;
            continue;
        }
        let id = st.db.next_id("product_name_mappings").await.map_err(db_err)?;
        st.db
            .exec(&format!(
                "INSERT INTO product_name_mappings (id, company_id, counterpart_id, scanned_name, product_id, created_at) \
                 VALUES ({id}, {company}, {counterpart}, {}, {product}, {})",
                lit(&name),
                lit(&stamp)
            ))
            .await
            .map_err(db_err)?;
        by_name.insert(name, id);
    }
    let rows = st
        .db
        .query(&format!(
            "SELECT id, counterpart_id, scanned_name, product_id FROM product_name_mappings WHERE counterpart_id = {counterpart} AND company_id = {company}"
        ))
        .await
        .map_err(db_err)?;
    let items: Vec<Value> = rows.iter().map(row_json).collect();
    Ok(paginate(items, &PageQuery::default()))
}

fn digits(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn same_id(a: &str, b: &str) -> bool {
    let x = a.trim().to_ascii_lowercase();
    let y = b.trim().to_ascii_lowercase();
    !x.is_empty() && x == y
}

fn same_eik(a: &str, b: &str) -> bool {
    !crate::counterparts::is_placeholder_eik(a)
        && !crate::counterparts::is_placeholder_eik(b)
        && same_id(a, b)
}

pub struct PartyHit {
    pub id: i32,
    pub name: String,
    pub eik: String,
    pub vat: String,
}

pub async fn counterpart_hit(db: &Db, company: i32, vat: &str, eik: &str) -> Result<Option<PartyHit>, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT id, name, eik, vat_number FROM counterparts WHERE company_id = {company}"
        ))
        .await
        .map_err(db_err)?;
    for row in &rows {
        let have_eik = cell_str(row, 2);
        let have_vat = cell_str(row, 3);
        let vat_ok = same_id(&have_vat, vat);
        let eik_ok = same_eik(&have_eik, eik)
            || same_eik(&have_eik, &digits(vat))
            || same_eik(eik, &digits(&have_vat));
        if vat_ok || eik_ok {
            return Ok(Some(PartyHit {
                id: cell_i32_at(row, 0),
                name: cell_str(row, 1),
                eik: have_eik,
                vat: have_vat,
            }));
        }
    }
    Ok(None)
}

pub async fn product_by_code(db: &Db, company: i32, code: &str) -> Result<i32, ApiError> {
    let want = code.trim().to_ascii_lowercase();
    if want.is_empty() {
        return Ok(0);
    }
    let rows = db
        .query(&format!("SELECT id, code FROM products WHERE company_id = {company}"))
        .await
        .map_err(db_err)?;
    for row in &rows {
        if cell_str(row, 1).trim().to_ascii_lowercase() == want {
            return Ok(cell_i32_at(row, 0));
        }
    }
    Ok(0)
}

pub async fn invoice_by_number(db: &Db, company: i32, number: &str) -> Result<i32, ApiError> {
    let want = number.trim();
    if want.is_empty() {
        return Ok(0);
    }
    let rows = db
        .query(&format!(
            "SELECT id, number FROM invoices WHERE company_id = {company}"
        ))
        .await
        .map_err(db_err)?;
    for row in &rows {
        if cell_str(row, 1).trim() == want {
            return Ok(cell_i32_at(row, 0));
        }
    }
    Ok(0)
}

pub async fn product_for(db: &Db, company: i32, counterpart_id: i32, name: &str, seller: &str) -> Result<i32, ApiError> {
    if counterpart_id <= 0 {
        return Ok(0);
    }
    let rows = db
        .query(&format!(
            "SELECT scanned_name, product_id FROM product_name_mappings WHERE counterpart_id = {counterpart_id} AND company_id = {company}"
        ))
        .await
        .map_err(db_err)?;
    let want = norm(name);
    let seller_n = norm(seller);
    for row in &rows {
        if !want.is_empty() && cell_str(row, 0) == want {
            return Ok(cell_i32_at(row, 1));
        }
    }
    if !seller_n.is_empty() {
        for row in &rows {
            if cell_str(row, 0) == seller_n {
                return Ok(cell_i32_at(row, 1));
            }
        }
    }
    Ok(0)
}
