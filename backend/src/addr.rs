//! Разбиване на адрес към полетата на SAF-T.
//! С ключ в настройките на фирмата пита Mistral. Без ключ дели локално.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_str, Db};
use crate::form::{self, active_or_one, auth_sub, db_err};
use crate::httpx;
use crate::saft;
use crate::{AppState, ApiError};

#[derive(Clone)]
pub struct Parts {
    pub street_name: String,
    pub building_number: String,
    pub address_building: String,
    pub additional_address_detail: String,
    pub city: String,
    pub post_code: String,
    pub region: String,
    pub country: String,
    pub address_type: String,
    pub ok: bool,
}

impl Parts {
    fn empty(country: &str) -> Self {
        Self {
            street_name: String::new(),
            building_number: String::new(),
            address_building: String::new(),
            additional_address_detail: String::new(),
            city: String::new(),
            post_code: String::new(),
            region: String::new(),
            country: country.to_string(),
            address_type: String::new(),
            ok: false,
        }
    }
}

pub fn parts_json(p: &Parts, note: &str) -> Value {
    json!({
        "street_name": p.street_name,
        "building_number": p.building_number,
        "address_building": p.address_building,
        "additional_address_detail": p.additional_address_detail,
        "city": p.city,
        "post_code": p.post_code,
        "region": p.region,
        "country": p.country,
        "address_type": p.address_type,
        "parse_ok": p.ok,
        "parse_note": note,
    })
}

pub async fn mistral_key(db: &Db, company_id: i32) -> Result<String, ApiError> {
    if company_id <= 0 {
        return Ok(String::new());
    }
    let rows = db
        .query(&format!("SELECT settings FROM companies WHERE id = {company_id}"))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Ok(String::new());
    };
    let raw = cell_str(row, 0);
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Ok(String::new());
    };
    Ok(value
        .get("mistral_api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string())
}

fn region_of_city(city: &str) -> String {
    let want = city.trim().to_lowercase();
    if want.is_empty() {
        return String::new();
    }
    let Some(rows) = saft::catalog().get("regions") else {
        return String::new();
    };
    for line in rows {
        let mut parts = line.splitn(2, '|');
        let code = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        if name.trim().to_lowercase() == want {
            return code.to_string();
        }
    }
    String::new()
}

fn take_digits(token: &str) -> bool {
    token.chars().next().is_some_and(|c| c.is_ascii_digit())
}

pub fn split_local(raw: &str, country_hint: &str, city_hint: &str, post_hint: &str) -> Parts {
    let country = {
        let c = country_hint.trim().to_ascii_uppercase();
        if c.is_empty() { "BG".into() } else { c }
    };
    let text = raw.trim();
    if text.is_empty() || text == "---" {
        return Parts::empty(&country);
    }
    let mut lines: Vec<String> = text
        .replace('\r', "")
        .split(['\n', ','])
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(last) = lines.last().cloned() {
        let up = last.to_ascii_uppercase();
        if up == "BG" || up == "BULGARIA" || last == "България" || (up.len() == 2 && up.chars().all(|c| c.is_ascii_alphabetic())) {
            lines.pop();
        }
    }
    let mut post = post_hint.trim().to_string();
    let mut city = city_hint.trim().to_string();
    let mut kept = Vec::new();
    for line in lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        if post.is_empty() {
            if let Some(pos) = words.iter().position(|w| w.len() == 4 && w.chars().all(|c| c.is_ascii_digit())) {
                post = words[pos].to_string();
                let after = words[pos + 1..].join(" ");
                if city.is_empty() && !after.is_empty() {
                    city = after;
                }
                let before = words[..pos].join(" ");
                if !before.is_empty() {
                    kept.push(before);
                }
                continue;
            }
        }
        kept.push(line);
    }
    let street_line = kept.first().cloned().unwrap_or_default();
    let mut words: Vec<String> = street_line.split_whitespace().map(|s| s.to_string()).collect();
    if words.first().is_some_and(|w| {
        let l = w.trim_end_matches('.').to_lowercase();
        l == "ул" || l == "бул" || l == "жк" || l == "улица" || l == "булевард"
    }) {
        words.remove(0);
    }
    let mut extra = Vec::new();
    let mut building = String::new();
    words.retain(|w| {
        let l = w.to_lowercase();
        if l.starts_with("ет") || l.starts_with("вх") || l.starts_with("ап") || l.starts_with("fl") {
            extra.push(w.clone());
            false
        } else if l.starts_with("бл") || l.starts_with("блок") {
            building = w.trim_start_matches("бл.").trim_start_matches("блок").trim().to_string();
            if building.is_empty() {
                building = w.clone();
            }
            false
        } else {
            true
        }
    });
    let mut number = String::new();
    if words.last().is_some_and(|w| take_digits(w)) {
        number = words.pop().unwrap_or_default();
    }
    let street = words.join(" ");
    if city.is_empty() {
        if let Some(second) = kept.get(1) {
            city = second.clone();
        }
    }
    let mut p = Parts::empty(&country);
    p.street_name = street.chars().take(70).collect();
    p.building_number = number.chars().take(18).collect();
    p.address_building = building.chars().take(35).collect();
    p.additional_address_detail = extra.join(" ").chars().take(70).collect();
    p.city = city.chars().take(35).collect();
    p.post_code = post.chars().take(18).collect();
    p.region = region_of_city(&p.city);
    p.address_type = if p.street_name.is_empty() { String::new() } else { "StreetAddress".into() };
    p.ok = !p.street_name.is_empty() || !p.city.is_empty();
    p
}

