//! Издаване на Протокол по чл. 117 от ЗДДС (09) и Митническа декларация (07)
//! към входяща фактура без директно отразяване в ДДС дневниците.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{Map, Value};

use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_lines::{self, NewLine};
use crate::invoice_money;
use crate::invoice_read;
use crate::invoices::{self, Head};
use crate::{api_err, AppState, ApiError};

fn vat_of(net_cents: i64, rate_cents: i64) -> i64 {
    let prod = net_cents as i128 * rate_cents as i128;
    if prod >= 0 {
        ((prod + 5_000) / 10_000) as i64
    } else {
        ((prod - 5_000) / 10_000) as i64
    }
}

fn opt_str(obj: &Map<String, Value>, key: &str, default: &str) -> Result<String, ApiError> {
    Ok(form::str_field(obj, key)?.unwrap_or_else(|| default.to_string()))
}

fn month_of(issue_date: &str, raw: &str) -> String {
    let t = raw.trim();
    if !t.is_empty() {
        return t.to_string();
    }
    if issue_date.len() >= 7 {
        issue_date[..7].to_string()
    } else {
        issue_date.to_string()
    }
}

pub async fn issue_protocol(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    body: Option<Json<Value>>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let inv = invoice_read::load(&st.db, id).await?;
    invoice_read::belongs(&inv, company)?;
    if inv.direction != "in" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "протокол по чл. 117 се издава само към входяща фактура",
        ));
    }
    let empty = Map::new();
    let obj = body
        .as_ref()
        .and_then(|Json(v)| v.as_object())
        .unwrap_or(&empty);
    let article = {
        let a = opt_str(obj, "vat_article", "чл. 117")?;
        if a.trim().is_empty() {
            "чл. 117".into()
        } else {
            a.trim().to_string()
        }
    };
    let rate_raw = {
        let r = opt_str(obj, "vat_rate", "20")?;
        if r.trim().is_empty() {
            "20".into()
        } else {
            r.trim().to_string()
        }
    };
    let rate_c = invoice_money::cents(&rate_raw).unwrap_or(2000);
    let rate_str = invoice_money::money(rate_c);
    let issue_date = {
        let d = opt_str(obj, "issue_date", &inv.issue_date)?;
        if d.trim().is_empty() {
            inv.issue_date.clone()
        } else {
            d.trim().to_string()
        }
    };
    let number = {
        let n = opt_str(obj, "number", "")?;
        if n.trim().is_empty() {
            invoices::next_out(&st, company, "09").await?
        } else {
            n.trim().to_string()
        }
    };
    let src_lines = invoice_read::lines_of(&st.db, inv.id).await?;
    let mut new_lines = Vec::new();
    if src_lines.is_empty() {
        let net_c = invoice_money::cents(&inv.net_amount).unwrap_or(0);
        let vat_c = vat_of(net_c, rate_c);
        let net_s = invoice_money::money(net_c);
        let vat_s = invoice_money::money(vat_c);
        let tot_s = invoice_money::money(net_c + vat_c);
        new_lines.push(NewLine {
            product_id: 0,
            code: String::new(),
            description: format!(
                "Протокол по {article} от ЗДДС към фактура № {}",
                inv.number
            ),
            quantity: "1".into(),
            unit: "C62".into(),
            unit_price: net_s.clone(),
            vat_rate: rate_str,
            net_amount: net_s,
            vat_amount: vat_s,
            total_amount: tot_s,
        });
    } else {
        for l in &src_lines {
            let net_c = invoice_money::cents(&l.net_amount).unwrap_or(0);
            let r_c = {
                let lc = invoice_money::cents(&l.vat_rate).unwrap_or(0);
                if lc > 0 { lc } else { rate_c }
            };
            let vat_c = vat_of(net_c, r_c);
            new_lines.push(NewLine {
                product_id: l.product_id,
                code: l.code.clone(),
                description: l.description.clone(),
                quantity: l.quantity.clone(),
                unit: l.unit.clone(),
                unit_price: l.unit_price.clone(),
                vat_rate: invoice_money::money(r_c),
                net_amount: invoice_money::money(net_c),
                vat_amount: invoice_money::money(vat_c),
                total_amount: invoice_money::money(net_c + vat_c),
            });
        }
    }
    let notes = format!(
        "Протокол по {article} от ЗДДС към входяща фактура № {} от {}",
        inv.number, inv.issue_date
    );
    let head = Head {
        direction: "in".into(),
        document_type: "09".into(),
        number,
        issue_date: issue_date.clone(),
        due_date: String::new(),
        tax_event_date: issue_date.clone(),
        accounting_month: month_of(&issue_date, &inv.accounting_month),
        counterpart_id: inv.counterpart_id,
        currency: inv.currency.clone(),
        currency_rate: inv.currency_rate.clone(),
        payment_method: String::new(),
        notes,
        discount_percent: "0".into(),
        discount_amount: "0".into(),
        vat_exemption_reason: String::new(),
        original_invoice_id: inv.id,
        location_id: inv.location_id,
        exclude_vat: 0,
        vat_article: article,
        customs_statistical: String::new(),
        customs_duty: String::new(),
        customs_vat: String::new(),
    };
    let name = invoice_lines::prepare(&st, company, &head, 0).await?;
    let new_id = invoices::insert_header(&st, company, &head, &name, &new_lines).await?;
    Ok((
        StatusCode::CREATED,
        Json(invoice_read::detail(&st, &sub, new_id).await?),
    ))
}

