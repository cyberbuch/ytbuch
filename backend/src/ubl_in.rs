//! UBL XML към същия преглед като сканирането. Черновата се записва после с фактурата.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use base64::Engine;
use serde_json::{json, Value};

use crate::db::{cell_str, Db};
use crate::form::{self, active_or_one, auth_sub, db_err};
use crate::pnmap;
use crate::ubl_xml::{self, Draft, Party};
use crate::{api_err, AppState, ApiError};

fn clean_b64(raw: &str) -> String {
    let head: String = raw.chars().take(80).collect();
    let body = if head.contains("base64") {
        raw.find(',').map(|i| &raw[i + 1..]).unwrap_or(raw)
    } else {
        raw
    };
    body.chars().filter(|c| !c.is_whitespace()).collect()
}

fn decode_xml(raw: &str, b64: &str) -> Result<String, ApiError> {
    let mut xml = raw.trim().trim_start_matches('\u{feff}').to_string();
    if xml.is_empty() {
        if b64.trim().is_empty() {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "празен файл"));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(clean_b64(b64))
            .map_err(|_| api_err(StatusCode::UNPROCESSABLE_ENTITY, "Файлът не е XML."))?;
        if bytes.len() > 8_000_000 {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Файлът е над 8 МБ."));
        }
        xml = String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}').trim().to_string();
    }
    if xml.len() > 8_000_000 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Файлът е над 8 МБ."));
    }
    Ok(xml)
}

fn same(a: &str, b: &str) -> bool {
    let x = a.trim().to_ascii_lowercase();
    let y = b.trim().to_ascii_lowercase();
    !x.is_empty() && x == y
}

fn digits(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn is_us(party: &Party, vat: &str, eik: &str) -> bool {
    same(&party.vat, vat)
        || same(&party.eik, eik)
        || same(&party.eik, &digits(vat))
        || same(eik, &digits(&party.vat))
}

fn choose<'a>(doc: &'a Draft, direction: &str, vat: &str, eik: &str) -> (&'a Party, String) {
    let sup = is_us(&doc.supplier, vat, eik);
    let cus = is_us(&doc.customer, vat, eik);
    if sup && !cus {
        return (&doc.customer, "out".into());
    }
    if cus && !sup {
        return (&doc.supplier, "in".into());
    }
    if direction == "out" {
        (&doc.customer, "out".into())
    } else {
        (&doc.supplier, direction.to_string())
    }
}

async fn company_ids(db: &Db, id: i32) -> Result<(String, String), ApiError> {
    let rows = db
        .query(&format!("SELECT vat_number, eik FROM companies WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Ok((String::new(), String::new()));
    };
    Ok((cell_str(row, 0), cell_str(row, 1)))
}

pub async fn import(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let filename = form::str_field(obj, "filename")?
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "invoice.xml".into());
    let direction = form::str_field(obj, "direction")?.unwrap_or_else(|| "in".into());
    let direction = direction.trim();
    if direction != "in" && direction != "out" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "direction must be 'in' or 'out'",
        ));
    }
    let xml_in = form::str_field(obj, "xml")?.unwrap_or_default();
    let b64 = form::str_field(obj, "content_base64")?.unwrap_or_default();
    let xml = decode_xml(&xml_in, &b64)?;
    let doc = ubl_xml::parse(&xml).map_err(|e| api_err(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    let company = active_or_one(&st, &sub).await?;
    let (cvat, ceik) = company_ids(&st.db, company).await?;
    let (party, direction) = choose(&doc, direction, &cvat, &ceik);
    let party_name = party.name.clone();
    let party_eik = party.eik.clone();
    let party_vat = party.vat.clone();
    let hit = pnmap::counterpart_hit(&st.db, company, &party_vat, &party_eik).await?;
    let mut lines = Vec::new();
    for line in &doc.lines {
        let mut pid = 0;
        if let Some(cp) = &hit {
            pid = pnmap::product_for(&st.db, cp.id, &line.name, &line.seller).await?;
        }
        if pid == 0 {
            pid = pnmap::product_by_code(&st.db, company, &line.seller).await?;
        }
        let mut item = json!({
            "description": line.name,
            "seller_code": line.seller,
            "quantity": if line.qty.is_empty() { "1" } else { line.qty.as_str() },
            "unit": if line.unit.is_empty() { "C62" } else { line.unit.as_str() },
            "unit_price": if line.price.is_empty() { "0" } else { line.price.as_str() },
            "vat_rate": if line.pct.is_empty() { "20" } else { line.pct.as_str() },
            "net_amount": line.net,
            "vat_amount": "",
            "total_amount": "",
        });
        if pid > 0 {
            item["product_id"] = json!(pid);
        }
        lines.push(item);
    }
    let original_id = pnmap::invoice_by_number(&st.db, company, &doc.original).await?;
    let mut out = json!({
        "direction": direction,
        "filename": filename,
        "invoice_number": doc.number,
        "issue_date": doc.issue_date,
        "due_date": doc.due_date,
        "tax_event_date": if doc.tax_point.is_empty() { doc.issue_date.clone() } else { doc.tax_point.clone() },
        "document_type": ubl_xml::document_type(&doc.root, &doc.type_code),
        "counterpart_name": party_name,
        "counterpart_eik": party_eik,
        "counterpart_vat_number": party_vat,
        "currency": if doc.currency.is_empty() { "EUR".into() } else { doc.currency.clone() },
        "currency_rate": if doc.currency_rate.is_empty() { "1".into() } else { doc.currency_rate.clone() },
        "payment_method": ubl_xml::pay_method(&doc.pay_code),
        "payment_note": doc.note,
        "vat_exemption_reason": ubl_xml::exemption(&doc.exemption, &doc.tax_cat),
        "original_invoice_number": doc.original,
        "total_net_amount": doc.net,
        "total_vat_amount": doc.vat,
        "total_amount": doc.total,
        "lines": lines,
    });
    if original_id > 0 {
        out["original_invoice_id"] = json!(original_id);
    }
    if let Some(cp) = hit {
        out["counterpart_id"] = json!(cp.id);
        out["counterpart"] = json!({
            "id": cp.id,
            "name": cp.name,
            "eik": cp.eik,
            "vat_number": cp.vat,
        });
    }
    Ok(Json(out))
}
