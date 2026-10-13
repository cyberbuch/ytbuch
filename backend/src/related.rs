//! Обекти, собственици, крайни майки и серии. Един ред на заявка.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit, Row};
use crate::form::{self, active_or_one, auth_sub, db_err, ensure_company};
use crate::{api_err, AppState, ApiError};

fn loc_type_ok(value: &str) -> bool {
    matches!(value, "OFFICE" | "STORE" | "WAREHOUSE" | "BRANCH" | "OTHER")
}

fn loc_json(r: &Row) -> Value {
    json!({
        "id": cell_i32_at(r, 0),
        "name": cell_str(r, 1),
        "location_type": cell_str(r, 2),
        "street_name": cell_str(r, 3),
        "building_number": cell_str(r, 4),
        "city": cell_str(r, 5),
        "post_code": cell_str(r, 6),
        "region": cell_str(r, 7),
        "country": cell_str(r, 8),
        "is_main": cell_i32_at(r, 9),
    })
}

const LOC_SEL: &str = "SELECT id, name, location_type, street_name, building_number, city, \
post_code, region, country, is_main, company_id FROM company_locations";

async fn loc_load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let rows = st
        .db
        .query(&format!("{LOC_SEL} WHERE id = {id} AND company_id = {company}"))
        .await
        .map_err(db_err)?;
    let Some(r) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "обектът не е намерен"));
    };
    let mut v = loc_json(r);
    v["company_id"] = json!(cell_i32_at(r, 10));
    Ok(v)
}

fn take_str(obj: &Map<String, Value>, key: &str, slot: &mut String) -> Result<(), ApiError> {
    if let Some(v) = form::str_field(obj, key)? {
        *slot = v;
    }
    Ok(())
}

pub async fn location_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    if company_id != company {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    }
    ensure_company(&st.db, company_id).await?;
    let obj = form::object(&body)?;
    let name = form::required(form::str_field(obj, "name")?, "name")?;
    let mut location_type = form::str_field(obj, "location_type")?.unwrap_or_else(|| "OFFICE".into());
    if location_type.trim().is_empty() {
        location_type = "OFFICE".into();
    }
    if !loc_type_ok(location_type.trim()) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "location_type трябва да е OFFICE, STORE, WAREHOUSE, BRANCH или OTHER",
        ));
    }
    let street = form::str_field(obj, "street_name")?.unwrap_or_default();
    let building = form::str_field(obj, "building_number")?.unwrap_or_default();
    let city = form::str_field(obj, "city")?.unwrap_or_default();
    let post = form::str_field(obj, "post_code")?.unwrap_or_default();
    let region = form::str_field(obj, "region")?.unwrap_or_default();
    let mut country = form::str_field(obj, "country")?.unwrap_or_else(|| "BG".into());
    if country.trim().is_empty() {
        country = "BG".into();
    }
    let is_main = form::flag_field(obj, "is_main")?.unwrap_or(0);
    let id = st.db.next_id("company_locations").await.map_err(db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO company_locations (id, company_id, name, location_type, street_name, \
             building_number, city, post_code, region, country, is_main) VALUES ({id}, {company_id}, \
             {}, {}, {}, {}, {}, {}, {}, {}, {is_main})",
            lit(name.trim()),
            lit(location_type.trim()),
            lit(&street),
            lit(&building),
            lit(city.trim()),
            lit(post.trim()),
            lit(region.trim()),
            lit(country.trim())
        ))
        .await
        .map_err(db_err)?;
    Ok((StatusCode::CREATED, Json(loc_load(&st, id, company).await?)))
}

pub async fn location_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let current = loc_load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let mut name = current["name"].as_str().unwrap_or("").to_string();
    let mut location_type = current["location_type"].as_str().unwrap_or("").to_string();
    let mut street = current["street_name"].as_str().unwrap_or("").to_string();
    let mut building = current["building_number"].as_str().unwrap_or("").to_string();
    let mut city = current["city"].as_str().unwrap_or("").to_string();
    let mut post = current["post_code"].as_str().unwrap_or("").to_string();
    let mut region = current["region"].as_str().unwrap_or("").to_string();
    let mut country = current["country"].as_str().unwrap_or("").to_string();
    let mut is_main = current["is_main"].as_i64().unwrap_or(0) as i32;
    take_str(obj, "name", &mut name)?;
    if name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    if let Some(v) = form::str_field(obj, "location_type")? {
        if !loc_type_ok(v.trim()) {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "location_type трябва да е OFFICE, STORE, WAREHOUSE, BRANCH или OTHER",
            ));
        }
        location_type = v;
    }
    take_str(obj, "street_name", &mut street)?;
    take_str(obj, "building_number", &mut building)?;
    take_str(obj, "city", &mut city)?;
    take_str(obj, "post_code", &mut post)?;
    take_str(obj, "region", &mut region)?;
    take_str(obj, "country", &mut country)?;
    if let Some(v) = form::flag_field(obj, "is_main")? {
        is_main = v;
    }
    st.db
        .exec(&format!(
            "UPDATE company_locations SET name = {}, location_type = {}, street_name = {}, \
             building_number = {}, city = {}, post_code = {}, region = {}, country = {}, is_main = {is_main} \
             WHERE id = {id}",
            lit(name.trim()),
            lit(location_type.trim()),
            lit(&street),
            lit(&building),
            lit(city.trim()),
            lit(post.trim()),
            lit(region.trim()),
            lit(country.trim())
        ))
        .await
        .map_err(db_err)?;
    Ok(Json(loc_load(&st, id, company).await?))
}

