//! Име от сканиран документ към наш артикул. Търсенето по име е в приложението.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{self, cell_i32_at, cell_str, lit, Db};
use crate::dec;
use crate::form::{self, auth_sub, db_err};
use crate::invoice_money::{cents, money};
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

pub async fn company_vat_registered(db: &Db, company_id: i32) -> Result<bool, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT is_vat_registered, vat_number FROM companies WHERE id = {company_id}"
        ))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Ok(false);
    };
    let flag = cell_i32_at(row, 0);
    let vat = cell_str(row, 1);
    Ok(flag == 1 && !vat.trim().is_empty())
}

fn str_v(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

/// При нерегистрирана по ДДС фирма и входяща фактура разпределя ДДС-то
/// директно в стойността на артикулите (net_amount = total_amount, vat_rate = 0),
/// без да остава разлика в стотинките спрямо общата сума на документа.
pub fn absorb_vat_into_lines(lines: &mut [Value], doc_net_str: &str, doc_vat_str: &str, doc_total_str: &str) -> (String, String) {
    if lines.is_empty() {
        let tot_c = cents(doc_total_str).unwrap_or(0);
        let net_c = cents(doc_net_str).unwrap_or(0);
        let vat_c = cents(doc_vat_str).unwrap_or(0);
        let fin = if tot_c != 0 { tot_c } else { net_c + vat_c };
        let m = money(fin);
        return (m.clone(), m);
    }
    let mut nets = Vec::with_capacity(lines.len());
    let mut gross = Vec::with_capacity(lines.len());
    for line in lines.iter() {
        let q = dec::or0(&str_v(line, "quantity"));
        let q = if q == 0 { 1_000_000 } else { q };
        let up = dec::or0(&str_v(line, "unit_price"));
        let rate_pct = dec::or0(&str_v(line, "vat_rate"));
        let net_s = str_v(line, "net_amount");
        let vat_s = str_v(line, "vat_amount");
        let tot_s = str_v(line, "total_amount");
        let net_c = if !net_s.is_empty() {
            cents(&net_s).unwrap_or(0)
        } else {
            let n_dec = dec::mul(q, up).unwrap_or(0);
            cents(&dec::money(n_dec)).unwrap_or(0)
        };
        let vat_c = if !vat_s.is_empty() {
            cents(&vat_s).unwrap_or(0)
        } else if !tot_s.is_empty() && cents(&tot_s).unwrap_or(0) != 0 && net_c != 0 {
            cents(&tot_s).unwrap_or(0) - net_c
        } else if rate_pct > 0 {
            let v_dec = (i128::from(net_c) * rate_pct + 50_000_000) / 100_000_000;
            v_dec as i64
        } else {
            0
        };
        let tot_c = if !tot_s.is_empty() && cents(&tot_s).unwrap_or(0) != 0 {
            cents(&tot_s).unwrap_or(0)
        } else {
            net_c + vat_c
        };
        nets.push(net_c);
        gross.push(tot_c);
    }
    let doc_tot_c = cents(doc_total_str).unwrap_or(0);
    let doc_net_c = cents(doc_net_str).unwrap_or(0);
    let doc_vat_c = cents(doc_vat_str).unwrap_or(0);
    let sum_net: i64 = nets.iter().sum();
    let mut sum_gross: i64 = gross.iter().sum();
    let target = if doc_tot_c != 0 {
        doc_tot_c
    } else if doc_net_c != 0 || doc_vat_c != 0 {
        doc_net_c + doc_vat_c
    } else {
        sum_gross
    };
    if sum_gross == sum_net && target > sum_net && sum_net != 0 {
        let extra = target - sum_net;
        let mut acc = 0i64;
        let n = gross.len();
        for i in 0..n {
            let share = if i + 1 == n {
                extra - acc
            } else {
                let s = (i128::from(extra) * i128::from(nets[i]) + i128::from(sum_net.abs()) / 2)
                    / i128::from(sum_net);
                acc += s as i64;
                s as i64
            };
            gross[i] = nets[i] + share;
        }
        sum_gross = gross.iter().sum();
    }
    if target != 0 && sum_gross != target {
        let diff = target - sum_gross;
        if let Some(last) = gross.last_mut() {
            if *last + diff >= 0 {
                *last += diff;
            }
        }
    }
    let final_sum: i64 = gross.iter().sum();
    for (line, &g) in lines.iter_mut().zip(gross.iter()) {
        let g_str = money(g);
        let q = dec::or0(&str_v(line, "quantity"));
        let g_dec = dec::or0(&g_str);
        let up_str = if q > 0 {
            dec::money(dec::div(g_dec, q).unwrap_or(g_dec))
        } else {
            g_str.clone()
        };
        line["unit_price"] = json!(up_str);
        line["vat_rate"] = json!("0");
        line["net_amount"] = json!(g_str);
        line["vat_amount"] = json!("0.00");
        line["total_amount"] = json!(g_str);
    }
    let fin_str = money(final_sum);
    (fin_str.clone(), fin_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorbs_vat_without_rounding_drift() {
        let mut lines = vec![
            json!({
                "description": "Артикул 1",
                "quantity": "3",
                "unit": "C62",
                "unit_price": "10.00",
                "vat_rate": "20",
                "net_amount": "30.00",
                "vat_amount": "6.00",
                "total_amount": "36.00"
            }),
            json!({
                "description": "Артикул 2",
                "quantity": "3",
                "unit": "C62",
                "unit_price": "3.33",
                "vat_rate": "20",
                "net_amount": "9.99",
                "vat_amount": "2.00",
                "total_amount": "11.99"
            }),
        ];
        let (net, tot) = absorb_vat_into_lines(&mut lines, "39.99", "8.00", "47.99");
        assert_eq!(net, "47.99");
        assert_eq!(tot, "47.99");
        assert_eq!(lines[0]["net_amount"], "36.00");
        assert_eq!(lines[0]["vat_rate"], "0");
        assert_eq!(lines[0]["vat_amount"], "0.00");
        assert_eq!(lines[1]["net_amount"], "11.99");
        assert_eq!(lines[1]["total_amount"], "11.99");
        assert_eq!(lines[1]["unit_price"], "4.00");
    }

    #[test]
    fn distributes_header_vat_across_net_only_lines() {
        let mut lines = vec![
            json!({
                "description": "Стока А",
                "quantity": "1",
                "unit": "C62",
                "unit_price": "33.33",
                "vat_rate": "0",
                "net_amount": "33.33",
                "vat_amount": "",
                "total_amount": "33.33"
            }),
            json!({
                "description": "Стока Б",
                "quantity": "1",
                "unit": "C62",
                "unit_price": "33.33",
                "vat_rate": "0",
                "net_amount": "33.33",
                "vat_amount": "",
                "total_amount": "33.33"
            }),
            json!({
                "description": "Стока В",
                "quantity": "1",
                "unit": "C62",
                "unit_price": "33.34",
                "vat_rate": "0",
                "net_amount": "33.34",
                "vat_amount": "",
                "total_amount": "33.34"
            }),
        ];
        let (net, tot) = absorb_vat_into_lines(&mut lines, "100.00", "20.00", "120.00");
        assert_eq!(net, "120.00");
        assert_eq!(tot, "120.00");
        let s: i64 = lines
            .iter()
            .map(|l| cents(l["net_amount"].as_str().unwrap()).unwrap())
            .sum();
        assert_eq!(s, 12000);
    }
}
