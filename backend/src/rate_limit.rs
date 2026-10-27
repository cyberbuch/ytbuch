//! Rate limiting с tower_governor.
//! Зад Next.js проксито всички заявки идват от един IP, затова общият лимит
//! е на потребителя (JWT токен), а не на адреса — иначе всички делят едно
//! гърне и бързото обикаляне на менютата го изчерпва. Без токен (и на
//! `/v1/auth/*`) ключът е IP-то.
//! Токенът се проверява по подпис: иначе всеки нов текст в Authorization
//! би бил нов кош и лимитът се заобикаля с нов заглавен блок на всяка заявка.
//! Зад прокси клиентският адрес идва от `X-Forwarded-For`, но само когато
//! връзката е от доверена (частна/loopback) мрежа — иначе всеки би си
//! измислил нов адрес и нов кош.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request};
use governor::middleware::NoOpMiddleware;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::{GovernorError, GovernorLayer};

use crate::auth;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum ClientKey {
    User(u64),
    Ip(IpAddr),
}

/// Клиентският IP адрес. От доверен прокси (частна/loopback връзка) се
/// ползва последното добавено в `X-Forwarded-For` — него го е записал
/// проксито, не клиентът. Иначе — адресът на връзката.
pub fn client_ip<T>(req: &Request<T>) -> Option<IpAddr> {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>()?.0;
    if trusted_peer(peer.ip()) {
        if let Some(ip) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|xff| xff.split(',').map(str::trim).rfind(|s| !s.is_empty()))
            .and_then(|s| s.parse::<IpAddr>().ok())
        {
            return Some(ip);
        }
    }
    Some(peer.ip())
}

fn trusted_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local(),
    }
}

/// Ключ по потребителя от проверен токен; без такъв — по IP на клиента.
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
        Ok(ClientKey::Ip(
            client_ip(req).ok_or(GovernorError::UnableToExtractKey)?,
        ))
    }
}

/// Ключ само по клиентски IP — за `/v1/auth/*`, където няма смисъл от токен
/// и той не бива да дава нов кош.
#[derive(Clone)]
pub struct ClientIpExtractor;

impl KeyExtractor for ClientIpExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        client_ip(req).ok_or(GovernorError::UnableToExtractKey)
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

/// По-строг лимит за опитите за вход и парола, на клиента:
/// 30 накуп и по 1 на 500мс (2/сек).
pub fn auth_layer() -> GovernorLayer<ClientIpExtractor, NoOpMiddleware, Body> {
    let config = GovernorConfigBuilder::default()
        .per_millisecond(500)
        .burst_size(30)
        .key_extractor(ClientIpExtractor)
        .finish()
        .expect("governor config");
    GovernorLayer::new(Arc::new(config))
}

/// Заключване по акаунт при неуспешен вход — ключът е опитаният login,
/// независимо дали съществува. Еднакво поведение и за двата случая,
/// затова не издава кои акаунти има. Пази се в паметта на процеса.
const MAX_FAILS: u32 = 10;
const LOCK_FOR: Duration = Duration::from_secs(15 * 60);
const MAX_TRACKED: usize = 10_000;

struct AccountState {
    fails: u32,
    locked_until: Option<Instant>,
}

fn account_map() -> &'static Mutex<HashMap<String, AccountState>> {
    static MAP: OnceLock<Mutex<HashMap<String, AccountState>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_map<R>(f: impl FnOnce(&mut HashMap<String, AccountState>) -> R) -> R {
    let mut map = account_map().lock().expect("login throttle");
    if map.len() > MAX_TRACKED {
        let now = Instant::now();
        map.retain(|_, s| s.locked_until.is_none_or(|until| until > now));
    }
    f(&mut map)
}

/// Оставащо време до отключване, ако акаунтът е заключен.
pub fn login_locked(login: &str) -> Option<Duration> {
    let key = login.trim().to_lowercase();
    with_map(|map| {
        map.get(&key)
            .and_then(|s| s.locked_until)
            .and_then(|until| until.checked_duration_since(Instant::now()))
    })
}

