//! Потребители към фирми. Членството дава роля, ролята дава права
//! само в рамките на активната фирма. Супер администраторът е навсякъде.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::db::{self, cell_i32_at, cell_str, lit, Db};
use crate::form::{self, auth_sub, db_err};
use crate::pagination::{paginate, PageQuery};
use crate::{access, api_err, auth, identity, AppState, ApiError};

pub const ROLE_ADMIN: &str = "Администратор на фирма";
pub const ROLE_ACCOUNTANT: &str = "Счетоводител";
pub const ROLE_FACTURIST: &str = "Фактурист";

const ADMIN_PERMS: &str = r#"["companies","accounts","counterparts","products","invoices","scans","journal","periods","vat","banking","assets","stock","production","dividends","reports","saft","users","rates"]"#;
const ACCOUNTANT_PERMS: &str = r#"["companies","accounts","counterparts","products","invoices","scans","journal","periods","vat","banking","assets","stock","production","dividends","reports","saft","rates"]"#;
const FACTURIST_PERMS: &str = r#"["invoices","counterparts","products","scans"]"#;

/// Трите роли на фирма се създават при старт, ако липсват.
pub async fn ensure_roles(db: &Db) -> Result<(), ApiError> {
    let seeds: [(&str, &str, &str); 3] = [
        (
            ROLE_ADMIN,
            "Пълен достъп до данните на фирмата, включително нейните потребители",
            ADMIN_PERMS,
        ),
        (
            ROLE_ACCOUNTANT,
            "Счетоводни модули на фирмата без управление на потребители",
            ACCOUNTANT_PERMS,
        ),
        (
            ROLE_FACTURIST,
            "Фактури, контрагенти и стоки на фирмата",
            FACTURIST_PERMS,
        ),
    ];
    for (name, desc, perms) in seeds {
        let rows = db
            .query(&format!("SELECT id FROM roles WHERE name = {}", lit(name)))
            .await
            .map_err(db_err)?;
        if !rows.is_empty() {
            continue;
        }
        let id = db.next_id("roles").await.map_err(db_err)?;
        db.exec(&format!(
            "INSERT INTO roles (id, name, description, permissions, created_at) VALUES ({id}, {}, {}, {}, {})",
            lit(name),
            lit(desc),
            lit(perms),
            lit(&db::now_stamp())
        ))
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

pub async fn role_id_by_name(db: &Db, name: &str) -> Result<i32, ApiError> {
    let rows = db
        .query(&format!("SELECT id FROM roles WHERE name = {}", lit(name)))
        .await
        .map_err(db_err)?;
    rows.first()
        .map(|r| cell_i32_at(r, 0))
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("липсва роля {name}")))
}

/// Еднократно: съществуващите потребители получават администраторска
/// роля във всички фирми, за да не останат без достъп след ъпгрейда.
pub async fn backfill(db: &Db) -> Result<(), ApiError> {
    let marked = db
        .query("SELECT id FROM schema_migrations WHERE id = 4")
        .await
        .map_err(db_err)?;
    if !marked.is_empty() {
        return Ok(());
    }
    let admin = role_id_by_name(db, ROLE_ADMIN).await?;
    let users = db
        .query("SELECT id FROM users WHERE is_super_admin = 0")
        .await
        .map_err(db_err)?;
    let companies = db
        .query("SELECT id FROM companies")
        .await
        .map_err(db_err)?;
    for u in &users {
        let uid = cell_i32_at(u, 0);
        for c in &companies {
            let cid = cell_i32_at(c, 0);
            if membership_role(db, uid, cid).await?.is_none() {
                grant(db, uid, cid, admin).await?;
            }
        }
    }
    db.exec("INSERT INTO schema_migrations (id, name) VALUES (4, 'user_companies')")
        .await
        .map_err(db_err)?;
    Ok(())
}

pub async fn membership_role(db: &Db, user_id: i32, company_id: i32) -> Result<Option<i32>, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT role_id FROM user_companies WHERE user_id = {user_id} AND company_id = {company_id}"
        ))
        .await
        .map_err(db_err)?;
    Ok(rows.first().map(|r| cell_i32_at(r, 0)))
}

