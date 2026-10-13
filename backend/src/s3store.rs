//! S3 през aws-sdk-s3. Path-style и свой endpoint, както boto3 в bagabuch.
//! Няма отделен Python процес.

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation};
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::{json, Value};
use std::io::Write;

use crate::dump;
use crate::form::auth_sub;
use crate::settings::{self, S3Cfg};
use crate::{api_err, AppState, ApiError};

fn explain(err: &(impl ProvideErrorMetadata + std::fmt::Display), bucket: &str) -> String {
    match err.code() {
        Some("NoSuchBucket") => format!("Bucket '{bucket}' не съществува"),
        Some("InvalidAccessKeyId") => "Невалиден Access Key".into(),
        Some("SignatureDoesNotMatch") => "Невалиден Secret Key".into(),
        Some("AccessDenied") => "Достъпът е отказан".into(),
        Some(_) => err.message().unwrap_or("S3 грешка").to_string(),
        None => err.to_string(),
    }
}

async fn require_super_admin(st: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let sub = auth_sub(st, headers)?;
    let Some(user) = crate::identity::by_login(&st.db, &sub).await? else {
        return Err(api_err(StatusCode::UNAUTHORIZED, "потребителят не съществува"));
    };
    if user.is_super_admin != 1 {
        return Err(api_err(StatusCode::FORBIDDEN, "изисква се супер администратор"));
    }
    Ok(sub)
}

fn client(cfg: &S3Cfg) -> Result<Client, ApiError> {
    if cfg.bucket.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "S3 bucket е задължителен"));
    }
    if cfg.access_key.is_empty() || cfg.secret_key.is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "Невалидни credentials"));
    }
    let creds = Credentials::new(
        cfg.access_key.clone(),
        cfg.secret_key.clone(),
        None,
        None,
        "ytbuch",
    );
    let mut b = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(cfg.region.clone()))
        .credentials_provider(creds)
        .force_path_style(true)
        .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
        .response_checksum_validation(ResponseChecksumValidation::WhenRequired);
    if !cfg.endpoint.is_empty() {
        b = b.endpoint_url(cfg.endpoint.as_str());
    }
    Ok(Client::from_conf(b.build()))
}

fn human(size: i64) -> String {
    if size < 1_048_576 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / 1_048_576.0)
    }
}

pub async fn s3_test(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let cfg = settings::s3_cfg(&settings::load(&st.db).await?);
    let s3 = client(&cfg)?;
    s3.list_objects_v2()
        .bucket(cfg.bucket.as_str())
        .max_keys(1)
        .send()
        .await
        .map_err(|e| api_err(StatusCode::BAD_REQUEST, explain(&e, &cfg.bucket)))?;
    Ok(Json(json!({ "message": format!("Връзката с S3 е успешна. Bucket: {}", cfg.bucket) })))
}

pub async fn s3_list(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    require_super_admin(&st, &headers).await?;
    let cfg = settings::s3_cfg(&settings::load(&st.db).await?);
    let s3 = client(&cfg)?;
    let mut token: Option<String> = None;
    let mut items = Vec::new();
    loop {
        let mut req = s3
            .list_objects_v2()
            .bucket(cfg.bucket.as_str())
            .prefix(cfg.prefix.as_str());
        if let Some(t) = &token {
            req = req.continuation_token(t.as_str());
        }
        let page = req
            .send()
            .await
            .map_err(|e| api_err(StatusCode::BAD_REQUEST, explain(&e, &cfg.bucket)))?;
        for obj in page.contents() {
            let key = obj.key().unwrap_or("");
            let filename = key.strip_prefix(&cfg.prefix).unwrap_or(key);
            if !is_backup_name(filename) {
                continue;
            }
            let size = obj.size().unwrap_or(0);
            let modified = obj
                .last_modified()
                .map(|d| d.to_string())
                .unwrap_or_default();
            items.push(json!({
                "key": key,
                "filename": filename,
                "size": size,
                "size_human": human(size),
                "last_modified": modified,
            }));
        }
        if page.is_truncated() == Some(true) {
            match page.next_continuation_token() {
                Some(t) => token = Some(t.to_string()),
                None => break,
            }
        } else {
            break;
        }
    }
    items.sort_by(|a, b| {
        b["last_modified"]
            .as_str()
            .unwrap_or("")
            .cmp(a["last_modified"].as_str().unwrap_or(""))
    });
    let count = items.len();
    Ok(Json(json!({ "items": items, "count": count })))
}