/// Регистрира неуспешен опит и заключва при твърде много такива.
pub fn login_failure(login: &str) {
    let key = login.trim().to_lowercase();
    with_map(|map| {
        let state = map.entry(key).or_insert(AccountState {
            fails: 0,
            locked_until: None,
        });
        state.fails += 1;
        if state.fails >= MAX_FAILS {
            state.locked_until = Some(Instant::now() + LOCK_FOR);
        }
    });
}

/// Успешен вход чисти брояча.
pub fn login_success(login: &str) {
    let key = login.trim().to_lowercase();
    with_map(|map| map.remove(&key));
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

    fn req_at(addr: &str, xff: Option<&str>) -> Request<Body> {
        let mut req = Request::builder().body(Body::empty()).unwrap();
        if let Some(v) = xff {
            req.headers_mut()
                .insert("x-forwarded-for", v.parse().unwrap());
        }
        req.extensions_mut()
            .insert(ConnectInfo::<SocketAddr>(addr.parse().unwrap()));
        req
    }

    fn req(header_value: Option<&str>, addr: &str) -> Request<Body> {
        let mut req = req_at(addr, None);
        if let Some(t) = header_value {
            req.headers_mut()
                .insert(header::AUTHORIZATION, t.parse().unwrap());
        }
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
        let b = ex().extract(&req(Some(&header), "127.0.0.2:1")).unwrap();
        let c = ex().extract(&req(Some(&format!("Bearer {}", token("petar"))), "127.0.0.1:1")).unwrap();
        // един и същ потребител е едно гърне, друг потребител — друго
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn without_token_key_is_client_ip() {
        let a = ex().extract(&req(None, "127.0.0.1:1")).unwrap();
        let b = ex().extract(&req(None, "127.0.0.1:2")).unwrap();
        let c = ex().extract(&req(None, "127.0.0.2:1")).unwrap();
        // един IP е едно гърне, друг IP — друго
        assert_eq!(a, b);
        assert_ne!(a, c);
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

    #[test]
    fn forwarded_for_is_used_behind_trusted_proxy() {
        // Зад прокси клиентският адрес е последното добавено в X-Forwarded-For.
        let a = ex().extract(&req_at("172.18.0.5:4000", Some("203.0.113.7"))).unwrap();
        let b = ex().extract(&req_at("172.18.0.5:4001", Some("203.0.113.7"))).unwrap();
        let c = ex().extract(&req_at("172.18.0.5:4002", Some("203.0.113.8"))).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(matches!(a, ClientKey::Ip(ip) if ip == "203.0.113.7".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn spoofed_forwarded_for_is_ignored_from_public_peer() {
        // Директна връзка от публичен адрес: заглавието не се вярва.
        let a = ex().extract(&req_at("8.8.8.8:1", Some("203.0.113.7"))).unwrap();
        let b = ex().extract(&req_at("8.8.8.8:2", Some("203.0.113.8"))).unwrap();
        assert_eq!(a, b);
        assert!(matches!(a, ClientKey::Ip(ip) if ip == "8.8.8.8".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn spoofed_prefix_cannot_change_key_behind_proxy() {
        // Клиент праща своя X-Forwarded-For; проксито добавя отзад истинския.
        let a = ex().extract(&req_at("172.18.0.5:4000", Some("1.2.3.4, 203.0.113.7"))).unwrap();
        let b = ex().extract(&req_at("172.18.0.5:4001", Some("5.6.7.8, 203.0.113.7"))).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn login_lockout_is_per_account_and_uniform() {
        let victim = "има-такъв@пример.bg";
        let other = "няма-такъв@пример.bg";
        for _ in 0..9 {
            login_failure(victim);
        }
        assert!(login_locked(victim).is_none());
        login_failure(victim);
        let locked = login_locked(victim);
        assert!(locked.is_some_and(|d| d > Duration::from_secs(14 * 60)));
        // заключването не зависи от съществуването на акаунта
        for _ in 0..10 {
            login_failure(other);
        }
        assert!(login_locked(other).is_some());
        // успешен вход чисти брояча
        login_success(other);
        assert!(login_locked(other).is_none());
        login_success(victim);
        assert!(login_locked(victim).is_none());
    }
}
