//! Глобални настройки. Колоната в базата е setting_key, в JSON е key.

use std::collections::HashMap;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{self, cell_str, lit, Db};
use crate::form::{auth_sub, db_err, listed};
use crate::{api_err, AppState, ApiError};

pub async fn load(db: &Db) -> Result<HashMap<String, String>, ApiError> {
    let rows = db
        .query("SELECT setting_key, value FROM settings")
        .await
        .map_err(db_err)?;
    let mut map = HashMap::new();
    for row in rows {
        map.insert(cell_str(&row, 0), cell_str(&row, 1));
    }
    Ok(map)
}

fn items_json(map: &HashMap<String, String>) -> Vec<Value> {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    keys.into_iter()
        .map(|k| json!({ "key": k, "value": map.get(k).cloned().unwrap_or_default() }))
        .collect()
}

pub async fn index(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let map = load(&st.db).await?;
    Ok(listed(items_json(&map)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let items = body
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| api_err(StatusCode::BAD_REQUEST, "items е задължителен масив"))?;
    for item in items {
        let key = item.get("key").and_then(Value::as_str).unwrap_or("").trim();
        if key.is_empty() {
            continue;
        }
        let value = match item.get("value") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Bool(b)) => if *b { "1".into() } else { "0".into() },
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Null) | None => String::new(),
            Some(_) => {
                return Err(api_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("{key} трябва да е низ"),
                ))
            }
        };
        upsert(&st.db, key, &value).await?;
    }
    let map = load(&st.db).await?;
    Ok(listed(items_json(&map)))
}

async fn upsert(db: &Db, key: &str, value: &str) -> Result<(), ApiError> {
    let rows = db
        .query(&format!(
            "SELECT id FROM settings WHERE setting_key = {}",
            lit(key)
        ))
        .await
        .map_err(db_err)?;
    if let Some(row) = rows.first() {
        let id = db::cell_i32_at(row, 0);
        db.exec(&format!(
            "UPDATE settings SET value = {} WHERE id = {id}",
            lit(value)
        ))
        .await
        .map_err(db_err)?;
    } else {
        let id = db.next_id("settings").await.map_err(db_err)?;
        db.exec(&format!(
            "INSERT INTO settings (id, setting_key, value) VALUES ({id}, {}, {})",
            lit(key),
            lit(value)
        ))
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

pub struct SmtpCfg {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub use_tls: bool,
}

pub struct S3Cfg {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub prefix: String,
}

fn get<'a>(map: &'a HashMap<String, String>, key: &str) -> &'a str {
    map.get(key).map(String::as_str).unwrap_or("")
}

pub fn smtp_cfg(map: &HashMap<String, String>) -> SmtpCfg {
    let port = get(map, "smtp.port").parse().unwrap_or(587);
    let tls = get(map, "smtp.use_tls");
    let from_name = get(map, "smtp.from_name");
    SmtpCfg {
        host: get(map, "smtp.host").trim().to_string(),
        port: if port == 0 { 587 } else { port },
        username: get(map, "smtp.username").to_string(),
        password: get(map, "smtp.password").to_string(),
        from_email: get(map, "smtp.from_email").trim().to_string(),
        from_name: if from_name.is_empty() {
            "ytbuch".into()
        } else {
            from_name.to_string()
        },
        use_tls: tls != "0" && !tls.eq_ignore_ascii_case("false"),
    }
}

pub fn s3_cfg(map: &HashMap<String, String>) -> S3Cfg {
    let region = get(map, "s3.region");
    let prefix = get(map, "s3.prefix");
    S3Cfg {
        endpoint: get(map, "s3.endpoint").trim().to_string(),
        bucket: get(map, "s3.bucket").trim().to_string(),
        access_key: get(map, "s3.access_key").trim().to_string(),
        secret_key: get(map, "s3.secret_key").to_string(),
        region: if region.is_empty() {
            "us-east-1".into()
        } else {
            region.to_string()
        },
        prefix: if prefix.is_empty() {
            "backups/".into()
        } else {
            prefix.to_string()
        },
    }
}

pub fn app_name(map: &HashMap<String, String>) -> String {
    let name = get(map, "app.name").trim();
    if name.is_empty() {
        "ytbuch".into()
    } else {
        name.to_string()
    }
}
