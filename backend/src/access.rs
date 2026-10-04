//! Потребители и роли. Правата са JSON текст, защото Octo няма тип JSON.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::auth;
use crate::db::{self, cell_i32_at, cell_str, lit, Db};
use crate::form::{self, auth_sub, db_err, listed};
use crate::identity;
use crate::{api_err, AppState, ApiError};

fn user_json(row: &db::Row) -> Value {
    json!({
        "id": cell_i32_at(row, 0),
        "email": cell_str(row, 1),
        "name": cell_str(row, 2),
        "role": cell_str(row, 3),
        "is_active": cell_i32_at(row, 4),
        "is_super_admin": cell_i32_at(row, 5),
        "role_id": cell_i32_at(row, 6),
        "totp_enabled": cell_i32_at(row, 7),
        "created_at": cell_str(row, 8),
    })
}

const USER_SEL: &str = "SELECT id, email, name, role, is_active, is_super_admin, role_id, totp_enabled, created_at FROM users";

async fn user_one(db: &Db, id: i32) -> Result<Value, ApiError> {
    let rows = db
        .query(&format!("{USER_SEL} WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    rows.first()
        .map(user_json)
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "потребителят не е намерен"))
}

async fn super_count(db: &Db) -> Result<i32, ApiError> {
    let rows = db
        .query("SELECT id FROM users WHERE is_super_admin = 1")
        .await
        .map_err(db_err)?;
    Ok(rows.len() as i32)
}

fn perms_text(value: &Value) -> Result<String, ApiError> {
    let list = match value {
        Value::Array(items) => items.clone(),
        Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
        Value::Null => Vec::new(),
        _ => {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "permissions трябва да е списък",
            ))
        }
    };
    let mut out = Vec::new();
    for item in list {
        let Value::String(s) = item else {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "permissions трябва да е списък от низове",
            ));
        };
        let t = s.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
    }
    Ok(serde_json::to_string(&out).unwrap_or_else(|_| "[]".into()))
}

fn perms_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| json!([]))
}

fn role_json(row: &db::Row) -> Value {
    json!({
        "id": cell_i32_at(row, 0),
        "name": cell_str(row, 1),
        "description": cell_str(row, 2),
        "permissions": perms_value(&cell_str(row, 3)),
        "created_at": cell_str(row, 4),
    })
}

pub async fn role_permissions(db: &Db, role_id: i32) -> Result<Vec<Value>, ApiError> {
    if role_id <= 0 {
        return Ok(Vec::new());
    }
    let rows = db
        .query(&format!("SELECT permissions FROM roles WHERE id = {role_id}"))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Ok(Vec::new());
    };
    Ok(perms_value(&cell_str(row, 0))
        .as_array()
        .cloned()
        .unwrap_or_default())
}

pub async fn users_index(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let rows = st
        .db
        .query(&format!("{USER_SEL} ORDER BY name"))
        .await
        .map_err(db_err)?;
    Ok(listed(rows.iter().map(user_json).collect()))
}

pub async fn users_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let email = form::required(form::str_field(obj, "email")?, "email")?;
    let email = email.trim().to_string();
    let name = form::required(form::str_field(obj, "name")?, "name")?;
    let password = form::required(form::str_field(obj, "password")?, "password")?;
    if password.chars().count() < 8 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "паролата трябва да е поне 8 символа",
        ));
    }
    let role = form::str_field(obj, "role")?
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "user".into());
    let is_active = form::flag_field(obj, "is_active")?.unwrap_or(1);
    let is_super = form::flag_field(obj, "is_super_admin")?.unwrap_or(0);
    let role_id = form::i32_field(obj, "role_id")?.unwrap_or(0);
    if identity::email_taken(&st.db, &email, 0).await? {
        return Err(api_err(StatusCode::CONFLICT, "имейлът вече се ползва"));
    }
    let id = st.db.next_id("users").await.map_err(db_err)?;
    let hash = auth::hash_password(&email, &password);
    st.db
        .exec(&format!(
            "INSERT INTO users (id, email, name, password_hash, role, is_active, is_super_admin, role_id, totp_enabled, created_at) \
             VALUES ({id}, {}, {}, {}, {}, {is_active}, {is_super}, {role_id}, 0, {})",
            lit(&email),
            lit(name.trim()),
            lit(&hash),
            lit(role.trim()),
            lit(&db::now_stamp())
        ))
        .await
        .map_err(db_err)?;
    if let Err(e) = identity::ensure_login(&st.db, id, &email).await {
        let _ = st.db.exec(&format!("DELETE FROM users WHERE id = {id}")).await;
        return Err(e);
    }
    Ok((StatusCode::CREATED, Json(user_one(&st.db, id).await?)))
}

