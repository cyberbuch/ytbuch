//! Публична регистрация и забравена парола. Фирмата се създава веднага, без потвърждение по имейл.

use std::collections::HashMap;
use std::io::Read;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth;
use crate::companies;
use crate::db::{self, cell_i32_at, lit, Db};
use crate::form::db_err;
use crate::identity;
use crate::mail;
use crate::settings;
use crate::{api_err, AppState, ApiError};

const FORGOT_MSG: &str = "Ако съществува акаунт с този email, ще получите линк за възстановяване.";
const BAD_TOKEN: &str = "Невалиден или изтекъл токен. Заявете нов линк.";

pub async fn registration_status(State(st): State<AppState>) -> Result<Json<Value>, ApiError> {
    let map = settings::load(&st.db).await?;
    let en = reg_flag(map.get("auth.registration_enabled").map(String::as_str));
    Ok(Json(json!({ "ok": 1, "registration_enabled": en })))
}

pub async fn register(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let map = settings::load(&st.db).await?;
    if reg_flag(map.get("auth.registration_enabled").map(String::as_str)) == 0 {
        return Err(api_err(StatusCode::FORBIDDEN, "Регистрацията е спряна."));
    }
    if !body.is_object() {
        return Err(api_err(StatusCode::BAD_REQUEST, "очаква се обект"));
    }
    let email = trimmed(&body, "email");
    if !email.contains('@') {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "валиден email е задължителен"));
    }
    let name = trimmed(&body, "name");
    if name.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "името е задължително"));
    }
    let password = raw(&body, "password");
    let confirm = confirmation(&body, &password);
    if let Some(msg) = password_problem(&password, &confirm) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, msg));
    }
    let company_name = trimmed(&body, "company_name");
    if company_name.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "име на фирмата е задължително"));
    }
    let eik = trimmed(&body, "company_eik");
    if eik.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "ЕИК е задължителен"));
    }
    if identity::email_taken(&st.db, &email, 0).await? {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "потребител с този email вече съществува",
        ));
    }
    let cid = companies::create_for_register(
        &st.db,
        &company_name,
        &eik,
        &trimmed(&body, "company_vat_number"),
        &trimmed(&body, "company_address"),
        &trimmed(&body, "company_city"),
        &email,
    )
    .await?;
    let uid = match insert_admin(&st.db, &email, &name, &password).await {
        Ok(id) => id,
        Err(e) => {
            drop_company(&st.db, cid).await;
            return Err(e);
        }
    };
    let key = companies::active_key(&email);
    if let Err(e) = settings::upsert(&st.db, &key, &cid.to_string()).await {
        drop_user(&st.db, uid).await;
        drop_company(&st.db, cid).await;
        return Err(e);
    }
    let admin_role = crate::members::role_id_by_name(&st.db, crate::members::ROLE_ADMIN).await?;
    if let Err(e) = crate::members::grant(&st.db, uid, cid, admin_role).await {
        drop_user(&st.db, uid).await;
        drop_company(&st.db, cid).await;
        return Err(e);
    }
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "ok": 1,
            "message": "Регистрацията е успешна. Можете да влезете.",
            "company_id": cid
        })),
    ))
}

pub async fn forgot_password(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    if !body.is_object() {
        return Err(api_err(StatusCode::BAD_REQUEST, "очаква се обект"));
    }
    let email = trimmed(&body, "email");
    if email.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Email е задължителен"));
    }
    let user = identity::by_email(&st.db, &email).await?;
    let answer = Json(json!({ "ok": 1, "message": FORGOT_MSG }));
    let Some(user) = user else {
        return Ok(answer);
    };
    let raw_token = raw_token()?;
    let hash = token_hash(&raw_token);
    let id = st.db.next_id("password_reset_tokens").await.map_err(db_err)?;
    let exp = now_unix() + 3600;
    st.db
        .exec(&format!(
            "INSERT INTO password_reset_tokens (id, user_id, token_hash, expires_at, used) \
             VALUES ({id}, {}, {}, {exp}, 0)",
            user.id,
            lit(&hash)
        ))
        .await
        .map_err(db_err)?;
    let map = match settings::load(&st.db).await {
        Ok(map) => map,
        Err(_) => return Ok(answer),
    };
    if mail_ready(&map) {
        send_reset_mail(&map, &email, &raw_token).await;
    }
    Ok(answer)
}