pub async fn location_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    loc_load(&st, id, company).await?;
    st.db
        .exec(&format!("DELETE FROM company_locations WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

fn owner_json(r: &Row) -> Value {
    json!({
        "id": cell_i32_at(r, 0),
        "first_name_bg": cell_str(r, 1),
        "last_name_bg": cell_str(r, 2),
        "egn": cell_str(r, 3),
        "first_name_latin": cell_str(r, 4),
        "last_name_latin": cell_str(r, 5),
        "country": cell_str(r, 6),
        "ownership_percentage": cell_str(r, 7),
    })
}

const OWN_SEL: &str = "SELECT id, first_name_bg, last_name_bg, egn, first_name_latin, \
last_name_latin, country, ownership_percentage FROM beneficial_owners";

async fn owner_load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let rows = st
        .db
        .query(&format!("{OWN_SEL} WHERE id = {id} AND company_id = {company}"))
        .await
        .map_err(db_err)?;
    rows.first()
        .map(owner_json)
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "собственикът не е намерен"))
}

pub async fn owner_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    if company_id != company {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    }
    ensure_company(&st.db, company_id).await?;
    let obj = form::object(&body)?;
    let first = form::required(form::str_field(obj, "first_name_bg")?, "first_name_bg")?;
    let last = form::required(form::str_field(obj, "last_name_bg")?, "last_name_bg")?;
    let egn = form::str_field(obj, "egn")?.unwrap_or_default();
    let first_lat = form::str_field(obj, "first_name_latin")?.unwrap_or_default();
    let last_lat = form::str_field(obj, "last_name_latin")?.unwrap_or_default();
    let mut country = form::str_field(obj, "country")?.unwrap_or_else(|| "BG".into());
    if country.trim().is_empty() {
        country = "BG".into();
    }
    let perc = form::str_field(obj, "ownership_percentage")?.unwrap_or_default();
    let id = st.db.next_id("beneficial_owners").await.map_err(db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO beneficial_owners (id, company_id, first_name_bg, last_name_bg, egn, \
             first_name_latin, last_name_latin, country, ownership_percentage) VALUES ({id}, \
             {company_id}, {}, {}, {}, {}, {}, {}, {})",
            lit(first.trim()),
            lit(last.trim()),
            lit(egn.trim()),
            lit(first_lat.trim()),
            lit(last_lat.trim()),
            lit(country.trim()),
            lit(perc.trim())
        ))
        .await
        .map_err(db_err)?;
    Ok((StatusCode::CREATED, Json(owner_load(&st, id, company).await?)))
}

pub async fn owner_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let current = owner_load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let mut first = current["first_name_bg"].as_str().unwrap_or("").to_string();
    let mut last = current["last_name_bg"].as_str().unwrap_or("").to_string();
    let mut egn = current["egn"].as_str().unwrap_or("").to_string();
    let mut first_lat = current["first_name_latin"].as_str().unwrap_or("").to_string();
    let mut last_lat = current["last_name_latin"].as_str().unwrap_or("").to_string();
    let mut country = current["country"].as_str().unwrap_or("").to_string();
    let mut perc = current["ownership_percentage"].as_str().unwrap_or("").to_string();
    take_str(obj, "first_name_bg", &mut first)?;
    take_str(obj, "last_name_bg", &mut last)?;
    if first.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "first_name_bg е задължително"));
    }
    if last.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "last_name_bg е задължително"));
    }
    take_str(obj, "egn", &mut egn)?;
    take_str(obj, "first_name_latin", &mut first_lat)?;
    take_str(obj, "last_name_latin", &mut last_lat)?;
    take_str(obj, "country", &mut country)?;
    take_str(obj, "ownership_percentage", &mut perc)?;
    st.db
        .exec(&format!(
            "UPDATE beneficial_owners SET first_name_bg = {}, last_name_bg = {}, egn = {}, \
             first_name_latin = {}, last_name_latin = {}, country = {}, ownership_percentage = {} \
             WHERE id = {id}",
            lit(first.trim()),
            lit(last.trim()),
            lit(egn.trim()),
            lit(first_lat.trim()),
            lit(last_lat.trim()),
            lit(country.trim()),
            lit(perc.trim())
        ))
        .await
        .map_err(db_err)?;
    Ok(Json(owner_load(&st, id, company).await?))
}

