//! Сканиране на фактура: Mistral OCR, после JSON за чернова.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::addr;
use crate::form::{self, active_or_one, auth_sub};
use crate::httpx;
use crate::pnmap;
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

fn mime_of(filename: &str, mime: &str) -> String {
    let m = mime.trim();
    if !m.is_empty() && m != "application/octet-stream" {
        return m.to_string();
    }
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        "application/pdf".into()
    } else if lower.ends_with(".png") {
        "image/png".into()
    } else if lower.ends_with(".webp") {
        "image/webp".into()
    } else {
        "image/jpeg".into()
    }
}

fn is_pdf(filename: &str, mime: &str) -> bool {
    mime == "application/pdf" || filename.to_ascii_lowercase().ends_with(".pdf")
}

fn str_field(v: &Value, key: &str, default: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn chat_content(body: &str) -> String {
    let Ok(doc) = serde_json::from_str::<Value>(body) else {
        return String::new();
    };
    doc.pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn ocr_text(body: &str) -> String {
    let Ok(doc) = serde_json::from_str::<Value>(body) else {
        return String::new();
    };
    let Some(pages) = doc.get("pages").and_then(|v| v.as_array()) else {
        return String::new();
    };
    pages
        .iter()
        .filter_map(|p| p.get("markdown").and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn ocr(key: &str, filename: &str, mime: &str, b64: &str) -> Result<String, ApiError> {
    let raw = clean_b64(b64);
    if raw.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "празен файл"));
    }
    let mt = mime_of(filename, mime);
    let data = format!("data:{mt};base64,{raw}");
    let document = if is_pdf(filename, &mt) {
        json!({ "type": "document_url", "document_url": data })
    } else {
        json!({ "type": "image_url", "image_url": data })
    };
    let body = json!({
        "model": "mistral-ocr-latest",
        "document": document,
        "include_image_base64": false
    });
    let (status, text) = httpx::post_json("https://api.mistral.ai/v1/ocr", key, &body, 120)
        .await
        .map_err(|_| {
            api_err(
                StatusCode::BAD_GATEWAY,
                "Mistral OCR: връзката към api.mistral.ai се скъса (TLS/таймаут). Опитай по-малък файл.",
            )
        })?;
    if status != 200 {
        return Err(api_err(
            StatusCode::BAD_GATEWAY,
            format!("Mistral OCR върна статус {status}"),
        ));
    }
    let markdown = ocr_text(&text);
    if markdown.is_empty() {
        return Err(api_err(
            StatusCode::BAD_GATEWAY,
            "OCR не върна текст от документа",
        ));
    }
    Ok(markdown)
}

fn prompt(direction: &str) -> String {
    let who = if direction == "in" {
        "This is a PURCHASE: counterpart is the supplier (issuer of the invoice)."
    } else {
        "This is a SALE: counterpart is the customer (recipient), not our company."
    };
    format!(
        "Extract a Bulgarian invoice as JSON only. Keys: invoice_number, issue_date (YYYY-MM-DD), due_date, document_type (01 invoice, 02 debit note, 03 credit note), counterpart_name, counterpart_eik (digits only), counterpart_vat_number (with country prefix e.g. BG), currency, currency_rate, payment_method, payment_note, vat_exemption_reason, tax_event_date, original_invoice_number, total_net_amount, total_vat_amount, total_amount, lines (array of description, quantity, unit, unit_price, vat_rate, net_amount, vat_amount, total_amount). Amounts as strings with 2 decimals. unit_price is net without VAT. unit is UN/CEFACT (C62 pcs, KGM kg, LTR litre, MTR metre) else C62. vat_rate is 20, 9 or 0. Empty string if unknown. {who}"
    )
}

async fn extract_json(key: &str, direction: &str, text: &str) -> Result<Value, ApiError> {
    let body = json!({
        "model": "mistral-small-2603",
        "temperature": 0.1,
        "max_tokens": 4096,
        "response_format": {"type": "json_object"},
        "messages": [
            {"role": "system", "content": "You extract Bulgarian invoices. Return only JSON."},
            {"role": "user", "content": format!("{}\n\nOCR text:\n{text}", prompt(direction))}
        ]
    });
    let (status, raw) = httpx::post_json("https://api.mistral.ai/v1/chat/completions", key, &body, 60)
        .await
        .map_err(|_| api_err(StatusCode::BAD_GATEWAY, "Mistral chat е недостъпен (мрежова грешка)"))?;
    if status != 200 {
        return Err(api_err(
            StatusCode::BAD_GATEWAY,
            format!("Mistral chat върна статус {status}"),
        ));
    }
    let content = chat_content(&raw);
    let start = content.find('{').ok_or_else(|| {
        api_err(StatusCode::BAD_GATEWAY, "Mistral не върна JSON на фактурата")
    })?;
    let end = content.rfind('}').ok_or_else(|| {
        api_err(StatusCode::BAD_GATEWAY, "Mistral не върна JSON на фактурата")
    })?;
    serde_json::from_str(&content[start..=end])
        .map_err(|_| api_err(StatusCode::BAD_GATEWAY, "невалиден JSON от Mistral"))
}

pub async fn extract(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let filename = form::str_field(obj, "filename")?
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "document.pdf".into());
    let mime = form::str_field(obj, "mime")?.unwrap_or_default();
    let b64 = form::required(form::str_field(obj, "content_base64")?, "content_base64")?;
    let direction = form::str_field(obj, "direction")?.unwrap_or_else(|| "in".into());
    let direction = direction.trim();
    if direction != "in" && direction != "out" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "direction must be 'in' or 'out'",
        ));
    }
    let company = active_or_one(&st, &sub).await?;
    let key = addr::mistral_key(&st.db, company).await?;
    if key.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Няма Mistral ключ в настройките на фирмата (Настройки → Интеграции).",
        ));
    }
    let text = ocr(&key, &filename, &mime, &b64).await?;
    let doc = extract_json(&key, direction, &text).await?;
    let vat = str_field(&doc, "counterpart_vat_number", "");
    let eik = str_field(&doc, "counterpart_eik", "");
    let hit = pnmap::counterpart_hit(&st.db, company, &vat, &eik).await?;
    let raw_lines = doc.get("lines").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut lines = Vec::new();
    for line in raw_lines {
        let desc = str_field(&line, "description", "");
        let total = str_field(&line, "total_amount", "");
        let total = if total.is_empty() { str_field(&line, "total", "0") } else { total };
        let mut pid = 0;
        if let Some(cp) = &hit {
            pid = pnmap::product_for(&st.db, company, cp.id, &desc, "").await?;
        }
        let mut item = json!({
            "description": desc,
            "quantity": str_field(&line, "quantity", "1"),
            "unit": str_field(&line, "unit", "C62"),
            "unit_price": str_field(&line, "unit_price", "0"),
            "vat_rate": str_field(&line, "vat_rate", "20"),
            "net_amount": str_field(&line, "net_amount", ""),
            "vat_amount": str_field(&line, "vat_amount", ""),
            "total_amount": total,
        });
        if pid > 0 {
            item["product_id"] = json!(pid);
        }
        lines.push(item);
    }
    let original = str_field(&doc, "original_invoice_number", "");
    let original_id = pnmap::invoice_by_number(&st.db, company, &original).await?;
    let mut out = json!({
        "direction": direction,
        "filename": filename,
        "invoice_number": str_field(&doc, "invoice_number", ""),
        "issue_date": str_field(&doc, "issue_date", ""),
        "due_date": str_field(&doc, "due_date", ""),
        "tax_event_date": str_field(&doc, "tax_event_date", ""),
        "document_type": str_field(&doc, "document_type", "01"),
        "counterpart_name": str_field(&doc, "counterpart_name", ""),
        "counterpart_eik": eik,
        "counterpart_vat_number": vat,
        "currency": str_field(&doc, "currency", "EUR"),
        "currency_rate": str_field(&doc, "currency_rate", "1"),
        "payment_method": str_field(&doc, "payment_method", "банков превод"),
        "payment_note": str_field(&doc, "payment_note", ""),
        "vat_exemption_reason": str_field(&doc, "vat_exemption_reason", ""),
        "original_invoice_number": original,
        "total_net_amount": str_field(&doc, "total_net_amount", "0"),
        "total_vat_amount": str_field(&doc, "total_vat_amount", "0"),
        "total_amount": str_field(&doc, "total_amount", "0"),
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