pub async fn reset_password(
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    if !body.is_object() {
        return Err(api_err(StatusCode::BAD_REQUEST, "очаква се обект"));
    }
    let token = trimmed(&body, "token");
    if token.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Невалиден или липсващ токен"));
    }
    let password = raw(&body, "password");
    let confirm = confirmation(&body, &password);
    if let Some(msg) = password_problem(&password, &confirm) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, msg));
    }
    let hash = token_hash(&token);
    let rows = st
        .db
        .query(&format!(
            "SELECT id, user_id, expires_at, used FROM password_reset_tokens WHERE token_hash = {}",
            lit(&hash)
        ))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, BAD_TOKEN));
    };
    let tid = cell_i32_at(row, 0);
    let uid = cell_i32_at(row, 1);
    let exp = cell_i32_at(row, 2) as i64;
    let used = cell_i32_at(row, 3);
    if !token_live(used, exp, now_unix()) {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, BAD_TOKEN));
    }
    let Some(user) = identity::by_id(&st.db, uid).await? else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, BAD_TOKEN));
    };
    identity::set_password(&st.db, user.id, &user.email, &password).await?;
    st.db
        .exec(&format!(
            "UPDATE password_reset_tokens SET used = 1 WHERE id = {tid}"
        ))
        .await
        .map_err(db_err)?;
    Ok(Json(json!({
        "ok": 1,
        "message": "Паролата е променена. Можете да влезете."
    })))
}

fn reg_flag(value: Option<&str>) -> i32 {
    match value {
        Some(v) if flag_off(v) => 0,
        _ => 1,
    }
}

fn flag_off(value: &str) -> bool {
    let value = value.trim();
    value == "0" || value.eq_ignore_ascii_case("false")
}

fn password_problem(password: &str, confirm: &str) -> Option<&'static str> {
    if password.len() < 8 {
        Some("паролата трябва да е поне 8 символа")
    } else if password != confirm {
        Some("паролите не съвпадат")
    } else {
        None
    }
}