pub async fn s3_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    require_super_admin(&st, &headers).await?;
    let key = body.get("s3_key").and_then(Value::as_str).unwrap_or("").trim();
    if key.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "s3_key е задължителен"));
    }
    if !is_backup_name(key) {
        return Err(api_err(StatusCode::BAD_REQUEST, "ключът не е архив на ytbuch"));
    }
    let cfg = settings::s3_cfg(&settings::load(&st.db).await?);
    let s3 = client(&cfg)?;
    s3.delete_object()
        .bucket(cfg.bucket.as_str())
        .key(key)
        .send()
        .await
        .map_err(|e| api_err(StatusCode::BAD_REQUEST, explain(&e, &cfg.bucket)))?;
    Ok(Json(json!({ "message": "Бекъпът е изтрит" })))
}

pub async fn s3_backup(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    require_super_admin(&st, &headers).await?;
    let cfg = settings::s3_cfg(&settings::load(&st.db).await?);
    let s3 = client(&cfg)?;
    let (sql, tables) = dump::logical_sql(&st.db).await?;
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(sql.as_bytes())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let bytes = enc
        .finish()
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let size = bytes.len();
    let stamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("ytbuch_backup_{stamp}.sql.gz");
    let key = format!("{}{filename}", cfg.prefix);
    s3.put_object()
        .bucket(cfg.bucket.as_str())
        .key(key.as_str())
        .body(ByteStream::from(bytes))
        .content_type("application/gzip")
        .send()
        .await
        .map_err(|e| api_err(StatusCode::BAD_REQUEST, explain(&e, &cfg.bucket)))?;
    let mut message = format!("Бекъп създаден: {filename} ({tables} таблици)");
    // Последният MUPIP архив на YottaDB от /backup също влиза в S3.
    if let Some((name, data)) = newest_ydb_backup() {
        let ydb_key = format!("{}{name}", cfg.prefix);
        s3.put_object()
            .bucket(cfg.bucket.as_str())
            .key(ydb_key.as_str())
            .body(ByteStream::from(data))
            .content_type("application/gzip")
            .send()
            .await
            .map_err(|e| api_err(StatusCode::BAD_REQUEST, explain(&e, &cfg.bucket)))?;
        message.push_str(&format!(" + {name} (MUPIP)"));
    } else {
        message.push_str(" + няма MUPIP архив в /backup");
    }
    Ok(Json(json!({
        "message": message,
        "filename": filename,
        "s3_key": key,
        "size": size,
        "tables": tables,
    })))
}

/// Последният архив от `mupip backup` (ydb_backup_*.tar.gz).
fn newest_ydb_backup() -> Option<(String, Vec<u8>)> {
    let dir = std::env::var("YTBUCH_YDB_BACKUP_DIR").unwrap_or_else(|_| "/ydb-backup".into());
    let mut best: Option<(String, std::path::PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy().to_string();
        if name.starts_with("ydb_backup_") && name.ends_with(".tar.gz")
            && best.as_ref().map(|(n, _)| name > *n).unwrap_or(true) {
                best = Some((name, path));
            }
    }
    let (name, path) = best?;
    let data = std::fs::read(path).ok()?;
    Some((name, data))
}

fn is_backup_name(name: &str) -> bool {
    name.contains("ytbuch_backup") || name.starts_with("ydb_backup_")
}