pub async fn issue_customs(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let inv = invoice_read::load(&st.db, id).await?;
    invoice_read::belongs(&inv, company)?;
    if inv.direction != "in" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "митническа декларация се издава само към входяща фактура",
        ));
    }
    let obj = form::object(&body)?;
    let number = opt_str(obj, "number", "")?.trim().to_string();
    if number.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "номерът на митническата декларация (MRN) е задължителен",
        ));
    }
    let issue_date = {
        let d = opt_str(obj, "issue_date", &inv.issue_date)?;
        if d.trim().is_empty() {
            inv.issue_date.clone()
        } else {
            d.trim().to_string()
        }
    };
    let stat_raw = opt_str(obj, "customs_statistical", &inv.net_amount)?;
    let duty_raw = opt_str(obj, "customs_duty", "0")?;
    let stat_c = invoice_money::cents(&stat_raw).ok_or_else(|| {
        api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "невалидна статистическа стойност",
        )
    })?;
    let duty_c = invoice_money::cents(&duty_raw)
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидно мито"))?;
    let vat_raw = opt_str(obj, "customs_vat", "")?;
    let vat_c = if vat_raw.trim().is_empty() {
        vat_of(stat_c + duty_c, 2000)
    } else {
        invoice_money::cents(&vat_raw).ok_or_else(|| {
            api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "невалидно митническо ДДС",
            )
        })?
    };
    let base_c = stat_c + duty_c;
    let base_s = invoice_money::money(base_c);
    let vat_s = invoice_money::money(vat_c);
    let tot_s = invoice_money::money(base_c + vat_c);
    let src_lines = invoice_read::lines_of(&st.db, inv.id).await?;
    let first_prod = src_lines.first().map(|l| l.product_id).unwrap_or(0);
    let line = NewLine {
        product_id: first_prod,
        code: String::new(),
        description: format!(
            "Митническа декларация № {number} към фактура № {} (Стат. ст-ст {}, Мито {})",
            inv.number,
            invoice_money::money(stat_c),
            invoice_money::money(duty_c)
        ),
        quantity: "1".into(),
        unit: "C62".into(),
        unit_price: base_s.clone(),
        vat_rate: "20.00".into(),
        net_amount: base_s,
        vat_amount: vat_s.clone(),
        total_amount: tot_s,
    };
    let notes = format!(
        "Митническа декларация № {number} към фактура № {} ({}) — Разчети с митници",
        inv.number, inv.counterpart_name
    );
    let head = Head {
        direction: "in".into(),
        document_type: "07".into(),
        number,
        issue_date: issue_date.clone(),
        due_date: String::new(),
        tax_event_date: issue_date.clone(),
        accounting_month: month_of(&issue_date, &inv.accounting_month),
        counterpart_id: 0,
        currency: "EUR".into(),
        currency_rate: "1".into(),
        payment_method: String::new(),
        notes,
        discount_percent: "0".into(),
        discount_amount: "0".into(),
        vat_exemption_reason: String::new(),
        original_invoice_id: inv.id,
        location_id: inv.location_id,
        exclude_vat: 0,
        vat_article: String::new(),
        customs_statistical: invoice_money::money(stat_c),
        customs_duty: invoice_money::money(duty_c),
        customs_vat: vat_s,
    };
    let name = invoice_lines::prepare(&st, company, &head, 0).await?;
    let new_id = invoices::insert_header(&st, company, &head, &name, &[line]).await?;
    Ok((
        StatusCode::CREATED,
        Json(invoice_read::detail(&st, &sub, new_id).await?),
    ))
}
