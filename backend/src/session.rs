//! Вход и `/v1/me`. При включена 2FA токенът за API се дава след `/v1/auth/mfa`.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth;
use crate::form::auth_sub;
use crate::identity;
use crate::rate_limit;
use crate::totp;
use crate::{api_err, AppState, ApiError};

/// Колко пъти може да се сбърка кодът за 2FA, преди да се заключи.
const MFA_MAX_FAILS: i32 = 5;
/// Заключване на 2FA след много неуспешни опити (секунди).
const MFA_LOCK_SEC: i64 = 900;

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
    // Заключване по акаунт — ключът е опитаният login и за съществуващ, и за
    // несъществуващ, затова не издава кои акаунти има.
    if let Some(left) = rate_limit::login_locked(login) {
        let mins = left.as_secs().div_ceil(60).max(1);
        return Err(api_err(
            StatusCode::TOO_MANY_REQUESTS,
            format!("твърде много неуспешни опити, опитайте след {mins} минути"),
        ));
    }
    let found = identity::by_login(&st.db, login).await?;
    let Some(user) = found else {
        // Еднакво време при липсващ потребител — да не се познава кои имейли има.
        auth::burn_password_check(&password);
        rate_limit::login_failure(login);
        return Err(api_err(StatusCode::UNAUTHORIZED, "грешен потребител или парола"));
    };
    match auth::check_password(&user.password_hash, &user.email, &password) {
        auth::PasswordCheck::Fail => {
            rate_limit::login_failure(login);
            return Err(api_err(StatusCode::UNAUTHORIZED, "грешен потребител или парола"));
        }
        // Стар sha256 хеш: паролата е вярна, записваме я отново с argon2.
        auth::PasswordCheck::Rehash => {
            if let Err(e) = identity::rehash_password(&st.db, user.id, &password).await {
                tracing::warn!("argon2 rehash на потребител {}: {e:?}", user.id);
            }
        }
        auth::PasswordCheck::Ok => {}
    }
    rate_limit::login_success(login);
    // След валидна парола — иначе отговорът 403 издава, че акаунтът съществува.
    if user.is_active == 0 {
        return Err(api_err(StatusCode::FORBIDDEN, "потребителят е неактивен"));
    }
    if user.totp_enabled == 1 {
        let mfa = auth::issue_mfa(&st.jwt_secret, &user.username, user.ver)?;
        return Ok(Json(json!({
            "mfa": "totp",
            "mfa_token": mfa,
            "expires_in": 300,
            "message": "Въведи кода от приложението или резервен код",
        })));
    }
    let token = auth::issue(&st.jwt_secret, &user.username, user.ver)?;
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
    if user.mfa_lock_until > now {
        return Err(api_err(
            StatusCode::TOO_MANY_REQUESTS,
            "2FA е заключен за 15 минути след неуспешни опити",
        ));
    }
    if let Some(counter) = totp::totp_counter(&user.totp_secret, code.trim(), now) {
        // Прозорецът се консумира: един и същ код не минава повторно.
        if identity::totp_consume_counter(&st.db, user.id, counter).await? {
            identity::mfa_reset(&st.db, user.id).await?;
            let token = auth::issue(&st.jwt_secret, &user.username, user.ver)?;
            return Ok(token_json(token, &user.username));
        }
    }
    if identity::take_recovery(&st.db, user.id, code.trim()).await? {
        identity::mfa_reset(&st.db, user.id).await?;
        let token = auth::issue(&st.jwt_secret, &user.username, user.ver)?;
        return Ok(token_json(token, &user.username));
    }
    // Много опити с код → заключване, за да не се гадае кодът на догадки.
    let fails = user.mfa_fails + 1;
    identity::mfa_failure(&st.db, user.id).await?;
    if fails >= MFA_MAX_FAILS {
        identity::mfa_lock(&st.db, user.id, now + MFA_LOCK_SEC).await?;
        return Err(api_err(
            StatusCode::TOO_MANY_REQUESTS,
            "изчерпани опити за 2FA — заключено за 15 минути",
        ));
    }
    Err(api_err(StatusCode::UNAUTHORIZED, "грешен код"))
}

pub async fn me(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let Some(user) = identity::by_login(&st.db, &sub).await? else {
        return Ok(Json(json!({ "sub": sub, "permissions": [] })));
    };
    let companies = crate::members::me_companies(&st.db, user.id).await?;
    let perms = if user.is_super_admin == 1 {
        vec![serde_json::json!("*")]
    } else {
        match crate::companies::resolve_active(&st.db, &user.username).await {
            Ok(company) => crate::members::perms_for(&st.db, &user, company).await?,
            Err(_) => Vec::new(),
        }
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
        "companies": companies,
    })))
}
