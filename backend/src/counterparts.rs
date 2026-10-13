//! Контрагенти. Допълнителният адрес е в counterpart_extra.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit, Row};
use crate::form::{self, active_or_one, auth_sub, db_err};
use crate::pagination::{paginate, PageQuery};
use crate::saft;
use crate::{api_err, AppState, ApiError};

struct Counterpart {
    id: i32,
    company_id: i32,
    name: String,
    counterpart_type: String,
    eik: String,
    vat_number: String,
    address: String,
    city: String,
    post_code: String,
    country: String,
    contact_person: String,
    vies_address: String,
    street_name: String,
    building_number: String,
    address_building: String,
    additional_address_detail: String,
    address_type: String,
    region: String,
    email: String,
    phone: String,
}

fn cp_json(c: &Counterpart) -> Value {
    json!({
        "id": c.id,
        "name": c.name,
        "counterpart_type": c.counterpart_type,
        "eik": c.eik,
        "vat_number": c.vat_number,
        "address": c.address,
        "city": c.city,
        "post_code": c.post_code,
        "country": c.country,
        "contact_person": c.contact_person,
        "vies_address": c.vies_address,
        "street_name": c.street_name,
        "building_number": c.building_number,
        "address_building": c.address_building,
        "additional_address_detail": c.additional_address_detail,
        "address_type": c.address_type,
        "region": c.region,
        "email": c.email,
        "phone": c.phone,
    })
}

fn cp_from(r: &Row) -> Counterpart {
    Counterpart {
        id: cell_i32_at(r, 0),
        company_id: cell_i32_at(r, 1),
        name: cell_str(r, 2),
        counterpart_type: cell_str(r, 3),
        eik: cell_str(r, 4),
        vat_number: cell_str(r, 5),
        address: cell_str(r, 6),
        city: cell_str(r, 7),
        post_code: cell_str(r, 8),
        country: cell_str(r, 9),
        contact_person: cell_str(r, 10),
        vies_address: cell_str(r, 11),
        street_name: cell_str(r, 12),
        building_number: cell_str(r, 13),
        address_building: cell_str(r, 14),
        additional_address_detail: cell_str(r, 15),
        address_type: cell_str(r, 16),
        region: cell_str(r, 17),
        email: cell_str(r, 18),
        phone: cell_str(r, 19),
    }
}

const CP_SEL: &str = "SELECT c.id, c.company_id, c.name, c.counterpart_type, c.eik, c.vat_number, \
c.address, c.city, c.post_code, c.country, c.contact_person, e.vies_address, e.street_name, \
e.building_number, e.address_building, e.additional_address_detail, e.address_type, e.region, \
e.email, e.phone FROM counterparts c LEFT JOIN counterpart_extra e ON c.id = e.id";

async fn cp_rows(st: &AppState, sql: &str) -> Result<Vec<Counterpart>, ApiError> {
    let rows = st.db.query(sql).await.map_err(db_err)?;
    Ok(rows.iter().map(cp_from).collect())
}

pub async fn counterparts(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let items: Vec<Value> = cp_rows(
        &st,
        &format!("{CP_SEL} WHERE c.company_id = {company} ORDER BY c.id"),
    )
    .await?
    .iter()
    .map(cp_json)
    .collect();
    Ok(paginate(items, &q))
}

async fn cp_load(st: &AppState, id: i32, company: i32) -> Result<Counterpart, ApiError> {
    let mut rows = cp_rows(
        st,
        &format!("{CP_SEL} WHERE c.id = {id} AND c.company_id = {company}"),
    )
    .await?;
    rows.pop()
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "контрагентът не е намерен"))
}

fn cp_type_ok(value: &str) -> bool {
    matches!(value, "customer" | "supplier" | "both")
}

fn address_type_norm(raw: &str) -> Result<String, ApiError> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(String::new());
    }
    let canon = match t.to_ascii_uppercase().as_str() {
        "STREETADDRESS" => "StreetAddress",
        "POSTALADDRESS" => "PostalAddress",
        "BILLINGADDRESS" => "BillingAddress",
        "SHIPTOADDRESS" => "ShipToAddress",
        "SHIPFROMADDRESS" => "ShipFromAddress",
        _ => {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "address_type е StreetAddress, PostalAddress, BillingAddress, ShipToAddress или ShipFromAddress",
            ))
        }
    };
    Ok(canon.into())
}

