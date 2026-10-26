//! Rate limiting с tower_governor.
//! Зад Next.js проксито всички заявки идват от един IP, затова общият лимит
//! е на потребителя (JWT токен), а не на адреса — иначе всички делят едно
//! гърне и бързото обикаляне на менютата го изчерпва. Без токен (и на
//! `/v1/auth/*`) ключът е IP-то.
//! Токенът се проверява по подпис: иначе всеки нов текст в Authorization
//! би бил нов кош и лимитът се заобикаля с нов заглавен блок на всяка заявка.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request};
use governor::middleware::NoOpMiddleware;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::{KeyExtractor, PeerIpKeyExtractor};
use tower_governor::{GovernorError, GovernorLayer};

use crate::auth;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum ClientKey {
    User(u64),
    Ip(SocketAddr),
}

/// Ключ по потребителя от проверен токен; без такъв — по IP на връзката.
/// Самият токен не се пази, само хеш.
#[derive(Clone)]
pub struct ClientKeyExtractor {
    secret: String,
}

impl KeyExtractor for ClientKeyExtractor {
    type Key = ClientKey;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let raw = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim();
        if !raw.is_empty() {
            let token = raw
                .strip_prefix("Bearer ")
                .or_else(|| raw.strip_prefix("bearer "))
                .unwrap_or("");
            // Само истинският токен дава свой кош; фалшивият пада на IP.
            if let Some(sub) = auth::verify_subject(&self.secret, token) {
                let mut hasher = DefaultHasher::new();
                sub.hash(&mut hasher);
                return Ok(ClientKey::User(hasher.finish()));
            }
        }
        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| ClientKey::Ip(c.0))
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Общ лимит: 300 заявки накуп и по 1 на 50мс (20/сек) на потребител.
/// Едно отваряне на екран прави десетки заявки заради prefetch на менютата.
pub fn general_layer(secret: &str) -> GovernorLayer<ClientKeyExtractor, NoOpMiddleware, Body> {
    let config = GovernorConfigBuilder::default()
        .per_millisecond(50)
        .burst_size(300)
        .key_extractor(ClientKeyExtractor {
            secret: secret.to_string(),
        })
        .finish()
        .expect("governor config");
    GovernorLayer::new(Arc::new(config))
}

/// По-строг лимит за опитите за вход и парола, на IP:
/// 30 накуп и по 1 на 500мс (2/сек).
pub fn auth_layer() -> GovernorLayer<PeerIpKeyExtractor, NoOpMiddleware, Body> {
    let config = GovernorConfigBuilder::default()
        .per_millisecond(500)
        .burst_size(30)
        .finish()
        .expect("governor config");
    GovernorLayer::new(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct Claims {
        sub: String,
        iat: usize,
        exp: usize,
        iss: String,
        mfa: i64,
    }

    const SECRET: &str = "тест-тайна";

    fn token(sub: &str) -> String {
        let claims = Claims {
            sub: sub.into(),
            iat: 1,
            exp: 4_000_000_000,
            iss: "ytbuch".into(),
            mfa: 0,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap()
    }

    fn req(header_value: Option<&str>, addr: &str) -> Request<Body> {
        let mut req = Request::builder().body(Body::empty()).unwrap();
        if let Some(t) = header_value {
            req.headers_mut()
                .insert(header::AUTHORIZATION, t.parse().unwrap());
        }
        req.extensions_mut()
            .insert(ConnectInfo::<SocketAddr>(addr.parse().unwrap()));
        req
    }

    fn ex() -> ClientKeyExtractor {
        ClientKeyExtractor {
            secret: SECRET.to_string(),
        }
    }

    #[test]
    fn same_user_shares_one_bucket() {
        let t = token("ivan");
        let header = format!("Bearer {t}");
        let a = ex().extract(&req(Some(&header), "127.0.0.1:1")).unwrap();
        let b = ex().extract(&req(Some(&header), "127.0.0.1:2")).unwrap();
        let c = ex().extract(&req(Some(&format!("Bearer {}", token("petar"))), "127.0.0.1:1")).unwrap();
        // един и същ потребител е едно гърне, друг потребител — друго
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn without_token_key_is_ip() {
        let a = ex().extract(&req(None, "127.0.0.1:1")).unwrap();
        let b = ex().extract(&req(None, "127.0.0.1:2")).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn signed_token_shares_bucket_per_user() {
        let t = token("ivan");
        let a = ex().extract(&req(Some(&format!("Bearer {t}")), "127.0.0.1:1")).unwrap();
        let b = ex().extract(&req(Some(&format!("Bearer {t}")), "127.0.0.1:2")).unwrap();
        assert_eq!(a, b);
        assert!(matches!(a, ClientKey::User(_)));
    }

    #[test]
    fn fake_token_falls_back_to_ip() {
        // Различен текст на всяка заявка не бива да дава нов кош.
        let a = ex().extract(&req(Some("Bearer junk-1"), "127.0.0.1:1")).unwrap();
        let b = ex().extract(&req(Some("Bearer junk-2"), "127.0.0.1:1")).unwrap();
        assert_eq!(a, b);
        assert!(matches!(a, ClientKey::Ip(_)));
    }
}
