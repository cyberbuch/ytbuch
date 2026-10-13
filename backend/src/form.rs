//! Общи помощници за тяло на заявка. Липсващо поле не се пипа.
//! `null` на низ го изчиства.

use axum::http::{HeaderMap, StatusCode};
use serde_json::{Map, Value};

use crate::db::Db;
use crate::{api_err, auth, companies, AppState, ApiError};

pub fn db_err(e: tokio_postgres::Error) -> ApiError {
    api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

pub fn object(body: &Value) -> Result<&Map<String, Value>, ApiError> {
    body.as_object()
        .ok_or_else(|| api_err(StatusCode::BAD_REQUEST, "очаква се обект"))
}

pub fn str_field(obj: &Map<String, Value>, key: &str) -> Result<Option<String>, ApiError> {
    match obj.get(key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Null) => Ok(Some(String::new())),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(_) => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} трябва да е низ"),
        )),
    }
}

pub fn flag_field(obj: &Map<String, Value>, key: &str) -> Result<Option<i32>, ApiError> {
    match obj.get(key) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(i32::from(*b))),
        Some(Value::Null) => Ok(Some(0)),
        Some(Value::Number(n)) => Ok(Some(i32::from(n.as_i64().unwrap_or(0) != 0))),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t == "1" || t.eq_ignore_ascii_case("true") {
                Ok(Some(1))
            } else if t.is_empty() || t == "0" || t.eq_ignore_ascii_case("false") {
                Ok(Some(0))
            } else {
                Err(api_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("{key} е невалидно"),
                ))
            }
        }
        Some(_) => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} е невалидно"),
        )),
    }
}

pub fn i32_field(obj: &Map<String, Value>, key: &str) -> Result<Option<i32>, ApiError> {
    match obj.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(0)),
        Some(Value::Number(n)) => {
            let v = n.as_i64().ok_or_else(|| {
                api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("{key} е невалидно"))
            })?;
            if !(0..=i32::MAX as i64).contains(&v) {
                return Err(api_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("{key} е невалидно"),
                ));
            }
            Ok(Some(v as i32))
        }
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                return Ok(Some(0));
            }
            t.parse::<i32>().map(Some).map_err(|_| {
                api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("{key} е невалидно"))
            })
        }
        Some(_) => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} е невалидно"),
        )),
    }
}

pub fn types_csv(obj: &Map<String, Value>) -> Result<Option<String>, ApiError> {
    match obj.get("document_types") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(String::new())),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Array(items)) => {
            let mut parts = Vec::new();
            for el in items {
                let Value::String(s) = el else {
                    return Err(api_err(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "document_types трябва да е списък от низове",
                    ));
                };
                let t = s.trim();
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
            }
            Ok(Some(parts.join(",")))
        }
        Some(_) => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "document_types трябва да е низ или списък",
        )),
    }
}

pub fn required(value: Option<String>, key: &str) -> Result<String, ApiError> {
    match value {
        Some(s) if !s.trim().is_empty() => Ok(s),
        _ => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} е задължително"),
        )),
    }
}

pub fn auth_sub(st: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    auth::subject(&st.jwt_secret, headers)
}

pub async fn active_or_one(st: &AppState, sub: &str) -> Result<i32, ApiError> {
    companies::resolve_active(&st.db, sub).await
}

pub async fn ensure_company(db: &Db, id: i32) -> Result<(), ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid id"));
    }
    let rows = db
        .query(&format!("SELECT id FROM companies WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    if rows.is_empty() {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    }
    Ok(())
}

/// Редът от `table` трябва да принадлежи на фирма `company`.
/// Чужд ред се отчита като липсващ (404), за да не се издава съществуването му.
/// `table` е вътрешна константа, не идва от заявката.
pub async fn ensure_row(
    db: &Db,
    table: &str,
    id: i32,
    company: i32,
    what: &str,
) -> Result<(), ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::NOT_FOUND, format!("{what} не е намерен")));
    }
    let rows = db
        .query(&format!(
            "SELECT id FROM {table} WHERE id = {id} AND company_id = {company}"
        ))
        .await
        .map_err(db_err)?;
    if rows.is_empty() {
        return Err(api_err(StatusCode::NOT_FOUND, format!("{what} не е намерен")));
    }
    Ok(())
}

/// Като `ensure_row`, но за незадължителни препратки: 0/отрицателно е "няма".
pub async fn ensure_ref(
    db: &Db,
    table: &str,
    id: i32,
    company: i32,
    what: &str,
) -> Result<(), ApiError> {
    if id <= 0 {
        return Ok(());
    }
    ensure_row(db, table, id, company, what).await
}

/// Редът със собственик `owner_col`; за таблици без company_id (напр. journal_entries).
pub async fn ensure_owner(
    db: &Db,
    table: &str,
    id: i32,
    owner_col: &str,
    company: i32,
    what: &str,
) -> Result<(), ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::NOT_FOUND, format!("{what} не е намерен")));
    }
    let rows = db
        .query(&format!(
            "SELECT id FROM {table} WHERE id = {id} AND {owner_col} = {company}"
        ))
        .await
        .map_err(db_err)?;
    if rows.is_empty() {
        return Err(api_err(StatusCode::NOT_FOUND, format!("{what} не е намерен")));
    }
    Ok(())
}

/// Идентификаторът на реда от `table`, ако принадлежи на фирма `company`.
pub async fn owned_id(
    db: &Db,
    table: &str,
    id: i32,
    company: i32,
) -> Result<Option<i32>, ApiError> {
    if id <= 0 {
        return Ok(None);
    }
    let rows = db
        .query(&format!(
            "SELECT id FROM {table} WHERE id = {id} AND company_id = {company}"
        ))
        .await
        .map_err(db_err)?;
    Ok(rows.first().map(|_| id))
}