fn trimmed(body: &Value, key: &str) -> String {
    body.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn raw(body: &Value, key: &str) -> String {
    body.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn confirmation(body: &Value, password: &str) -> String {
    match body.get("password_confirmation") {
        None | Some(Value::Null) => password.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(_) => String::new(),
    }
}

fn token_hash(raw_token: &str) -> String {
    hex::encode(Sha256::digest(raw_token.as_bytes()))
}

fn token_live(used: i32, exp: i64, now: i64) -> bool {
    used != 1 && exp >= now
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

fn raw_token() -> Result<String, ApiError> {
    let mut buf = [0u8; 16];
    let mut file = std::fs::File::open("/dev/urandom")
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    file.read_exact(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(hex::encode(buf))
}

fn reset_link(app_url: &str, token: &str) -> String {
    let base = app_url.trim().trim_end_matches('/');
    format!("{base}/reset-password?token={token}")
}

fn mail_ready(map: &HashMap<String, String>) -> bool {
    let url = map.get("app.url").map(|s| s.trim()).unwrap_or("");
    if url.is_empty() {
        return false;
    }
    if map.get("smtp.enabled").is_some_and(|v| flag_off(v)) {
        return false;
    }
    !map.get("smtp.host").map(|s| s.trim()).unwrap_or("").is_empty()
}

fn reset_html(link: &str) -> String {
    let href = link
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    format!(
        "<p>Заявка за нова парола в ytbuch.</p><p><a href=\"{href}\">Смени паролата</a></p><p>Линкът е валиден 1 час.</p>"
    )
}

async fn send_reset_mail(map: &HashMap<String, String>, to: &str, token: &str) {
    let url = map.get("app.url").map(|s| s.trim()).unwrap_or("");
    let link = reset_link(url, token);
    let html = reset_html(&link);
    let cfg = settings::smtp_cfg(map);
    let to = to.to_string();
    let subject = "Възстановяване на парола — ytbuch".to_string();
    match tokio::task::spawn_blocking(move || mail::send_html(&cfg, &to, &subject, &html)).await {
        Ok(Ok(())) => {}
        Ok(Err(msg)) => tracing::warn!("reset mail: {msg}"),
        Err(err) => tracing::warn!("reset mail task: {err}"),
    }
}

async fn insert_admin(db: &Db, email: &str, name: &str, password: &str) -> Result<i32, ApiError> {
    let id = db.next_id("users").await.map_err(db_err)?;
    let hash = auth::hash_new(password)?;
    db.exec(&format!(
        "INSERT INTO users (id, email, name, password_hash, role, is_active, is_super_admin, role_id, totp_enabled, created_at) \
         VALUES ({id}, {}, {}, {}, 'admin', 1, 0, 0, 0, {})",
        lit(email),
        lit(name),
        lit(&hash),
        lit(&db::now_stamp())
    ))
    .await
    .map_err(db_err)?;
    if let Err(e) = identity::ensure_login(db, id, email).await {
        let _ = db.exec(&format!("DELETE FROM users WHERE id = {id}")).await;
        return Err(e);
    }
    Ok(id)
}

async fn drop_user(db: &Db, id: i32) {
    let _ = db.exec(&format!("DELETE FROM user_login WHERE id = {id}")).await;
    let _ = db.exec(&format!("DELETE FROM users WHERE id = {id}")).await;
}

async fn drop_company(db: &Db, id: i32) {
    if id <= 1 {
        return;
    }
    let _ = db.exec(&format!("DELETE FROM companies WHERE id = {id}")).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_is_on_unless_disabled() {
        assert_eq!(reg_flag(None), 1);
        assert_eq!(reg_flag(Some("")), 1);
        assert_eq!(reg_flag(Some("1")), 1);
        assert_eq!(reg_flag(Some("0")), 0);
        assert_eq!(reg_flag(Some("false")), 0);
        assert_eq!(reg_flag(Some("FALSE")), 0);
    }

    #[test]
    fn password_rules() {
        assert_eq!(
            password_problem("short", "short"),
            Some("паролата трябва да е поне 8 символа")
        );
        assert_eq!(
            password_problem("long-enough", "other-pass"),
            Some("паролите не съвпадат")
        );
        assert_eq!(password_problem("long-enough", "long-enough"), None);
    }

    #[test]
    fn reset_link_trims_slash() {
        assert_eq!(
            reset_link("http://127.0.0.1:3020/", "abc"),
            "http://127.0.0.1:3020/reset-password?token=abc"
        );
    }

    #[test]
    fn token_hash_is_sha256() {
        assert_eq!(
            token_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn used_or_expired_token_is_dead() {
        assert!(token_live(0, 200, 100));
        assert!(token_live(0, 100, 100));
        assert!(!token_live(1, 200, 100));
        assert!(!token_live(0, 50, 100));
    }

    #[test]
    fn mail_needs_url_and_host() {
        let mut map = HashMap::new();
        assert!(!mail_ready(&map));
        map.insert("app.url".into(), "http://127.0.0.1:3020".into());
        assert!(!mail_ready(&map));
        map.insert("smtp.host".into(), "smtp.example".into());
        assert!(mail_ready(&map));
        map.insert("smtp.enabled".into(), "0".into());
        assert!(!mail_ready(&map));
    }
}
