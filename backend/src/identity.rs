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
    /// Версия на сесията. Расте при всяка смяна на паролата — токените,
    /// издадени преди нея, отпадат.
    pub ver: i64,
    /// Неуспешни опити за 2FA код.
    pub mfa_fails: i32,
    /// Unix момент до който 2FA е заключен.
    pub mfa_lock_until: i64,
}

const SEL: &str = "SELECT u.id, u.email, u.name, u.password_hash, u.is_active, u.is_super_admin, u.role_id, \
                  u.totp_enabled, s.ver, s.mfa_fails, s.mfa_lock_until \
                  FROM users u LEFT JOIN user_session s ON s.id = u.id";

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
        ver: cell_i32_at(row, 8) as i64,
        mfa_fails: cell_i32_at(row, 9),
        mfa_lock_until: cell_i32_at(row, 10) as i64,
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
        .query(&format!("{SEL} WHERE u.id = {id}"))
        .await
        .map_err(db_err)?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(with_login(db, from_row(row)).await?))
}

pub async fn by_email(db: &Db, email: &str) -> Result<Option<User>, ApiError> {
    let rows = db
        .query(&format!("{SEL} WHERE u.email = {}", lit(email)))
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
/// Смяна на паролата. Връща новата версия на сесията — токените, издадени
/// преди смяната, отпадат.
pub async fn set_password(db: &Db, id: i32, _email: &str, password: &str) -> Result<i64, ApiError> {
    let hash = auth::hash_new(password)?;
    db.exec(&format!(
        "UPDATE users SET password_hash = {} WHERE id = {id}",
        lit(&hash)
    ))
    .await
    .map_err(db_err)?;
    bump_version(db, id).await
}

/// Пренаписване на хеша при смяна на стария sha256 формат. Паролата не се
/// променя, затова и старите токени остават валидни.
pub async fn rehash_password(db: &Db, id: i32, password: &str) -> Result<(), ApiError> {
    let hash = auth::hash_new(password)?;
    db.exec(&format!(
        "UPDATE users SET password_hash = {} WHERE id = {id}",
        lit(&hash)
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

/// Расти версията на сесията. Токените, издадени преди смяната на
/// паролата, отпадат — иначе откраднат токен би работил и след нея.
pub async fn bump_version(db: &Db, user_id: i32) -> Result<i64, ApiError> {
    let ver = session_row(db, user_id).await?.0 + 1;
    save_session(db, user_id, Some(ver), None, None).await?;
    Ok(ver)
}

/// Брои неуспешните опити за 2FA код и връща новия брой.
pub async fn mfa_failure(db: &Db, user_id: i32) -> Result<i32, ApiError> {
    let fails = session_row(db, user_id).await?.1 + 1;
    save_session(db, user_id, None, Some(fails), None).await?;
    Ok(fails)
}

/// Заключва 2FA до даден момент (unix секунди).
pub async fn mfa_lock(db: &Db, user_id: i32, until: i64) -> Result<(), ApiError> {
    save_session(db, user_id, None, Some(0), Some(until)).await
}

/// Изчистява брояча и заключването след успешен вход с код.
pub async fn mfa_reset(db: &Db, user_id: i32) -> Result<(), ApiError> {
    save_session(db, user_id, None, Some(0), Some(0)).await
}

async fn session_row(db: &Db, user_id: i32) -> Result<(i64, i32, i64), ApiError> {
    let rows = db
        .query(&format!(
            "SELECT ver, mfa_fails, mfa_lock_until FROM user_session WHERE id = {user_id}"
        ))
        .await
        .map_err(db_err)?;
    Ok(match rows.first() {
        Some(row) => (
            cell_i32_at(row, 0) as i64,
            cell_i32_at(row, 1),
            cell_i32_at(row, 2) as i64,
        ),
        None => (0, 0, 0),
    })
}

/// Запис на състоянието на сесията. `None` означава „остави както е“.
async fn save_session(
    db: &Db,
    user_id: i32,
    ver: Option<i64>,
    fails: Option<i32>,
    lock: Option<i64>,
) -> Result<(), ApiError> {
    let (cur_ver, cur_fails, cur_lock) = session_row(db, user_id).await?;
    let ver = ver.unwrap_or(cur_ver);
    let fails = fails.unwrap_or(cur_fails);
    let lock = lock.unwrap_or(cur_lock);
    let found = db
        .query(&format!("SELECT id FROM user_session WHERE id = {user_id}"))
        .await
        .map_err(db_err)?;
    if found.first().is_some() {
        db.exec(&format!(
            "UPDATE user_session SET ver = {ver}, mfa_fails = {fails}, \
             mfa_lock_until = {lock} WHERE id = {user_id}"
        ))
        .await
        .map_err(db_err)?;
    } else {
        db.exec(&format!(
            "INSERT INTO user_session (id, user_id, ver, mfa_fails, mfa_lock_until) \
             VALUES ({user_id}, {user_id}, {ver}, {fails}, {lock})"
        ))
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

pub async fn set_email(db: &Db, id: i32, email: &str, password: &str) -> Result<i64, ApiError> {
    let hash = auth::hash_new(password)?;
    db.exec(&format!(
        "UPDATE users SET email = {}, password_hash = {} WHERE id = {id}",
        lit(email),
        lit(&hash)
    ))
    .await
    .map_err(db_err)?;
    bump_version(db, id).await
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
