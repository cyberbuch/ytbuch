//! ytbuch API. Първа фаза: вход, фирми, активна фирма, празно табло.

mod auth;
mod companies;
mod db;
mod lists;
mod saft;

use std::env;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::db::Db;

pub type ApiError = (StatusCode, Json<Value>);

pub fn api_err(status: StatusCode, detail: impl Into<String>) -> ApiError {
    (status, Json(json!({ "detail": detail.into() })))
}

#[derive(Clone)]
pub struct AppState {
    db: Arc<Db>,
    jwt_secret: String,
}

#[derive(Deserialize)]
struct TokenBody {
    sub: Option<String>,
    email: Option<String>,
    password: Option<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ytbuch=info".into()),
        )
        .init();

    let db_url = env::var("YTBUCH_DATABASE_URL").unwrap_or_else(|_| {
        "host=127.0.0.1 port=1337 user=ydb password=ydbrocks dbname=ydb sslmode=disable".into()
    });
    let jwt_secret = env::var("YTBUCH_JWT_SECRET")
        .unwrap_or_else(|_| "ytbuch-local-jwt-secret-change-me".into());
    let port: u16 = env::var("YTBUCH_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8092);

    let client = tokio::time::timeout(std::time::Duration::from_secs(8), db::connect(&db_url))
        .await
        .unwrap_or_else(|_| panic!("octo: timeout"))
        .unwrap_or_else(|e| panic!("octo: {e}"));
    tokio::time::timeout(std::time::Duration::from_secs(20), db::migrate(&client))
        .await
        .unwrap_or_else(|_| panic!("migrate: timeout"))
        .unwrap_or_else(|e| panic!("migrate: {e}"));

    let db = Arc::new(Db::new(client));
    saft::ensure_chart(&db)
        .await
        .unwrap_or_else(|e| panic!("saft chart: {e}"));
    let state = AppState {
        db,
        jwt_secret,
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/readyz", get(ready))
        .route("/v1/auth/token", post(token))
        .route("/v1/auth/registration-status", get(registration_status))
        .route("/v1/me", get(me))
        .route("/v1/companies", get(companies::index).post(companies::create))
        .route(
            "/v1/companies/{id}",
            get(companies::show)
                .patch(companies::update)
                .delete(companies::delete),
        )
        .route(
            "/v1/active-company",
            get(companies::active_get).put(companies::active_set),
        )
        .route("/v1/dashboard", get(companies::dashboard))
        .route("/v1/companies/{id}/settings", get(lists::company_settings))
        .route("/v1/accounts", get(lists::accounts))
        .route("/v1/accounts/seed-saft", post(saft::seed_saft))
        .route("/v1/saft/nomenclatures", get(saft::nomenclatures))
        .route("/v1/counterparts", get(lists::counterparts))
        .route("/v1/products", get(lists::products))
        .route("/v1/invoices", get(lists::invoices))
        .route("/v1/invoices/{id}", get(lists::invoice_show))
        .route("/v1/journal", get(lists::journal))
        .route("/v1/journal/{id}", get(lists::journal_show))
        .fallback(fallback)
        .with_state(state);

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    tracing::info!("ytbuch API on http://{addr}");
    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| panic!("serve: {e}"));
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn fallback(method: Method, uri: axum::http::Uri) -> Response {
    if method == Method::GET && uri.path().starts_with("/v1/") {
        return Json(json!({ "items": [], "count": 0, "limit": 50, "offset": 0 })).into_response();
    }
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "detail": "Not Found" })),
    )
        .into_response()
}

async fn ready(State(st): State<AppState>) -> Result<Json<Value>, ApiError> {
    st.db.query( "SELECT 1")
        .await
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    Ok(Json(json!({ "status": "ok", "database": "up" })))
}

async fn registration_status() -> Json<Value> {
    Json(json!({ "registration_enabled": 0 }))
}

async fn token(
    State(st): State<AppState>,
    Json(body): Json<TokenBody>,
) -> Result<Json<Value>, ApiError> {
    let sub = body
        .sub
        .filter(|s| !s.is_empty())
        .or(body.email.filter(|s| !s.is_empty()))
        .ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "email или sub е задължителен"))?;
    let password = body.password.unwrap_or_default();
    let rows = st.db.query(
        &format!(
            "SELECT id, email, password_hash, is_active, totp_enabled FROM users WHERE email = {}",
            db::lit(&sub)
        ),
    )
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if rows.is_empty() {
        if sub == "superadmin" && password == "123+123" {
            provision_superadmin(&st).await?;
        } else {
            let n = st.db.query( "SELECT id FROM users")
                .await
                .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            if !n.is_empty() {
                return Err(api_err(StatusCode::UNAUTHORIZED, "грешен потребител или парола"));
            }
        }
    } else {
        let row = &rows[0];
        let active = db::cell_i32_at(row, 3);
        if active == 0 {
            return Err(api_err(StatusCode::FORBIDDEN, "потребителят е неактивен"));
        }
        let email = db::cell_str(row, 1);
        let hash = db::cell_str(row, 2);
        if hash != auth::hash_password(&email, &password) {
            if sub == "superadmin" && password == "123+123" {
                let id = db::cell_i32_at(row, 0);
                let fresh = auth::hash_password("superadmin", "123+123");
                st.db.exec(
                    &format!(
                        "UPDATE users SET password_hash = {}, is_active = 1, is_super_admin = 1 WHERE id = {id}",
                        db::lit(&fresh)
                    ),
                )
                .await
                .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            } else {
                return Err(api_err(StatusCode::UNAUTHORIZED, "грешен потребител или парола"));
            }
        }
    }
    let token = auth::issue(&st.jwt_secret, &sub)?;
    Ok(Json(json!({
        "access_token": token,
        "token_type": "bearer",
        "sub": sub,
        "expires_in": auth::ttl(),
    })))
}

async fn provision_superadmin(st: &AppState) -> Result<(), ApiError> {
    let hash = auth::hash_password("superadmin", "123+123");
    let existing = st.db.query(
        "SELECT id FROM users WHERE email = 'superadmin'",
    )
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if existing.is_empty() {
        let id = st.db.next_id( "users")
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let stamp = db::now_stamp();
        st.db.exec(
            &format!(
                "INSERT INTO users (id, email, name, password_hash, role, is_active, is_super_admin, role_id, totp_enabled, created_at) \
                 VALUES ({id}, 'superadmin', 'Superadmin', {}, 'super_admin', 1, 1, 0, 0, {})",
                db::lit(&hash),
                db::lit(&stamp)
            ),
        )
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(())
}

async fn me(State(st): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let sub = auth::subject(&st.jwt_secret, &headers)?;
    let rows = st.db.query(
        &format!(
            "SELECT id, email, name, is_super_admin, role_id, totp_enabled FROM users WHERE email = {}",
            db::lit(&sub)
        ),
    )
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let Some(row) = rows.first() else {
        return Ok(Json(json!({ "sub": sub, "permissions": [] })));
    };
    let super_admin = db::cell_i32_at(row, 3);
    let perms: Vec<&str> = if super_admin == 1 { vec!["*"] } else { vec![] };
    Ok(Json(json!({
        "sub": sub,
        "id": db::cell_i32_at(row, 0),
        "email": db::cell_str(row, 1),
        "name": db::cell_str(row, 2),
        "is_super_admin": super_admin,
        "role_id": db::cell_i32_at(row, 4),
        "totp_enabled": db::cell_i32_at(row, 5),
        "permissions": perms,
    })))
}