pub async fn grant(db: &Db, user_id: i32, company_id: i32, role_id: i32) -> Result<(), ApiError> {
    let existing = db
        .query(&format!(
            "SELECT id FROM user_companies WHERE user_id = {user_id} AND company_id = {company_id}"
        ))
        .await
        .map_err(db_err)?;
    if let Some(row) = existing.first() {
        let id = cell_i32_at(row, 0);
        db.exec(&format!(
            "UPDATE user_companies SET role_id = {role_id} WHERE id = {id}"
        ))
        .await
        .map_err(db_err)?;
        return Ok(());
    }
    let id = db.next_id("user_companies").await.map_err(db_err)?;
    db.exec(&format!(
        "INSERT INTO user_companies (id, user_id, company_id, role_id, created_at) VALUES ({id}, {user_id}, {company_id}, {role_id}, {})",
        lit(&db::now_stamp())
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn revoke(db: &Db, user_id: i32, company_id: i32) -> Result<(), ApiError> {
    db.exec(&format!(
        "DELETE FROM user_companies WHERE user_id = {user_id} AND company_id = {company_id}"
    ))
    .await
    .map_err(db_err)?;
    Ok(())
}

/// Фирмите, до които потребителят има достъп.
pub async fn company_ids_of(db: &Db, user_id: i32) -> Result<Vec<i32>, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT company_id FROM user_companies WHERE user_id = {user_id} ORDER BY company_id"
        ))
        .await
        .map_err(db_err)?;
    Ok(rows.iter().map(|r| cell_i32_at(r, 0)).collect())
}

/// Членствата на потребителя с името и ролята на всяка фирма.
pub async fn memberships_of(db: &Db, user_id: i32) -> Result<Vec<Value>, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT u.company_id, u.role_id, r.name, c.name FROM user_companies u \
             LEFT JOIN roles r ON u.role_id = r.id LEFT JOIN companies c ON u.company_id = c.id \
             WHERE u.user_id = {user_id} ORDER BY u.company_id"
        ))
        .await
        .map_err(db_err)?;
    Ok(rows
        .iter()
        .map(|r| {
            json!({
                "company_id": cell_i32_at(r, 0),
                "role_id": cell_i32_at(r, 1),
                "role": cell_str(r, 2),
                "company_name": cell_str(r, 3),
            })
        })
        .collect())
}

/// Правата на потребителя в рамките на дадена фирма.
pub async fn perms_for(
    db: &Db,
    user: &identity::User,
    company: i32,
) -> Result<Vec<Value>, ApiError> {
    if user.is_super_admin == 1 {
        return Ok(vec![json!("*")]);
    }
    let Some(role_id) = membership_role(db, user.id, company).await? else {
        return Ok(Vec::new());
    };
    access::role_permissions(db, role_id).await
}

/// Само супер администратор.
pub async fn require_super_admin(st: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let sub = auth_sub(st, headers)?;
    let user = identity::by_login(&st.db, &sub)
        .await?
        .ok_or_else(|| api_err(StatusCode::UNAUTHORIZED, "потребителят не съществува"))?;
    if user.is_super_admin != 1 {
        return Err(api_err(
            StatusCode::FORBIDDEN,
            "изисква се супер администратор",
        ));
    }
    Ok(())
}

/// Действията по потребители на фирма: супер администратор или
/// администратор на същата фирма (активната).
async fn company_scope(
    st: &AppState,
    headers: &HeaderMap,
    company_id: i32,
) -> Result<(String, identity::User, i32), ApiError> {
    let sub = auth_sub(st, headers)?;
    let user = identity::by_login(&st.db, &sub)
        .await?
        .ok_or_else(|| api_err(StatusCode::UNAUTHORIZED, "потребителят не съществува"))?;
    if user.is_super_admin == 1 {
        form::ensure_company(&st.db, company_id).await?;
        return Ok((sub, user, company_id));
    }
    let active = form::active_or_one(st, &sub).await?;
    if active != company_id {
        return Err(api_err(StatusCode::NOT_FOUND, "фирмата не е намерена"));
    }
    let perms = perms_for(&st.db, &user, company_id).await?;
    let admin = perms
        .iter()
        .any(|p| p.as_str() == Some("*") || p.as_str() == Some("users"));
    if !admin {
        return Err(api_err(
            StatusCode::FORBIDDEN,
            "липсва право: users",
        ));
    }
    Ok((sub, user, company_id))
}

fn member_json(r: &db::Row) -> Value {
    json!({
        "user_id": cell_i32_at(r, 0),
        "email": cell_str(r, 1),
        "name": cell_str(r, 2),
        "is_active": cell_i32_at(r, 3),
        "role_id": cell_i32_at(r, 4),
        "role": cell_str(r, 5),
        "created_at": cell_str(r, 6),
    })
}

const MEMBER_SEL: &str = "SELECT u.id, u.email, u.name, u.is_active, m.role_id, r.name, m.created_at \
 FROM user_companies m LEFT JOIN users u ON m.user_id = u.id LEFT JOIN roles r ON m.role_id = r.id";

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<i32>,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    company_scope(&st, &headers, company_id).await?;
    let rows = st
        .db
        .query(&format!(
            "{MEMBER_SEL} WHERE m.company_id = {company_id} ORDER BY u.name"
        ))
        .await
        .map_err(db_err)?;
    let items: Vec<Value> = rows.iter().map(member_json).collect();
    let mut page = paginate(items, &q);
    let role_rows = st
        .db
        .query("SELECT id, name, description FROM roles ORDER BY name")
        .await
        .map_err(db_err)?;
    if let Some(map) = page.as_object_mut() {
        map.insert(
            "roles".into(),
            json!(role_rows
                .iter()
                .map(|r| json!({
                    "id": cell_i32_at(r, 0),
                    "name": cell_str(r, 1),
                    "description": cell_str(r, 2),
                }))
                .collect::<Vec<_>>()),
        );
    }
    Ok(page)
}

