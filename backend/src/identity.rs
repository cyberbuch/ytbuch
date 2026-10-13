//! Потребител: стабилно име за вход и отделен имейл.
//! Octo няма ALTER TABLE, затова тайната на 2FA е в `user_login`.

use crate::auth;
use crate::db::{self, cell_i32_at, cell_str, lit, Db};
use crate::form::db_err;
use crate::ApiError;

pub struct User {
    pub id: i32,
    pub email: String,
    pub username: String,
    pub name: String,
    pub password_hash: String,
    pub is_active: i32,
    pub is_super_admin: i32,
    pub role_id: i32,
    pub totp_enabled: i32,
    pub totp_secret: String,
    pub totp_pending: String,
}

const SEL: &str = "SELECT id, email, name, password_hash, is_active, is_super_admin, role_id, totp_enabled FROM users";

fn from_row(row: &db::Row) -> User {
    let email = cell_str(row, 1);
    User {
        id: cell_i32_at(row, 0),
        email: email.clone(),
        username: email,
        name: cell_str(row, 2),
        password_hash: cell_str(row, 3),
        is_active: cell_i32_at(row, 4),
        is_super_admin: cell_i32_at(row, 5),
        role_id: cell_i32_at(row, 6),
        totp_enabled: cell_i32_at(row, 7),
        totp_secret: String::new(),
        totp_pending: String::new(),
    }
}

async fn with_login(db: &Db, mut user: User) -> Result<User, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT username, totp_secret_b32, totp_pending_b32 FROM user_login WHERE id = {}",
            user.id
        ))
        .await
        .map_err(db_err)?;
    if let Some(row) = rows.first() {
        let username = cell_str(row, 0);
        if !username.is_empty() {
            user.username = username;
        }
        user.totp_secret = cell_str(row, 1);
        user.totp_pending = cell_str(row, 2);
    }
    Ok(user)
}

pub async fn by_id(db: &Db, id: i32) -> Result<Option<User>, ApiError> {
    let rows = db
        .query(&format!("{SEL} WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(with_login(db, from_row(row)).await?))
}

pub async fn by_email(db: &Db, email: &str) -> Result<Option<User>, ApiError> {
    let rows = db
        .query(&format!("{SEL} WHERE email = {}", lit(email)))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(with_login(db, from_row(row)).await?))
}

pub async fn by_login(db: &Db, login: &str) -> Result<Option<User>, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT id FROM user_login WHERE username = {}",
            lit(login)
        ))
        .await
        .map_err(db_err)?;
    if let Some(row) = rows.first() {
        return by_id(db, cell_i32_at(row, 0)).await;
    }
    by_email(db, login).await
}

pub async fn user_count(db: &Db) -> Result<i32, ApiError> {
    let rows = db.query("SELECT id FROM users").await.map_err(db_err)?;
    Ok(rows.len() as i32)
}

pub async fn is_super(db: &Db, login: &str) -> Result<bool, ApiError> {
    Ok(match by_login(db, login).await? {
        Some(u) => u.is_super_admin == 1,
        None => false,
    })
}

