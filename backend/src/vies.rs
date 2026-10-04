//! Справка във VIES и нов контрагент от нея.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::addr;
use crate::counterparts;
use crate::form::{self, active_or_one, auth_sub};
use crate::httpx;
use crate::{api_err, AppState, ApiError};

const EU: &[&str] = &[
    "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "EL", "HR", "HU", "IE",
    "IT", "LT", "LU", "LV", "MT", "NL", "PL", "PT", "RO", "SE", "SI", "SK", "XI", "GR",
];

#[derive(Deserialize)]
pub struct VatQuery {
    vat: Option<String>,
}

struct Hit {
    valid: bool,
    name: String,
    address: String,
    user_error: String,
    request_date: String,
}

fn normalize(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn shape_ok(vat: &str) -> bool {
    vat.len() >= 4 && vat.len() <= 15 && EU.contains(&&vat[..2])
}

fn api_cc(cc: &str) -> &str {
    if cc == "GR" { "EL" } else { cc }
}

async fn fetch(vat: &str) -> Result<Hit, ApiError> {
    let cc = api_cc(&vat[..2]);
    let num = &vat[2..];
    let url = format!("https://ec.europa.eu/taxation_customs/vies/rest-api/ms/{cc}/vat/{num}");
    let (status, body) = httpx::get_text(&url, 20).await.map_err(|_| {
        api_err(StatusCode::BAD_GATEWAY, "VIES услугата е недостъпна (мрежова грешка)")
    })?;
    if status != 200 {
        return Err(api_err(
            StatusCode::BAD_GATEWAY,
            format!("VIES върна статус {status} — опитайте пак"),
        ));
    }
    let doc: Value = serde_json::from_str(&body).map_err(|_| {
        api_err(StatusCode::BAD_GATEWAY, "невалиден JSON отговор от VIES")
    })?;
    let valid = doc.get("isValid").and_then(|v| v.as_bool()).unwrap_or(false);
    let text = |key: &str| {
        doc.get(key).and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
    };
    Ok(Hit {
        valid,
        name: text("name"),
        address: text("address"),
        user_error: text("userError"),
        request_date: text("requestDate"),
    })
}

fn lookup_json(vat: &str, hit: &Hit, parts: &addr::Parts, note: &str) -> Value {
    let mut o = addr::parts_json(parts, note);
    let obj = o.as_object_mut().expect("object");
    obj.insert("valid".into(), json!(hit.valid));
    obj.insert("name".into(), json!(hit.name));
    obj.insert("address".into(), json!(hit.address));
    obj.insert("vies_address".into(), json!(hit.address));
    obj.insert("user_error".into(), json!(hit.user_error));
    obj.insert("request_date".into(), json!(hit.request_date));
    obj.insert("vat_number".into(), json!(vat));
    obj.insert("country_code".into(), json!(&vat[..2]));
    o
}

pub async fn lookup(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<VatQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let vat = normalize(&q.vat.unwrap_or_default());
    if !shape_ok(&vat) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "vat трябва да е ЕС ДДС номер (напр. BG123456789)",
        ));
    }
    let hit = fetch(&vat).await?;
    let company = active_or_one(&st, &sub).await?;
    let raw = if hit.address == "---" { "" } else { hit.address.as_str() };
    let (parts, note) = addr::split(&st.db, company, raw, &vat[..2], "", "").await?;
    Ok(Json(lookup_json(&vat, &hit, &parts, &note)))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let vat = normalize(&form::required(form::str_field(obj, "vat_number")?, "vat_number")?);
    if !shape_ok(&vat) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "vat_number трябва да е ЕС ДДС номер (напр. BG123456789)",
        ));
    }
    let kind = form::str_field(obj, "counterpart_type")?
        .unwrap_or_else(|| "both".into());
    let kind = kind.trim();
    if kind != "customer" && kind != "supplier" && kind != "both" {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "counterpart_type трябва да е customer/supplier/both",
        ));
    }
    let hit = fetch(&vat).await?;
    if !hit.valid {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ДДС номерът не е намерен във VIES",
        ));
    }
    let company = active_or_one(&st, &sub).await?;
    let cc = &vat[..2];
    let addr_raw = if hit.address == "---" { String::new() } else { hit.address.clone() };
    let (parts, _) = addr::split(&st.db, company, &addr_raw, cc, "", "").await?;
    let name = if hit.name.is_empty() || hit.name == "---" {
        format!("VIES {vat}")
    } else {
        hit.name.clone()
    };
    let eik = if cc == "BG" { vat[2..].to_string() } else { String::new() };
    let country = if parts.country.is_empty() { cc.to_string() } else { parts.country.clone() };
    let mut map = Map::new();
    map.insert("name".into(), json!(name));
    map.insert("counterpart_type".into(), json!(kind));
    map.insert("eik".into(), json!(eik));
    map.insert("vat_number".into(), json!(vat));
    map.insert("address".into(), json!(addr_raw));
    map.insert("vies_address".into(), json!(addr_raw));
    map.insert("city".into(), json!(parts.city));
    map.insert("post_code".into(), json!(parts.post_code));
    map.insert("country".into(), json!(country));
    map.insert("street_name".into(), json!(parts.street_name));
    map.insert("building_number".into(), json!(parts.building_number));
    map.insert("address_building".into(), json!(parts.address_building));
    map.insert("additional_address_detail".into(), json!(parts.additional_address_detail));
    map.insert("address_type".into(), json!(parts.address_type));
    map.insert("region".into(), json!(parts.region));
    let created = counterparts::create_mapped(&st, company, &map).await?;
    Ok((StatusCode::CREATED, Json(created)))
}
