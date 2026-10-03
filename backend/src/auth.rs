//! Вход като bagabuch: SHA-256 на `bagabuch|{email}|{password}`, JWT HS256.

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