pub async fn owner_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    owner_load(&st, id, company).await?;
    st.db
        .exec(&format!("DELETE FROM beneficial_owners WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

fn parent_json(r: &Row) -> Value {
    json!({
        "id": cell_i32_at(r, 0),
        "name_bg": cell_str(r, 1),
        "uic": cell_str(r, 2),
        "name_latin": cell_str(r, 3),
        "country": cell_str(r, 4),
    })
}

const PAR_SEL: &str = "SELECT id, name_bg, uic, name_latin, country FROM ultimate_parents";

async fn parent_load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let rows = st
        .db
        .query(&format!("{PAR_SEL} WHERE id = {id} AND company_id = {company}"))
        .await
        .map_err(db_err)?;
    rows.first()
        .map(parent_json)
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "предприятието майка не е намерено"))
}

pub async fn parent_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    if company_id != company {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    }
    ensure_company(&st.db, company_id).await?;
    let obj = form::object(&body)?;
    let name = form::required(form::str_field(obj, "name_bg")?, "name_bg")?;
    let uic = form::str_field(obj, "uic")?.unwrap_or_default();
    let latin = form::str_field(obj, "name_latin")?.unwrap_or_default();
    let mut country = form::str_field(obj, "country")?.unwrap_or_else(|| "BG".into());
    if country.trim().is_empty() {
        country = "BG".into();
    }
    let id = st.db.next_id("ultimate_parents").await.map_err(db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO ultimate_parents (id, company_id, name_bg, uic, name_latin, country) \
             VALUES ({id}, {company_id}, {}, {}, {}, {})",
            lit(name.trim()),
            lit(uic.trim()),
            lit(latin.trim()),
            lit(country.trim())
        ))
        .await
        .map_err(db_err)?;
    Ok((StatusCode::CREATED, Json(parent_load(&st, id, company).await?)))
}

pub async fn parent_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let current = parent_load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let mut name = current["name_bg"].as_str().unwrap_or("").to_string();
    let mut uic = current["uic"].as_str().unwrap_or("").to_string();
    let mut latin = current["name_latin"].as_str().unwrap_or("").to_string();
    let mut country = current["country"].as_str().unwrap_or("").to_string();
    take_str(obj, "name_bg", &mut name)?;
    if name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name_bg е задължително"));
    }
    take_str(obj, "uic", &mut uic)?;
    take_str(obj, "name_latin", &mut latin)?;
    take_str(obj, "country", &mut country)?;
    st.db
        .exec(&format!(
            "UPDATE ultimate_parents SET name_bg = {}, uic = {}, name_latin = {}, country = {} WHERE id = {id}",
            lit(name.trim()),
            lit(uic.trim()),
            lit(latin.trim()),
            lit(country.trim())
        ))
        .await
        .map_err(db_err)?;
    Ok(Json(parent_load(&st, id, company).await?))
}

