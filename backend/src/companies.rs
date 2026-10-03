//! Фирми и активна фирма. JSON-ът съвпада с company_to_json в bagabuch.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

use crate::db::Db;
use crate::auth;
use crate::db::{self, cell_i32_at, cell_str};
use crate::{api_err, AppState, ApiError};

const SELECT: &str = "SELECT id, name, eik, vat_number, is_vat_registered, vat_period, currency, \
address, city, post_code, phone, email, website, mol, manager_eik, accountant_name, accountant_egn, \
tax_authority, nap_office, iban, bic, inventory_valuation_method, fiscal_year_start_month, country, \
manager_name, manager_egn, authorized_person_name, authorized_person_egn, vat_branch_number, \
tax_accounting_basis, tax_entity, software_company_name, software_id, software_version, \
is_part_of_group, street_name, building_number, address_building, additional_address_detail, \
address_type, region, settings, created_at FROM companies";

#[derive(Serialize)]
struct Company {
    id: i32,
    name: String,
    eik: String,
    vat_number: String,
    is_vat_registered: i32,
    vat_period: String,
    currency: String,
    address: String,
    city: String,
    post_code: String,
    phone: String,
    email: String,
    website: String,
    mol: String,
    manager_eik: String,
    accountant_name: String,
    accountant_egn: String,
    tax_authority: String,
    nap_office: String,
    iban: String,
    bic: String,
    inventory_valuation_method: String,
    fiscal_year_start_month: i32,
    country: String,
    manager_name: String,
    manager_egn: String,
    authorized_person_name: String,
    authorized_person_egn: String,
    vat_branch_number: String,
    tax_accounting_basis: String,
    tax_entity: String,
    software_company_name: String,
    software_id: String,
    software_version: String,
    is_part_of_group: String,
    street_name: String,
    building_number: String,
    address_building: String,
    additional_address_detail: String,
    address_type: String,
    region: String,
    settings: String,
    created_at: String,
}

fn from_row(row: &db::Row) -> Company {
    let s = |i| cell_str(row, i);
    Company {
        id: cell_i32_at(row, 0),
        name: s(1),
        eik: s(2),
        vat_number: s(3),
        is_vat_registered: cell_i32_at(row, 4),
        vat_period: s(5),
        currency: s(6),
        address: s(7),
        city: s(8),
        post_code: s(9),
        phone: s(10),
        email: s(11),
        website: s(12),
        mol: s(13),
        manager_eik: s(14),
        accountant_name: s(15),
        accountant_egn: s(16),
        tax_authority: s(17),
        nap_office: s(18),
        iban: s(19),
        bic: s(20),
        inventory_valuation_method: s(21),
        fiscal_year_start_month: cell_i32_at(row, 22),
        country: s(23),
        manager_name: s(24),
        manager_egn: s(25),
        authorized_person_name: s(26),
        authorized_person_egn: s(27),
        vat_branch_number: s(28),
        tax_accounting_basis: s(29),
        tax_entity: s(30),
        software_company_name: s(31),
        software_id: s(32),
        software_version: s(33),
        is_part_of_group: s(34),
        street_name: s(35),
        building_number: s(36),
        address_building: s(37),
        additional_address_detail: s(38),
        address_type: s(39),
        region: s(40),
        settings: s(41),
        created_at: s(42),
    }
}

fn to_json(c: &Company) -> Value {
    serde_json::to_value(c).unwrap_or_else(|_| json!({}))
}

pub(crate) async fn json_by_id(db: &Db, id: i32) -> Result<Option<Value>, ApiError> {
    Ok(find(db, id).await?.map(|c| to_json(&c)))
}

fn blank() -> Company {
    Company {
        id: 0,
        name: String::new(),
        eik: String::new(),
        vat_number: String::new(),
        is_vat_registered: 0,
        vat_period: "monthly".into(),
        currency: "EUR".into(),
        address: String::new(),
        city: String::new(),
        post_code: String::new(),
        phone: String::new(),
        email: String::new(),
        website: String::new(),
        mol: String::new(),
        manager_eik: String::new(),
        accountant_name: String::new(),
        accountant_egn: String::new(),
        tax_authority: String::new(),
        nap_office: String::new(),
        iban: String::new(),
        bic: String::new(),
        inventory_valuation_method: "WAC".into(),
        fiscal_year_start_month: 1,
        country: "BG".into(),
        manager_name: String::new(),
        manager_egn: String::new(),
        authorized_person_name: String::new(),
        authorized_person_egn: String::new(),
        vat_branch_number: String::new(),
        tax_accounting_basis: String::new(),
        tax_entity: String::new(),
        software_company_name: String::new(),
        software_id: String::new(),
        software_version: String::new(),
        is_part_of_group: String::new(),
        street_name: String::new(),
        building_number: String::new(),
        address_building: String::new(),
        additional_address_detail: String::new(),
        address_type: String::new(),
        region: String::new(),
        settings: String::new(),
        created_at: String::new(),
    }
}

