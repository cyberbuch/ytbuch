//! Вход като bagabuch: SHA-256 на `bagabuch|{email}|{password}`, JWT HS256.
//! Нови пароли се хешират с argon2id; старите SHA-256 хешове се сменят
//! при следващ успешен вход.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::http::HeaderMap;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api_err;
use crate::ApiError;

const TTL_SEC: usize = 604800;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
    pub iss: String,
    #[serde(default)]
    pub mfa: i64,
}

pub fn hash_password(email: &str, password: &str) -> String {
    let raw = format!("bagabuch|{email}|{password}");
    let dig = Sha256::digest(raw.as_bytes());
    hex::encode(dig)
}

/// Нов паролен хеш: argon2id като PHC низ (`$argon2id$...`).
pub fn hash_new(password: &str) -> Result<String, ApiError> {
    use argon2::password_hash::rand_core::OsRng;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| api_err(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// Резултат от проверка на парола. `Rehash` значи стари sha256 хеш:
/// паролата е вярна, но трябва да се запише отново с argon2.
pub enum PasswordCheck {
    Ok,
    Rehash,
    Fail,
}

pub fn check_password(stored: &str, email: &str, password: &str) -> PasswordCheck {
    if stored.starts_with("$argon2") {
        let Ok(parsed) = PasswordHash::new(stored) else {
            return PasswordCheck::Fail;
        };
        return match Argon2::default().verify_password(password.as_bytes(), &parsed) {
            Ok(()) => PasswordCheck::Ok,
            Err(_) => PasswordCheck::Fail,
        };
    }
    if stored == hash_password(email, password) {
        PasswordCheck::Rehash
    } else {
        PasswordCheck::Fail
    }
}

fn encode_claims(secret: &str, claims: &Claims) -> Result<String, ApiError> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| api_err(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub fn issue(secret: &str, sub: &str) -> Result<String, ApiError> {
    let now = chrono::Utc::now().timestamp() as usize;
    encode_claims(
        secret,
        &Claims {
            sub: sub.to_string(),
            iat: now,
            exp: now + TTL_SEC,
            iss: "ytbuch".into(),
            mfa: 0,
        },
    )
}

pub fn issue_mfa(secret: &str, sub: &str) -> Result<String, ApiError> {
    let now = chrono::Utc::now().timestamp() as usize;
    encode_claims(
        secret,
        &Claims {
            sub: sub.to_string(),
            iat: now,
            exp: now + 300,
            iss: "ytbuch".into(),
            mfa: 1,
        },
    )
}

fn decode_claims(secret: &str, token: &str) -> Result<Claims, ApiError> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_issuer(&["ytbuch"]);
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| api_err(axum::http::StatusCode::UNAUTHORIZED, "невалиден токен"))?;
    Ok(data.claims)
}

pub fn ttl() -> usize {
    TTL_SEC
}

pub fn subject(secret: &str, headers: &HeaderMap) -> Result<String, ApiError> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .unwrap_or("");
    if token.is_empty() {
        return Err(api_err(axum::http::StatusCode::UNAUTHORIZED, "липсва токен"));
    }
    let claims = decode_claims(secret, token)?;
    if claims.mfa == 1 {
        return Err(api_err(axum::http::StatusCode::UNAUTHORIZED, "невалиден токен"));
    }
    Ok(claims.sub)
}

pub fn mfa_subject(secret: &str, token: &str) -> Result<String, ApiError> {
    let claims = decode_claims(secret, token).map_err(|_| {
        api_err(
            axum::http::StatusCode::UNAUTHORIZED,
            "изтекъл или невалиден 2FA токен",
        )
    })?;
    if claims.mfa != 1 || claims.sub.is_empty() {
        return Err(api_err(
            axum::http::StatusCode::UNAUTHORIZED,
            "изтекъл или невалиден 2FA токен",
        ));
    }
    Ok(claims.sub)
}

#[cfg(test)]
mod tests {
    use super::{check_password, hash_new, hash_password, PasswordCheck};

    #[test]
    fn new_hash_verifies() {
        let hash = hash_new("парола-123").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(matches!(
            check_password(&hash, "a@b.c", "парола-123"),
            PasswordCheck::Ok
        ));
        assert!(matches!(
            check_password(&hash, "a@b.c", "грешна"),
            PasswordCheck::Fail
        ));
    }

    #[test]
    fn legacy_hash_wants_rehash() {
        let legacy = hash_password("a@b.c", "стара-парола");
        assert!(matches!(
            check_password(&legacy, "a@b.c", "стара-парола"),
            PasswordCheck::Rehash
        ));
        assert!(matches!(
            check_password(&legacy, "a@b.c", "друга"),
            PasswordCheck::Fail
        ));
    }
}
