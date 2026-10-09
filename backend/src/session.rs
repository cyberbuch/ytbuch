//! Вход и `/v1/me`. При включена 2FA токенът за API се дава след `/v1/auth/mfa`.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth;
use crate::form::auth_sub;
use crate::identity;
use crate::totp;
use crate::{api_err, AppState, ApiError};

#[derive(Deserialize)]
pub(crate) struct TokenBody {
    sub: Option<String>,
    email: Option<String>,
    password: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct MfaBody {
    mfa_token: Option<String>,
    code: Option<String>,
}

fn token_json(token: String, sub: &str) -> Json<Value> {
    Json(json!({
        "access_token": token,
        "token_type": "bearer",
        "sub": sub,
        "expires_in": auth::ttl(),
    }))
}

pub async fn token(State(st): State<AppState>, Json(body): Json<TokenBody>) -> Result<Json<Value>, ApiError> {
    let login = body
        .sub
        .filter(|s| !s.trim().is_empty())
        .or(body.email.filter(|s| !s.trim().is_empty()))
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "email или sub е задължителен"))?;
    let login = login.trim();
    let password = body.password.unwrap_or_default();
    let found = identity::by_login(&st.db, login).await?;
    let Some(user) = found else {
        if identity::user_count(&st.db).await? == 0 && login == "superadmin" && password == "123+123" {
            identity::provision_superadmin(&st.db).await?;
            let token = auth::issue(&st.jwt_secret, "superadmin")?;
            return Ok(token_json(token, "superadmin"));
        }
        return Err(api_err(StatusCode::UNAUTHORIZED, "грешен потребител или парола"));
    };
    if user.is_active == 0 {
        return Err(api_err(StatusCode::FORBIDDEN, "потребителят е неактивен"));
    }
    match auth::check_password(&user.password_hash, &user.email, &password) {
        auth::PasswordCheck::Fail => {
            return Err(api_err(StatusCode::UNAUTHORIZED, "грешен потребител или парола"));
        }
        // Стар sha256 хеш: паролата е вярна, записваме я отново с argon2.
        auth::PasswordCheck::Rehash => {
            if let Err(e) = identity::set_password(&st.db, user.id, &user.email, &password).await {
                tracing::warn!("argon2 rehash на потребител {}: {e:?}", user.id);
            }
        }
        auth::PasswordCheck::Ok => {}
    }
    if user.totp_enabled == 1 {
        let mfa = auth::issue_mfa(&st.jwt_secret, &user.username)?;
        return Ok(Json(json!({
            "mfa": "totp",
            "mfa_token": mfa,
            "expires_in": 300,
            "message": "Въведи кода от приложението или резервен код",
        })));
    }
    let token = auth::issue(&st.jwt_secret, &user.username)?;
    Ok(token_json(token, &user.username))
}

pub async fn mfa(State(st): State<AppState>, Json(body): Json<MfaBody>) -> Result<Json<Value>, ApiError> {
    let raw = body.mfa_token.unwrap_or_default();
    if raw.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "mfa_token е задължителен"));
    }
    let code = body.code.unwrap_or_default();
    if code.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "кодът е задължителен"));
    }
    let sub = auth::mfa_subject(&st.jwt_secret, raw.trim())?;
    let Some(user) = identity::by_login(&st.db, &sub).await? else {
        return Err(api_err(StatusCode::UNAUTHORIZED, "грешен код"));
    };
    let now = chrono::Utc::now().timestamp();
    if totp::totp_ok(&user.totp_secret, code.trim(), now) {
        let token = auth::issue(&st.jwt_secret, &user.username)?;
        return Ok(token_json(token, &user.username));
    }
    if identity::take_recovery(&st.db, user.id, code.trim()).await? {
        let token = auth::issue(&st.jwt_secret, &user.username)?;
        return Ok(token_json(token, &user.username));
    }
    Err(api_err(StatusCode::UNAUTHORIZED, "грешен код"))
}

pub async fn me(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let Some(user) = identity::by_login(&st.db, &sub).await? else {
        return Ok(Json(json!({ "sub": sub, "permissions": [] })));
    };
    let perms = if user.is_super_admin == 1 {
        vec![serde_json::json!("*")]
    } else {
        crate::access::role_permissions(&st.db, user.role_id).await?
    };
    Ok(Json(json!({
        "sub": user.username,
        "id": user.id,
        "email": user.email,
        "username": user.username,
        "name": user.name,
        "is_super_admin": user.is_super_admin,
        "role_id": user.role_id,
        "totp_enabled": user.totp_enabled,
        "permissions": perms,
    })))
}