fn apply_body(c: &mut Company, body: &Value) -> Result<(), ApiError> {
    let obj = body.as_object().ok_or_else(|| api_err(StatusCode::BAD_REQUEST, "очаква се обект"))?;
    let req = |key: &str| -> Result<Option<String>, ApiError> {
        match obj.get(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(Value::Null) => Ok(Some(String::new())),
            Some(_) => Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("{key} трябва да е низ"))),
        }
    };
    let set = |slot: &mut String, key: &str| -> Result<(), ApiError> {
        if let Some(v) = req(key)? {
            *slot = v;
        }
        Ok(())
    };
    set(&mut c.name, "name")?;
    set(&mut c.eik, "eik")?;
    set(&mut c.vat_number, "vat_number")?;
    set(&mut c.vat_period, "vat_period")?;
    set(&mut c.currency, "currency")?;
    set(&mut c.address, "address")?;
    set(&mut c.city, "city")?;
    set(&mut c.post_code, "post_code")?;
    set(&mut c.phone, "phone")?;
    set(&mut c.email, "email")?;
    set(&mut c.website, "website")?;
    set(&mut c.mol, "mol")?;
    set(&mut c.manager_eik, "manager_eik")?;
    set(&mut c.accountant_name, "accountant_name")?;
    set(&mut c.accountant_egn, "accountant_egn")?;
    set(&mut c.tax_authority, "tax_authority")?;
    set(&mut c.nap_office, "nap_office")?;
    set(&mut c.iban, "iban")?;
    set(&mut c.bic, "bic")?;
    set(&mut c.inventory_valuation_method, "inventory_valuation_method")?;
    set(&mut c.country, "country")?;
    set(&mut c.manager_name, "manager_name")?;
    set(&mut c.manager_egn, "manager_egn")?;
    set(&mut c.authorized_person_name, "authorized_person_name")?;
    set(&mut c.authorized_person_egn, "authorized_person_egn")?;
    set(&mut c.vat_branch_number, "vat_branch_number")?;
    set(&mut c.tax_accounting_basis, "tax_accounting_basis")?;
    set(&mut c.tax_entity, "tax_entity")?;
    set(&mut c.software_company_name, "software_company_name")?;
    set(&mut c.software_id, "software_id")?;
    set(&mut c.software_version, "software_version")?;
    set(&mut c.is_part_of_group, "is_part_of_group")?;
    set(&mut c.street_name, "street_name")?;
    set(&mut c.building_number, "building_number")?;
    set(&mut c.address_building, "address_building")?;
    set(&mut c.additional_address_detail, "additional_address_detail")?;
    set(&mut c.address_type, "address_type")?;
    set(&mut c.region, "region")?;
    set(&mut c.settings, "settings")?;
    if let Some(v) = obj.get("is_vat_registered") {
        c.is_vat_registered = match v {
            Value::Bool(b) => i32::from(*b),
            Value::Number(n) => n.as_i64().unwrap_or(0) as i32,
            _ => return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "is_vat_registered е невалидно")),
        };
    }
    if let Some(v) = obj.get("fiscal_year_start_month") {
        let n = v.as_i64().ok_or_else(|| {
            api_err(StatusCode::UNPROCESSABLE_ENTITY, "fiscal_year_start_month е невалидно")
        })?;
        c.fiscal_year_start_month = n as i32;
    }
    if c.vat_period.is_empty() {
        c.vat_period = "monthly".into();
    }
    if c.currency.is_empty() {
        c.currency = "EUR".into();
    }
    if c.inventory_valuation_method.is_empty() {
        c.inventory_valuation_method = "WAC".into();
    }
    if c.fiscal_year_start_month <= 0 {
        c.fiscal_year_start_month = 1;
    }
    Ok(())
}

fn q(value: &str) -> String {
    db::lit(value)
}

async fn find(db: &Db, id: i32) -> Result<Option<Company>, ApiError> {
    let rows = db.query(&format!("{SELECT} WHERE id = {id}"))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(rows.into_iter().next().map(|row| from_row(&row)))
}