pub async fn provision_superadmin(db: &Db) -> Result<(), ApiError> {
    let hash = auth::hash_new("123+123")?;
    let existing = db
        .query("SELECT id FROM users WHERE email = 'superadmin'")
        .await
        .map_err(db_err)?;
    if !existing.is_empty() {
        return Ok(());
    }
    let id = db.next_id("users").await.map_err(db_err)?;
    db.exec(&format!(
        "INSERT INTO users (id, email, name, password_hash, role, is_active, is_super_admin, role_id, totp_enabled, created_at) \
         VALUES ({id}, 'superadmin', 'Superadmin', {}, 'super_admin', 1, 1, 0, 0, {})",
        lit(&hash),
        lit(&db::now_stamp())
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn ensure_login(db: &Db, id: i32, username: &str) -> Result<(), ApiError> {
    let rows = db
        .query(&format!("SELECT id FROM user_login WHERE id = {id}"))
        .await
        .map_err(db_err)?;
    if rows.is_empty() {
        db.exec(&format!(
            "INSERT INTO user_login (id, username, totp_secret_b32, totp_pending_b32) VALUES ({id}, {}, '', '')",
            lit(username)
        ))
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

/// Имейлът е наследен от стария sha256 хеш; argon2 не го ползва,
/// но сигнатурата остава, за да не се менят извикващите.
pub async fn set_password(db: &Db, id: i32, _email: &str, password: &str) -> Result<(), ApiError> {
    let hash = auth::hash_new(password)?;
    db.exec(&format!(
        "UPDATE users SET password_hash = {} WHERE id = {id}",
        lit(&hash)
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn set_email(db: &Db, id: i32, email: &str, password: &str) -> Result<(), ApiError> {
    let hash = auth::hash_new(password)?;
    db.exec(&format!(
        "UPDATE users SET email = {}, password_hash = {} WHERE id = {id}",
        lit(email),
        lit(&hash)
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn set_name(db: &Db, id: i32, name: &str) -> Result<(), ApiError> {
    db.exec(&format!(
        "UPDATE users SET name = {} WHERE id = {id}",
        lit(name)
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn set_totp(db: &Db, id: i32, enabled: i32, secret: &str, pending: &str) -> Result<(), ApiError> {
    db.exec(&format!(
        "UPDATE users SET totp_enabled = {enabled} WHERE id = {id}"
    ))
    .await
    .map_err(db_err)?;
    db.exec(&format!(
        "UPDATE user_login SET totp_secret_b32 = {}, totp_pending_b32 = {} WHERE id = {id}",
        lit(secret),
        lit(pending)
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn email_taken(db: &Db, email: &str, except_id: i32) -> Result<bool, ApiError> {
    let users = db
        .query(&format!(
            "SELECT id FROM users WHERE email = {} AND id <> {except_id}",
            lit(email)
        ))
        .await
        .map_err(db_err)?;
    if !users.is_empty() {
        return Ok(true);
    }
    let names = db
        .query(&format!(
            "SELECT id FROM user_login WHERE username = {} AND id <> {except_id}",
            lit(email)
        ))
        .await
        .map_err(db_err)?;
    Ok(!names.is_empty())
}

pub async fn wipe_recovery(db: &Db, user_id: i32) -> Result<(), ApiError> {
    db.exec(&format!(
        "DELETE FROM user_recovery_codes WHERE user_id = {user_id}"
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn insert_recovery(db: &Db, user_id: i32, hashes: &[String]) -> Result<(), ApiError> {
    for hash in hashes {
        let id = db.next_id("user_recovery_codes").await.map_err(db_err)?;
        db.exec(&format!(
            "INSERT INTO user_recovery_codes (id, user_id, code_hash, used) VALUES ({id}, {user_id}, {}, 0)",
            lit(hash)
        ))
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

/// Връща истина и маркира кода като използван, ако съвпада с неизползван хеш.
pub async fn take_recovery(db: &Db, user_id: i32, code: &str) -> Result<bool, ApiError> {
    let want = crate::totp::recovery_hash(code);
    if want.is_empty() {
        return Ok(false);
    }
    let rows = db
        .query(&format!(
            "SELECT id, code_hash, used FROM user_recovery_codes WHERE user_id = {user_id}"
        ))
        .await
        .map_err(db_err)?;
    for row in &rows {
        if cell_i32_at(row, 2) != 0 {
            continue;
        }
        if cell_str(row, 1) == want {
            let id = cell_i32_at(row, 0);
            db.exec(&format!(
                "UPDATE user_recovery_codes SET used = 1 WHERE id = {id}"
            ))
            .await
            .map_err(db_err)?;
            return Ok(true);
        }
    }
    Ok(false)
}