fn region_norm(raw: &str, country: &str) -> Result<String, ApiError> {
    saft::region_code(raw, country).map_err(|_| {
        api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "region трябва да е код BG-01 … BG-28",
        )
    })
}

fn apply_cp(c: &mut Counterpart, obj: &serde_json::Map<String, Value>, fresh: bool) -> Result<(), ApiError> {
    if let Some(v) = form::str_field(obj, "name")? {
        if v.trim().is_empty() {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
        }
        c.name = v;
    } else if fresh {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    if let Some(v) = form::str_field(obj, "counterpart_type")? {
        let t = v.trim().to_string();
        if !cp_type_ok(&t) {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "counterpart_type е customer, supplier или both",
            ));
        }
        c.counterpart_type = t;
    }
    if let Some(v) = form::str_field(obj, "eik")? {
        c.eik = v;
    }
    if let Some(v) = form::str_field(obj, "vat_number")? {
        c.vat_number = v;
    }
    if let Some(v) = form::str_field(obj, "address")? {
        c.address = v;
    }
    if let Some(v) = form::str_field(obj, "city")? {
        c.city = v;
    }
    if let Some(v) = form::str_field(obj, "post_code")? {
        c.post_code = v;
    }
    if let Some(v) = form::str_field(obj, "country")? {
        c.country = v;
    }
    if let Some(v) = form::str_field(obj, "contact_person")? {
        c.contact_person = v;
    }
    if let Some(v) = form::str_field(obj, "vies_address")? {
        c.vies_address = v;
    }
    if let Some(v) = form::str_field(obj, "street_name")? {
        c.street_name = v;
    }
    if let Some(v) = form::str_field(obj, "building_number")? {
        c.building_number = v;
    }
    if let Some(v) = form::str_field(obj, "address_building")? {
        c.address_building = v;
    }
    if let Some(v) = form::str_field(obj, "additional_address_detail")? {
        c.additional_address_detail = v;
    }
    if let Some(v) = form::str_field(obj, "email")? {
        c.email = v;
    }
    if let Some(v) = form::str_field(obj, "phone")? {
        c.phone = v;
    }
    if obj.contains_key("address_type") || fresh {
        let raw = form::str_field(obj, "address_type")?.unwrap_or_else(|| c.address_type.clone());
        c.address_type = address_type_norm(&raw)?;
    }
    if obj.contains_key("region") || fresh {
        let raw = form::str_field(obj, "region")?.unwrap_or_else(|| c.region.clone());
        let country = if c.country.trim().is_empty() { "BG" } else { c.country.trim() };
        c.region = region_norm(&raw, country)?;
    }
    if c.country.trim().is_empty() {
        c.country = "BG".into();
    }
    if c.counterpart_type.trim().is_empty() {
        c.counterpart_type = "both".into();
    }
    Ok(())
}

async fn cp_save(st: &AppState, c: &Counterpart, insert: bool) -> Result<(), ApiError> {
    if insert {
        st.db
            .exec(&format!(
                "INSERT INTO counterparts (id, company_id, name, counterpart_type, eik, vat_number, \
                 address, city, post_code, country, contact_person) VALUES ({}, {}, {}, {}, {}, {}, \
                 {}, {}, {}, {}, {})",
                c.id,
                c.company_id,
                lit(c.name.trim()),
                lit(c.counterpart_type.trim()),
                lit(c.eik.trim()),
                lit(c.vat_number.trim()),
                lit(&c.address),
                lit(c.city.trim()),
                lit(c.post_code.trim()),
                lit(c.country.trim()),
                lit(&c.contact_person)
            ))
            .await
            .map_err(db_err)?;
    } else {
        st.db
            .exec(&format!(
                "UPDATE counterparts SET name = {}, counterpart_type = {}, eik = {}, vat_number = {}, \
                 address = {}, city = {}, post_code = {}, country = {}, contact_person = {} WHERE id = {}",
                lit(c.name.trim()),
                lit(c.counterpart_type.trim()),
                lit(c.eik.trim()),
                lit(c.vat_number.trim()),
                lit(&c.address),
                lit(c.city.trim()),
                lit(c.post_code.trim()),
                lit(c.country.trim()),
                lit(&c.contact_person),
                c.id
            ))
            .await
            .map_err(db_err)?;
    }
    let have = st
        .db
        .query(&format!("SELECT id FROM counterpart_extra WHERE id = {}", c.id))
        .await
        .map_err(db_err)?;
    let extra = format!(
        "vies_address = {}, street_name = {}, building_number = {}, address_building = {}, \
         additional_address_detail = {}, address_type = {}, region = {}, email = {}, phone = {}",
        lit(&c.vies_address),
        lit(&c.street_name),
        lit(&c.building_number),
        lit(&c.address_building),
        lit(&c.additional_address_detail),
        lit(&c.address_type),
        lit(&c.region),
        lit(c.email.trim()),
        lit(c.phone.trim())
    );
    let sql = if have.is_empty() {
        format!(
            "INSERT INTO counterpart_extra (id, vies_address, street_name, building_number, \
             address_building, additional_address_detail, address_type, region, email, phone) \
             VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            c.id,
            lit(&c.vies_address),
            lit(&c.street_name),
            lit(&c.building_number),
            lit(&c.address_building),
            lit(&c.additional_address_detail),
            lit(&c.address_type),
            lit(&c.region),
            lit(c.email.trim()),
            lit(c.phone.trim())
        )
    } else {
        format!("UPDATE counterpart_extra SET {extra} WHERE id = {}", c.id)
    };
    if let Err(e) = st.db.exec(&sql).await {
        if insert {
            let _ = st
                .db
                .exec(&format!("DELETE FROM counterparts WHERE id = {}", c.id))
                .await;
        }
        return Err(db_err(e));
    }
    Ok(())
}