async fn save(db: &Db, c: &Company) -> Result<(), ApiError> {
    let sql = format!(
        "UPDATE companies SET name={}, eik={}, vat_number={}, is_vat_registered={}, vat_period={}, \
         currency={}, address={}, city={}, post_code={}, phone={}, email={}, website={}, mol={}, \
         manager_eik={}, accountant_name={}, accountant_egn={}, tax_authority={}, nap_office={}, \
         iban={}, bic={}, inventory_valuation_method={}, fiscal_year_start_month={}, country={}, \
         manager_name={}, manager_egn={}, authorized_person_name={}, authorized_person_egn={}, \
         vat_branch_number={}, tax_accounting_basis={}, tax_entity={}, software_company_name={}, \
         software_id={}, software_version={}, is_part_of_group={}, street_name={}, \
         building_number={}, address_building={}, additional_address_detail={}, address_type={}, \
         region={}, settings={} WHERE id={}",
        q(&c.name), q(&c.eik), q(&c.vat_number), c.is_vat_registered, q(&c.vat_period),
        q(&c.currency), q(&c.address), q(&c.city), q(&c.post_code), q(&c.phone), q(&c.email),
        q(&c.website), q(&c.mol), q(&c.manager_eik), q(&c.accountant_name), q(&c.accountant_egn),
        q(&c.tax_authority), q(&c.nap_office), q(&c.iban), q(&c.bic),
        q(&c.inventory_valuation_method), c.fiscal_year_start_month, q(&c.country),
        q(&c.manager_name), q(&c.manager_egn), q(&c.authorized_person_name),
        q(&c.authorized_person_egn), q(&c.vat_branch_number), q(&c.tax_accounting_basis),
        q(&c.tax_entity), q(&c.software_company_name), q(&c.software_id), q(&c.software_version),
        q(&c.is_part_of_group), q(&c.street_name), q(&c.building_number), q(&c.address_building),
        q(&c.additional_address_detail), q(&c.address_type), q(&c.region), q(&c.settings), c.id
    );
    db.exec(&sql)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(())
}

pub async fn index(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    auth::subject(&st.jwt_secret, &headers)?;
    let rows = st.db.query( &format!("{SELECT} ORDER BY id"))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let items: Vec<Value> = rows.iter().map(|r| to_json(&from_row(r))).collect();
    let count = items.len();
    Ok(Json(json!({ "items": items, "count": count, "limit": 50, "offset": 0 })))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    auth::subject(&st.jwt_secret, &headers)?;
    let mut c = blank();
    apply_body(&mut c, &body)?;
    if c.name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    if c.eik.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "eik е задължителен"));
    }
    c.id = st.db.next_id( "companies")
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    c.created_at = db::now_stamp();
    let sql = format!(
        "INSERT INTO companies (id, name, eik, vat_number, is_vat_registered, vat_period, currency, \
         address, city, post_code, phone, email, website, mol, manager_eik, accountant_name, \
         accountant_egn, tax_authority, nap_office, iban, bic, inventory_valuation_method, \
         fiscal_year_start_month, country, manager_name, manager_egn, authorized_person_name, \
         authorized_person_egn, vat_branch_number, tax_accounting_basis, tax_entity, \
         software_company_name, software_id, software_version, is_part_of_group, street_name, \
         building_number, address_building, additional_address_detail, address_type, region, \
         settings, created_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, \
         {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, \
         {}, {}, {}, {}, {})",
        c.id, q(&c.name), q(&c.eik), q(&c.vat_number), c.is_vat_registered, q(&c.vat_period),
        q(&c.currency), q(&c.address), q(&c.city), q(&c.post_code), q(&c.phone), q(&c.email),
        q(&c.website), q(&c.mol), q(&c.manager_eik), q(&c.accountant_name), q(&c.accountant_egn),
        q(&c.tax_authority), q(&c.nap_office), q(&c.iban), q(&c.bic),
        q(&c.inventory_valuation_method), c.fiscal_year_start_month, q(&c.country),
        q(&c.manager_name), q(&c.manager_egn), q(&c.authorized_person_name),
        q(&c.authorized_person_egn), q(&c.vat_branch_number), q(&c.tax_accounting_basis),
        q(&c.tax_entity), q(&c.software_company_name), q(&c.software_id), q(&c.software_version),
        q(&c.is_part_of_group), q(&c.street_name), q(&c.building_number), q(&c.address_building),
        q(&c.additional_address_detail), q(&c.address_type), q(&c.region), q(&c.settings),
        q(&c.created_at)
    );
    st.db.exec( &sql)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut o = to_json(&c);
    if let Some(map) = o.as_object_mut() {
        map.insert("saft_accounts_seeded".into(), json!(0));
    }
    Ok((StatusCode::CREATED, Json(o)))
}