pub async fn users_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let current = identity::by_id(&st.db, id)
        .await?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "потребителят не е намерен"))?;
    let obj = form::object(&body)?;
    let email = form::str_field(obj, "email")?;
    let name = form::str_field(obj, "name")?;
    let role = form::str_field(obj, "role")?;
    let is_active = form::flag_field(obj, "is_active")?;
    let is_super = form::flag_field(obj, "is_super_admin")?;
    let role_id = form::i32_field(obj, "role_id")?;
    let password = form::str_field(obj, "password")?
        .filter(|s| !s.is_empty());
    if let Some(ref next) = email {
        let next = next.trim();
        if next != current.email && password.is_none() {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "при смяна на имейл подайте нова парола",
            ));
        }
        if next != current.email && identity::email_taken(&st.db, next, id).await? {
            return Err(api_err(StatusCode::CONFLICT, "имейлът вече се ползва"));
        }
    }
    if let Some(ref pass) = password {
        if pass.chars().count() < 8 {
            return Err(api_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "паролата трябва да е поне 8 символа",
            ));
        }
    }
    let next_super = is_super.unwrap_or(current.is_super_admin);
    if current.is_super_admin == 1 && next_super == 0 && super_count(&st.db).await? <= 1 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "последният супер администратор не може да се махне",
        ));
    }
    if let Some(v) = name {
        st.db
            .exec(&format!("UPDATE users SET name = {} WHERE id = {id}", lit(v.trim())))
            .await
            .map_err(db_err)?;
    }
    if let Some(v) = role {
        st.db
            .exec(&format!("UPDATE users SET role = {} WHERE id = {id}", lit(v.trim())))
            .await
            .map_err(db_err)?;
    }
    if let Some(v) = is_active {
        st.db
            .exec(&format!("UPDATE users SET is_active = {v} WHERE id = {id}"))
            .await
            .map_err(db_err)?;
    }
    if let Some(v) = is_super {
        st.db
            .exec(&format!("UPDATE users SET is_super_admin = {v} WHERE id = {id}"))
            .await
            .map_err(db_err)?;
    }
    if let Some(v) = role_id {
        st.db
            .exec(&format!("UPDATE users SET role_id = {v} WHERE id = {id}"))
            .await
            .map_err(db_err)?;
    }
    if let Some(pass) = password {
        let mail = email.as_deref().unwrap_or(&current.email).trim();
        identity::set_email(&st.db, id, mail, &pass).await?;
    }
    Ok(Json(user_one(&st.db, id).await?))
}

pub async fn users_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let user = identity::by_id(&st.db, id)
        .await?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "потребителят не е намерен"))?;
    if user.username == sub || user.email == sub {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "не може да изтриете собствения си потребител",
        ));
    }
    if user.is_super_admin == 1 && super_count(&st.db).await? <= 1 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "последният супер администратор не може да се изтрие",
        ));
    }
    st.db
        .exec(&format!("DELETE FROM user_recovery_codes WHERE user_id = {id}"))
        .await
        .map_err(db_err)?;
    st.db
        .exec(&format!("DELETE FROM password_reset_tokens WHERE user_id = {id}"))
        .await
        .map_err(db_err)?;
    st.db
        .exec(&format!("DELETE FROM user_login WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    st.db
        .exec(&format!("DELETE FROM users WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn roles_index(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let rows = st
        .db
        .query("SELECT id, name, description, permissions, created_at FROM roles ORDER BY name")
        .await
        .map_err(db_err)?;
    Ok(listed(rows.iter().map(role_json).collect()))
}

async fn role_one(db: &Db, id: i32) -> Result<Value, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT id, name, description, permissions, created_at FROM roles WHERE id = {id}"
        ))
        .await
        .map_err(db_err)?;
    rows.first()
        .map(role_json)
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "ролята не е намерена"))
}

pub async fn roles_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    auth_sub(&st, &headers)?;
    let obj = form::object(&body)?;
    let name = form::required(form::str_field(obj, "name")?, "name")?;
    let description = form::str_field(obj, "description")?.unwrap_or_default();
    let perms = match obj.get("permissions") {
        Some(v) => perms_text(v)?,
        None => "[]".into(),
    };
    let id = st.db.next_id("roles").await.map_err(db_err)?;
    st.db
        .exec(&format!(
            "INSERT INTO roles (id, name, description, permissions, created_at) VALUES ({id}, {}, {}, {}, {})",
            lit(name.trim()),
            lit(description.trim()),
            lit(&perms),
            lit(&db::now_stamp())
        ))
        .await
        .map_err(db_err)?;
    Ok((StatusCode::CREATED, Json(role_one(&st.db, id).await?)))
}

pub async fn roles_update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    role_one(&st.db, id).await?;
    let obj = form::object(&body)?;
    if let Some(v) = form::str_field(obj, "name")? {
        if v.trim().is_empty() {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
        }
        st.db
            .exec(&format!("UPDATE roles SET name = {} WHERE id = {id}", lit(v.trim())))
            .await
            .map_err(db_err)?;
    }
    if let Some(v) = form::str_field(obj, "description")? {
        st.db
            .exec(&format!(
                "UPDATE roles SET description = {} WHERE id = {id}",
                lit(v.trim())
            ))
            .await
            .map_err(db_err)?;
    }
    if let Some(v) = obj.get("permissions") {
        let perms = perms_text(v)?;
        st.db
            .exec(&format!(
                "UPDATE roles SET permissions = {} WHERE id = {id}",
                lit(&perms)
            ))
            .await
            .map_err(db_err)?;
    }
    Ok(Json(role_one(&st.db, id).await?))
}

pub async fn roles_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    auth_sub(&st, &headers)?;
    role_one(&st.db, id).await?;
    st.db
        .exec(&format!("DELETE FROM roles WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    Ok(StatusCode::NO_CONTENT)
}
