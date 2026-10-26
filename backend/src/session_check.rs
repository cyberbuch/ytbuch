//! Проверка на сесиите: смяната на паролата трябва да приключи старите токени.
//! Тестът върви срещу жива Octo; при липса на база се пропуска.

use std::sync::Arc;

use axum::http::{header, HeaderMap};

use crate::auth;
use crate::db::{self, lit, Db};
use crate::identity;

const USER: &str = "session-test";
const SECRET: &str = "session-check-secret";

async fn octo() -> Option<Arc<Db>> {
    let url = std::env::var("YTBUCH_DATABASE_URL").unwrap_or_else(|_| {
        "host=127.0.0.1 port=1337 user=ydb password=ydbrocks dbname=ydb sslmode=disable".into()
    });
    match tokio::time::timeout(std::time::Duration::from_secs(5), Db::connect(&url, 2)).await {
        Ok(Ok(db)) => Some(Arc::new(db)),
        _ => {
            eprintln!("SKIP: няма Octo на YTBUCH_DATABASE_URL");
            None
        }
    }
}

fn bearer(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let value = format!("Bearer {token}").parse().expect("bearer");
    headers.insert(header::AUTHORIZATION, value);
    headers
}

/// Чисти тестовия потребител от предишен прекъснат тест.
async fn purge(db: &Db) {
    let rows = db
        .query(&format!("SELECT id FROM users WHERE email = {}", lit(USER)))
        .await
        .expect("потребители");
    for row in &rows {
        let id = db::cell_i32_at(row, 0);
        let _ = db.exec(&format!("DELETE FROM user_login WHERE id = {id}")).await;
        let _ = db.exec(&format!("DELETE FROM user_session WHERE id = {id}")).await;
        let _ = db.exec(&format!("DELETE FROM users WHERE id = {id}")).await;
    }
}

async fn make_user(db: &Db) -> i32 {
    purge(db).await;
    let id = db.next_id("users").await.expect("id на потребител");
    let hash = auth::hash_new("стара-парола-123").expect("хеш");
    db.exec(&format!(
        "INSERT INTO users (id, email, name, password_hash, role, is_active, is_super_admin, role_id, totp_enabled, created_at) \
         VALUES ({id}, {}, 'Session Test', {}, 'accountant', 1, 0, 0, 0, {})",
        lit(USER),
        lit(&hash),
        lit(&db::now_stamp())
    ))
    .await
    .expect("потребител");
    id
}

#[tokio::test]
async fn password_change_ends_old_session() {
    let Some(db) = octo().await else { return };
    let id = make_user(&db).await;

    let user = identity::by_id(&db, id).await.expect("потребител").expect("има потребител");
    assert_eq!(user.ver, 0, "нов потребител започва с версия 0");

    let token = auth::issue(SECRET, USER, user.ver).expect("токен");
    let claims = auth::claims(SECRET, &bearer(&token)).expect("твърдения");
    assert_eq!(claims.ver, 0);

    let new_ver = identity::set_password(&db, id, USER, "нова-парола-123")
        .await
        .expect("смяна на парола");
    assert_eq!(new_ver, 1, "смяната на паролата расти версията");

    let user = identity::by_id(&db, id).await.expect("потребител").expect("има потребител");
    assert_eq!(user.ver, 1);

    // Старият токен носи старата версия и gate-ът го отхвърля.
    let old = auth::claims(SECRET, &bearer(&token)).expect("твърдения");
    assert_ne!(old.ver, user.ver, "старият токен не бива да важи");

    // Новият токен с текущата версия минава.
    let fresh = auth::issue(SECRET, USER, user.ver).expect("токен");
    let now = auth::claims(SECRET, &bearer(&fresh)).expect("твърдения");
    assert_eq!(now.ver, user.ver, "новият токен трябва да важи");

    purge(&db).await;
}