pub(crate) async fn json_by_id(
    db: &crate::db::Db,
    id: i32,
    company: i32,
) -> Result<Option<Value>, ApiError> {
    if id <= 0 {
        return Ok(None);
    }
    let rows = db
        .query(&format!(
            "{CP_SEL} WHERE c.id = {id} AND c.company_id = {company}"
        ))
        .await
        .map_err(db_err)?;
    Ok(rows.first().map(cp_from).map(|c| cp_json(&c)))
}

pub async fn counterpart_show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    Ok(Json(cp_json(&cp_load(&st, id, company).await?)))
}

pub async fn counterpart_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let mut c = Counterpart {
        id: 0,
        company_id: active_or_one(&st, &sub).await?,
        name: String::new(),
        counterpart_type: "both".into(),
        eik: String::new(),
        vat_number: String::new(),
        address: String::new(),
        city: String::new(),
        post_code: String::new(),
        country: "BG".into(),
        contact_person: String::new(),
        vies_address: String::new(),
        street_name: String::new(),
        building_number: String::new(),
        address_building: String::new(),
        additional_address_detail: String::new(),
        address_type: String::new(),
        region: String::new(),
        email: String::new(),
        phone: String::new(),
    };
    apply_cp(&mut c, obj, true)?;
    c.id = st.db.next_id("counterparts").await.map_err(db_err)?;
    cp_save(&st, &c, true).await?;
    Ok((StatusCode::CREATED, Json(cp_json(&cp_load(&st, c.id, c.company_id).await?))))
}

pub async fn counterpart_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let obj = form::object(&body)?;
    let mut c = cp_load(&st, id, company).await?;
    apply_cp(&mut c, obj, false)?;
    cp_save(&st, &c, false).await?;
    Ok(Json(cp_json(&cp_load(&st, id, company).await?)))
}

pub(crate) async fn create_mapped(
    st: &AppState,
    company_id: i32,
    obj: &serde_json::Map<String, Value>,
) -> Result<Value, ApiError> {
    let mut c = Counterpart {
        id: 0,
        company_id,
        name: String::new(),
        counterpart_type: "both".into(),
        eik: String::new(),
        vat_number: String::new(),
        address: String::new(),
        city: String::new(),
        post_code: String::new(),
        country: "BG".into(),
        contact_person: String::new(),
        vies_address: String::new(),
        street_name: String::new(),
        building_number: String::new(),
        address_building: String::new(),
        additional_address_detail: String::new(),
        address_type: String::new(),
        region: String::new(),
        email: String::new(),
        phone: String::new(),
    };
    apply_cp(&mut c, obj, true)?;
    c.id = st.db.next_id("counterparts").await.map_err(db_err)?;
    cp_save(st, &c, true).await?;
    Ok(cp_json(&cp_load(st, c.id, company_id).await?))
}

pub async fn counterpart_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    cp_load(&st, id, company).await?;
    st.db
        .exec(&format!("DELETE FROM counterpart_extra WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    st.db
        .exec(&format!("DELETE FROM counterparts WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}