fn clip(value: &Value, key: &str, max: usize) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .chars()
        .take(max)
        .collect()
}

fn from_mistral(value: &Value, country: &str) -> Parts {
    let mut p = Parts::empty(country);
    p.street_name = clip(value, "street_name", 70);
    p.building_number = clip(value, "building_number", 18);
    p.address_building = clip(value, "address_building", 35);
    p.additional_address_detail = clip(value, "additional_address_detail", 70);
    p.city = clip(value, "city", 35);
    p.post_code = clip(value, "post_code", 18);
    p.country = {
        let c = clip(value, "country", 2).to_ascii_uppercase();
        if c.len() == 2 { c } else { country.to_string() }
    };
    let region = clip(value, "region", 12);
    p.region = saft::region_code(&region, &p.country).unwrap_or_default();
    let kind = clip(value, "address_type", 32);
    p.address_type = match kind.to_ascii_lowercase().as_str() {
        "streetaddress" => "StreetAddress".into(),
        "postaladdress" => "PostalAddress".into(),
        "billingaddress" => "BillingAddress".into(),
        "shiptoaddress" => "ShipToAddress".into(),
        "shipfromaddress" => "ShipFromAddress".into(),
        _ => if p.street_name.is_empty() { String::new() } else { "StreetAddress".into() },
    };
    p.ok = !p.street_name.is_empty() || !p.city.is_empty();
    p
}

fn region_prompt() -> String {
    let Some(rows) = saft::catalog().get("regions") else {
        return String::new();
    };
    rows.join("\n")
}

async fn ask_mistral(key: &str, raw: &str, country: &str, city: &str, post: &str) -> Option<Parts> {
    let prompt = format!(
        "Split this address into JSON for Bulgarian SAF-T. Keys: street_name, building_number, address_building, additional_address_detail, city, post_code, region, country, address_type. street_name max 70. building_number max 18. region for Bulgaria is a code from the list, else empty. country is ISO alpha-2. address_type is StreetAddress, PostalAddress, BillingAddress, ShipToAddress or ShipFromAddress.\nRegions:\n{}\nCountry code: {country}\nKnown city: {city}\nKnown post code: {post}\nAddress:\n{raw}",
        region_prompt()
    );
    let body = json!({
        "model": "mistral-small-2603",
        "temperature": 0,
        "max_tokens": 512,
        "response_format": {"type": "json_object"},
        "messages": [
            {"role": "system", "content": "You extract structured address fields. Return only JSON."},
            {"role": "user", "content": prompt}
        ]
    });
    let Ok((status, text)) = httpx::post_json("https://api.mistral.ai/v1/chat/completions", key, &body, 30).await else {
        return None;
    };
    if status != 200 {
        return None;
    }
    let Ok(doc) = serde_json::from_str::<Value>(&text) else {
        return None;
    };
    let content = doc
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    let Ok(obj) = serde_json::from_str::<Value>(&content[start..=end]) else {
        return None;
    };
    Some(from_mistral(&obj, country))
}

pub async fn split(db: &Db, company_id: i32, raw: &str, country: &str, city: &str, post: &str) -> Result<(Parts, String), ApiError> {
    let cc = {
        let c = country.trim().to_ascii_uppercase();
        if c.is_empty() { "BG".into() } else { c }
    };
    let key = mistral_key(db, company_id).await?;
    if key.is_empty() {
        let parts = split_local(raw, &cc, city, post);
        let note = if parts.ok {
            "Адресът е разделен локално. Mistral ключ няма в настройките на фирмата."
        } else {
            "Няма Mistral ключ в настройките на фирмата — адресът не е разделен."
        };
        return Ok((parts, note.into()));
    }
    if let Some(parts) = ask_mistral(&key, raw, &cc, city, post).await {
        if parts.ok {
            return Ok((parts, "Адресът е разделен за SAF-T.".into()));
        }
    }
    let local = split_local(raw, &cc, city, post);
    let note = if local.ok {
        "Mistral не раздели адреса. Полетата са от локалното разбиване."
    } else {
        "VIES ок, но разделянето на адреса не успя."
    };
    Ok((local, note.into()))
}

pub async fn parse(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let address = form::required(form::str_field(obj, "address")?, "address")?;
    let country = form::str_field(obj, "country")?.unwrap_or_else(|| "BG".into());
    let city = form::str_field(obj, "city")?.unwrap_or_default();
    let post = form::str_field(obj, "post_code")?.unwrap_or_default();
    let company = active_or_one(&st, &sub).await?;
    let (parts, note) = split(&st.db, company, address.trim(), country.trim(), city.trim(), post.trim()).await?;
    Ok(Json(parts_json(&parts, &note)))
}
