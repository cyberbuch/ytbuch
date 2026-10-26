//! Профил: име, парола, имейл и 2FA. Паролата е argon2id; смяната на имейл
//! изисква парола за потвърждение.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::auth;
use crate::form::auth_sub;
use crate::identity::{self, User};
use crate::totp;
use crate::{api_err, AppState, ApiError};

async fn current(st: &AppState, headers: &HeaderMap) -> Result<User, ApiError> {
    let sub = auth_sub(st, headers)?;
    identity::by_login(&st.db, &sub)
        .await?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "потребителят не е намерен"))
}

fn password_ok(user: &User, password: &str) -> bool {
    !password.is_empty()
        && !matches!(
            auth::check_password(&user.password_hash, &user.email, password),
            auth::PasswordCheck::Fail
        )
}

fn str_val(body: &Value, key: &str) -> String {
    body.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

pub async fn update_name(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let user = current(&st, &headers).await?;
    let name = str_val(&body, "name");
    if name.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "името е задължително"));
    }
    identity::set_name(&st.db, user.id, &name).await?;
    Ok(Json(json!({ "message": "Името е записано", "name": name })))
}

pub async fn change_password(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let user = current(&st, &headers).await?;
    let current_password = str_val(&body, "current_password");
    let new_password = str_val(&body, "new_password");
    if current_password.is_empty() || new_password.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "текущата и новата парола са задължителни",
        ));
    }
    if !password_ok(&user, &current_password) {
        return Err(api_err(StatusCode::UNAUTHORIZED, "грешна парола"));
    }
    if new_password.chars().count() < 8 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "паролата трябва да е поне 8 символа",
        ));
    }
    let ver = identity::set_password(&st.db, user.id, &user.email, &new_password).await?;
    // Старият токен отпада при смяната на паролата — връщаме нов.
    let token = auth::issue(&st.jwt_secret, &user.username, ver)?;
    Ok(Json(json!({ "message": "Паролата е сменена", "access_token": token })))
}

pub async fn change_email(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let user = current(&st, &headers).await?;
    let email = str_val(&body, "email");
    let current_password = str_val(&body, "current_password");
    if email.is_empty() || current_password.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "имейл и текуща парола са задължителни",
        ));
    }
    if !email.contains('@') || email.contains(char::is_whitespace) || email.len() > 200 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "имейлът е невалиден"));
    }
    if !password_ok(&user, &current_password) {
        return Err(api_err(StatusCode::UNAUTHORIZED, "грешна парола"));
    }
    if identity::email_taken(&st.db, &email, user.id).await? {
        return Err(api_err(StatusCode::CONFLICT, "имейлът вече се ползва"));
    }
    identity::ensure_login(&st.db, user.id, &user.username).await?;
    let ver = identity::set_email(&st.db, user.id, &email, &current_password).await?;
    // И смяната на имейла приключва старите сесии — връщаме нов токен.
    let token = auth::issue(&st.jwt_secret, &user.username, ver)?;
    Ok(Json(json!({
        "message": "Имейлът е сменен",
        "email": email,
        "username": user.username,
        "access_token": token,
    })))
}

pub async fn totp_start(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let user = current(&st, &headers).await?;
    if user.totp_enabled == 1 {
        return Err(api_err(StatusCode::CONFLICT, "2FA вече е включена"));
    }
    identity::ensure_login(&st.db, user.id, &user.username).await?;
    let secret = totp::new_secret().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let account = if user.email.contains('@') { &user.email } else { &user.username };
    let uri = totp::otpauth_uri(&secret, account);
    let svg = totp::qr_svg(&uri).map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let made = totp::recovery_make().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let codes: Vec<String> = made.iter().map(|(code, _)| code.clone()).collect();
    let hashes: Vec<String> = made.iter().map(|(_, hash)| hash.clone()).collect();
    identity::wipe_recovery(&st.db, user.id).await?;
    identity::insert_recovery(&st.db, user.id, &hashes).await?;
    identity::set_totp(&st.db, user.id, 0, "", &secret).await?;
    Ok(Json(json!({
        "secret": secret,
        "uri": uri,
        "qr_svg": svg,
        "recovery_codes": codes,
        "message": "Сканирай кода и запази резервните кодове. Показват се само сега.",
    })))
}

pub async fn totp_confirm(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let user = current(&st, &headers).await?;
    let code = str_val(&body, "code");
    if code.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "кодът е задължителен"));
    }
    if user.totp_pending.is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "няма започнато включване на 2FA"));
    }
    let now = chrono::Utc::now().timestamp();
    if !totp::totp_ok(&user.totp_pending, &code, now) {
        return Err(api_err(StatusCode::UNAUTHORIZED, "грешен код"));
    }
    identity::set_totp(&st.db, user.id, 1, &user.totp_pending, "").await?;
    Ok(Json(json!({ "ok": 1, "totp_enabled": 1, "message": "2FA е включена" })))
}

pub async fn totp_disable(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let user = current(&st, &headers).await?;
    let password = str_val(&body, "password");
    let code = str_val(&body, "code");
    if password.is_empty() || code.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "парола и код са задължителни",
        ));
    }
    if !password_ok(&user, &password) {
        return Err(api_err(StatusCode::UNAUTHORIZED, "грешна парола"));
    }
    let now = chrono::Utc::now().timestamp();
    let mut ok = totp::totp_ok(&user.totp_secret, &code, now);
    if !ok {
        ok = identity::take_recovery(&st.db, user.id, &code).await?;
    }
    if !ok {
        return Err(api_err(StatusCode::UNAUTHORIZED, "грешен код"));
    }
    identity::set_totp(&st.db, user.id, 0, "", "").await?;
    identity::wipe_recovery(&st.db, user.id).await?;
    Ok(Json(json!({ "ok": 1, "totp_enabled": 0, "message": "2FA е изключена" })))
}