pub async fn show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    auth::subject(&st.jwt_secret, &headers)?;
    let c = find(&st.db, id)
        .await?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"))?;
    Ok(Json(to_json(&c)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth::subject(&st.jwt_secret, &headers)?;
    let mut c = find(&st.db, id)
        .await?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"))?;
    apply_body(&mut c, &body)?;
    save(&st.db, &c).await?;
    Ok(Json(to_json(&c)))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    auth::subject(&st.jwt_secret, &headers)?;
    let existing = st.db.query( &format!("SELECT id FROM companies WHERE id = {id}"))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if existing.is_empty() {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    }
    st.db.exec( &format!("DELETE FROM companies WHERE id = {id}"))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

fn active_key(sub: &str) -> String {
    format!("активна_фирма:{sub}")
}

pub async fn active_get(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let key = active_key(&sub);
    let rows = st.db.query(
        &format!("SELECT value FROM settings WHERE setting_key = {}", q(&key)),
    )
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let Some(row) = rows.first() else {
        return Ok(Json(json!({})));
    };
    let val = cell_str(row, 0);
    let id: i32 = val.parse().unwrap_or(0);
    if id <= 0 {
        return Ok(Json(json!({})));
    }
    match find(&st.db, id).await? {
        Some(c) => Ok(Json(to_json(&c))),
        None => Ok(Json(json!({}))),
    }
}

pub async fn active_set(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let id = body
        .get("company_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "company_id е задължителен"))?
        as i32;
    let c = find(&st.db, id)
        .await?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"))?;
    let key = active_key(&sub);
    let existing = st.db.query(
        &format!("SELECT id FROM settings WHERE setting_key = {}", q(&key)),
    )
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let val = id.to_string();
    if let Some(row) = existing.first() {
        let sid = cell_i32_at(row, 0);
        st.db.exec(
            &format!("UPDATE settings SET value = {} WHERE id = {sid}", q(&val)),
        )
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        let sid = st.db.next_id( "settings")
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        st.db.exec(
            &format!(
                "INSERT INTO settings (id, setting_key, value) VALUES ({sid}, {}, {})",
                q(&key),
                q(&val)
            ),
        )
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(Json(to_json(&c)))
}

pub async fn dashboard(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let key = active_key(&sub);
    let rows = st.db.query(
        &format!("SELECT value FROM settings WHERE setting_key = {}", q(&key)),
    )
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let company_id = rows
        .first()
        .map(|r| cell_str(r, 0).parse::<i64>().unwrap_or(0))
        .unwrap_or(0);
    let year = chrono::Utc::now().format("%Y").to_string().parse::<i32>().unwrap_or(2026);
    let invoices = st
        .db
        .query(
            "SELECT id, number, issue_date, counterpart_id, counterpart_name, currency, net_amount, vat_amount, total_amount, status, direction FROM invoices ORDER BY id DESC",
        )
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut draft = 0;
    let mut posted = 0;
    let mut recv = 0.0;
    let mut pay = 0.0;
    let mut revenue = 0.0;
    let mut expense = 0.0;
    let mut vat_out = 0.0;
    let mut vat_in = 0.0;
    let mut recent = Vec::new();
    for row in &invoices {
        let status = cell_str(row, 9);
        let direction = cell_str(row, 10);
        let net: f64 = cell_str(row, 6).parse().unwrap_or(0.0);
        let vat: f64 = cell_str(row, 7).parse().unwrap_or(0.0);
        let total: f64 = cell_str(row, 8).parse().unwrap_or(0.0);
        if status == "draft" {
            draft += 1;
        }
        if status == "posted" {
            posted += 1;
        }
        if direction == "out" {
            recv += total;
            revenue += net;
            vat_out += vat;
        } else {
            pay += total;
            expense += net;
            vat_in += vat;
        }
        if recent.len() < 5 {
            recent.push(json!({
                "id": cell_i32_at(row, 0),
                "number": cell_str(row, 1),
                "issue_date": cell_str(row, 2),
                "counterpart_id": cell_i32_at(row, 3),
                "counterpart_name": cell_str(row, 4),
                "currency": cell_str(row, 5),
                "total_amount": cell_str(row, 8),
                "status": status,
            }));
        }
    }
    let money = |v: f64| format!("{v:.2}");
    Ok(Json(json!({
        "company_id": company_id,
        "year": year,
        "invoices_draft": draft,
        "invoices_posted": posted,
        "receivables": money(recv),
        "payables": money(pay),
        "vat_output": money(vat_out),
        "vat_input": money(vat_in),
        "revenue_ytd": money(revenue),
        "expense_ytd": money(expense),
        "bank_total": "0.00",
        "recent": recent
    })))
}