pub async fn add(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(company_id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (_, _caller, _) = company_scope(&st, &headers, company_id).await?;
    let obj = form::object(&body)?;
    let email = form::required(form::str_field(obj, "email")?, "email")?;
    let email = email.trim().to_string();
    let role_id = resolve_role(&st.db, obj).await?;
    let user_id = match identity::by_login(&st.db, &email).await? {
        Some(u) => u.id,
        None => {
            let name = form::required(form::str_field(obj, "name")?, "name")?;
            let password = form::required(form::str_field(obj, "password")?, "password")?;
            if password.chars().count() < 8 {
                return Err(api_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "паролата трябва да е поне 8 символа",
                ));
            }
            create_user(&st.db, &email, name.trim(), &password).await?
        }
    };
    grant(&st.db, user_id, company_id, role_id).await?;
    Ok((StatusCode::CREATED, Json(member_one(&st.db, company_id, user_id).await?)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((company_id, user_id)): Path<(i32, i32)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    company_scope(&st, &headers, company_id).await?;
    let obj = form::object(&body)?;
    let role_id = resolve_role(&st.db, obj).await?;
    membership_role(&st.db, user_id, company_id)
        .await?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "потребителят не е във фирмата"))?;
    grant(&st.db, user_id, company_id, role_id).await?;
    Ok(Json(member_one(&st.db, company_id, user_id).await?))
}

pub async fn remove(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((company_id, user_id)): Path<(i32, i32)>,
) -> Result<StatusCode, ApiError> {
    let (_, caller, _) = company_scope(&st, &headers, company_id).await?;
    if caller.id == user_id {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "не може да махнете себе си от фирмата",
        ));
    }
    membership_role(&st.db, user_id, company_id)
        .await?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "потребителят не е във фирмата"))?;
    revoke(&st.db, user_id, company_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn resolve_role(db: &Db, obj: &serde_json::Map<String, Value>) -> Result<i32, ApiError> {
    if let Some(id) = form::i32_field(obj, "role_id")? {
        if id > 0 {
            let rows = db
                .query(&format!("SELECT id FROM roles WHERE id = {id}"))
                .await
                .map_err(db_err)?;
            if rows.is_empty() {
                return Err(api_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "ролята не е намерена",
                ));
            }
            return Ok(id);
        }
    }
    let name = form::str_field(obj, "role")?
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| ROLE_ACCOUNTANT.to_string());
    let rows = db
        .query(&format!("SELECT id FROM roles WHERE name = {}", lit(name.trim())))
        .await
        .map_err(db_err)?;
    rows.first()
        .map(|r| cell_i32_at(r, 0))
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "ролята не е намерена"))
}

async fn create_user(db: &Db, email: &str, name: &str, password: &str) -> Result<i32, ApiError> {
    if identity::email_taken(db, email, 0).await? {
        return Err(api_err(StatusCode::CONFLICT, "имейлът вече се ползва"));
    }
    let id = db.next_id("users").await.map_err(db_err)?;
    let hash = auth::hash_new(password)?;
    db.exec(&format!(
        "INSERT INTO users (id, email, name, password_hash, role, is_active, is_super_admin, role_id, totp_enabled, created_at) \
         VALUES ({id}, {}, {}, {}, 'user', 1, 0, 0, 0, {})",
        lit(email),
        lit(name),
        lit(&hash),
        lit(&db::now_stamp())
    ))
    .await
    .map_err(db_err)?;
    if let Err(e) = identity::ensure_login(db, id, email).await {
        let _ = db.exec(&format!("DELETE FROM users WHERE id = {id}")).await;
        return Err(e);
    }
    Ok(id)
}

async fn member_one(db: &Db, company_id: i32, user_id: i32) -> Result<Value, ApiError> {
    let rows = db
        .query(&format!(
            "{MEMBER_SEL} WHERE m.company_id = {company_id} AND m.user_id = {user_id}"
        ))
        .await
        .map_err(db_err)?;
    rows.first()
        .map(member_json)
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "потребителят не е във фирмата"))
}

/// Членствата за `/v1/me`.
pub async fn me_companies(db: &Db, user_id: i32) -> Result<Vec<Value>, ApiError> {
    memberships_of(db, user_id).await
}
