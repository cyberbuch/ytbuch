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
}

pub fn hash_password(email: &str, password: &str) -> String {
    let raw = format!("bagabuch|{email}|{password}");
    let dig = Sha256::digest(raw.as_bytes());
    hex::encode(dig)
}

pub fn issue(secret: &str, sub: &str) -> Result<String, ApiError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: sub.to_string(),
        iat: now,
        exp: now + TTL_SEC,
        iss: "ytbuch".into(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| api_err(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
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
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_issuer(&["ytbuch"]);
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| api_err(axum::http::StatusCode::UNAUTHORIZED, "невалиден токен"))?;
    Ok(data.claims.sub)
}