pub async fn parent_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    parent_load(&st, id, company).await?;
    st.db
        .exec(&format!("DELETE FROM ultimate_parents WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

fn series_json(r: &Row) -> Value {
    json!({
        "id": cell_i32_at(r, 0),
        "name": cell_str(r, 1),
        "document_types": cell_str(r, 2),
        "pad_width": cell_i32_at(r, 3),
        "start_number": cell_i32_at(r, 4),
        "next_number": cell_str(r, 5),
    })
}

const SER_SEL: &str =
    "SELECT id, name, document_types, pad_width, start_number, next_number, company_id FROM document_series";

async fn series_load(st: &AppState, id: i32, company: i32) -> Result<(Value, i32), ApiError> {
    let rows = st
        .db
        .query(&format!("{SER_SEL} WHERE id = {id} AND company_id = {company}"))
        .await
        .map_err(db_err)?;
    let Some(r) = rows.first() else {
        return Err(api_err(StatusCode::NOT_FOUND, "серията не е намерена"));
    };
    Ok((series_json(r), cell_i32_at(r, 6)))
}

fn clamp_pad(n: i32) -> i32 {
    n.clamp(1, 10)
}

fn clamp_start(n: i32) -> i32 {
    if n < 1 {
        1
    } else {
        n
    }
}

fn pad_num(n: i32, width: i32) -> String {
    let w = clamp_pad(width) as usize;
    let s = n.max(0).to_string();
    if s.len() >= w {
        s
    } else {
        format!("{s:0>w$}")
    }
}

fn csv_parts(csv: &str) -> Vec<String> {
    csv.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

async fn next_out_number(st: &AppState, company_id: i32, types: &str, start: i32, pad: i32) -> Result<String, ApiError> {
    let rows = st
        .db
        .query(&format!(
            "SELECT document_type, number FROM invoices WHERE company_id = {company_id} AND direction = 'out'"
        ))
        .await
        .map_err(db_err)?;
    let want = csv_parts(types);
    let mut base = start - 1;
    if base < 0 {
        base = 0;
    }
    for r in &rows {
        let ty = cell_str(r, 0);
        if !want.iter().any(|t| t == &ty) {
            continue;
        }
        let n: i32 = cell_str(r, 1).parse().unwrap_or(0);
        if n > base {
            base = n;
        }
    }
    Ok(pad_num(base + 1, pad))
}

async fn steal_types(st: &AppState, company_id: i32, keep_id: i32, types: &str) -> Result<(), ApiError> {
    let drop = csv_parts(types);
    if drop.is_empty() {
        return Ok(());
    }
    let rows = st
        .db
        .query(&format!(
            "SELECT id, document_types FROM document_series WHERE company_id = {company_id}"
        ))
        .await
        .map_err(db_err)?;
    for r in &rows {
        let id = cell_i32_at(r, 0);
        if id == keep_id {
            continue;
        }
        let parts = csv_parts(&cell_str(r, 1));
        let kept: Vec<String> = parts
            .iter()
            .filter(|t| !drop.iter().any(|d| d == *t))
            .cloned()
            .collect();
        if kept.len() == parts.len() {
            continue;
        }
        st.db
            .exec(&format!(
                "UPDATE document_series SET document_types = {} WHERE id = {id}",
                lit(&kept.join(","))
            ))
            .await
            .map_err(db_err)?;
    }
    Ok(())
}

pub async fn series_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    if company_id != company {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    }
    ensure_company(&st.db, company_id).await?;
    let obj = form::object(&body)?;
    let name = form::required(form::str_field(obj, "name")?, "name")?;
    let types = form::types_csv(obj)?.unwrap_or_default();
    let pad = clamp_pad(form::i32_field(obj, "pad_width")?.unwrap_or(10));
    let start = clamp_start(form::i32_field(obj, "start_number")?.unwrap_or(1));
    let next = next_out_number(&st, company_id, &types, start, pad).await?;
    let id = st.db.next_id("document_series").await.map_err(db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO document_series (id, company_id, name, document_types, pad_width, start_number, next_number) \
             VALUES ({id}, {company_id}, {}, {}, {pad}, {start}, {})",
            lit(name.trim()),
            lit(&types),
            lit(&next)
        ))
        .await
        .map_err(db_err)?;
    steal_types(&st, company_id, id, &types).await?;
    let (value, _) = series_load(&st, id, company).await?;
    Ok((StatusCode::CREATED, Json(value)))
}

pub async fn series_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let (current, company_id) = series_load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let mut name = current["name"].as_str().unwrap_or("").to_string();
    let mut types = current["document_types"].as_str().unwrap_or("").to_string();
    let mut pad = current["pad_width"].as_i64().unwrap_or(10) as i32;
    let mut start = current["start_number"].as_i64().unwrap_or(1) as i32;
    let mut next = current["next_number"].as_str().unwrap_or("").to_string();
    take_str(obj, "name", &mut name)?;
    if name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    let types_changed = obj.contains_key("document_types");
    if let Some(v) = form::types_csv(obj)? {
        types = v;
    }
    let pad_changed = obj.contains_key("pad_width");
    if let Some(v) = form::i32_field(obj, "pad_width")? {
        pad = clamp_pad(v);
    }
    let start_changed = obj.contains_key("start_number");
    if let Some(v) = form::i32_field(obj, "start_number")? {
        start = clamp_start(v);
    }
    if types_changed || pad_changed || start_changed {
        next = next_out_number(&st, company_id, &types, start, pad).await?;
    }
    st.db
        .exec(&format!(
            "UPDATE document_series SET name = {}, document_types = {}, pad_width = {pad}, \
             start_number = {start}, next_number = {} WHERE id = {id}",
            lit(name.trim()),
            lit(&types),
            lit(&next)
        ))
        .await
        .map_err(db_err)?;
    if types_changed {
        steal_types(&st, company_id, id, &types).await?;
    }
    let (value, _) = series_load(&st, id, company).await?;
    Ok(Json(value))
}

pub async fn series_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    series_load(&st, id, company).await?;
    st.db
        .exec(&format!("DELETE FROM document_series WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}
