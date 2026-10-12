//! Rate limiting с tower_governor.
//! Зад Next.js проксито всички заявки идват от един IP, затова общият лимит
//! е на потребителя (JWT токен), а не на адреса — иначе всички делят едно
//! гърне и бързото обикаляне на менютата го изчерпва. Без токен (и на
//! `/v1/auth/*`) ключът е IP-то.

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

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum ClientKey {
    User(u64),
    Ip(SocketAddr),
}

/// Ключ по токена на потребителя; без токен — по IP на връзката.
/// Самият токен не се пази, само хеш.
#[derive(Clone)]
pub struct ClientKeyExtractor;

impl KeyExtractor for ClientKeyExtractor {
    type Key = ClientKey;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let token = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim();
        if !token.is_empty() {
            let mut hasher = DefaultHasher::new();
            token.hash(&mut hasher);
            return Ok(ClientKey::User(hasher.finish()));
        }
        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| ClientKey::Ip(c.0))
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Общ лимит: 300 заявки накуп и по 1 на 50мс (20/сек) на потребител.
/// Едно отваряне на екран прави десетки заявки заради prefetch на менютата.
pub fn general_layer() -> GovernorLayer<ClientKeyExtractor, NoOpMiddleware, Body> {
    let config = GovernorConfigBuilder::default()
        .per_millisecond(50)
        .burst_size(300)
        .key_extractor(ClientKeyExtractor)
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

    fn req(token: Option<&str>, addr: &str) -> Request<Body> {
        let mut req = Request::builder().body(Body::empty()).unwrap();
        if let Some(t) = token {
            req.headers_mut()
                .insert(header::AUTHORIZATION, t.parse().unwrap());
        }
        req.extensions_mut()
            .insert(ConnectInfo::<SocketAddr>(addr.parse().unwrap()));
        req
    }

    #[test]
    fn token_gives_per_user_key() {
        let ex = ClientKeyExtractor;
        let a = ex.extract(&req(Some("Bearer abc"), "127.0.0.1:1")).unwrap();
        let b = ex.extract(&req(Some("Bearer abc"), "127.0.0.1:2")).unwrap();
        let c = ex.extract(&req(Some("Bearer zzz"), "127.0.0.1:1")).unwrap();
        // един и същ токен е едно гърне, различен токен — друго
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn without_token_key_is_ip() {
        let ex = ClientKeyExtractor;
        let a = ex.extract(&req(None, "127.0.0.1:1")).unwrap();
        let b = ex.extract(&req(None, "127.0.0.1:2")).unwrap();
        assert_ne!(a, b);
    }
}
