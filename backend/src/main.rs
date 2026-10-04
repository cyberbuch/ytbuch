//! ytbuch API. Първа фаза: вход, фирми, активна фирма, празно табло.

mod asset_ops;
mod assets;
mod auth;
mod bank_csv;
mod bank_tx;
mod banks;
mod companies;
mod counterparts;
mod db;
mod dec;
mod dividends;
mod dump;
mod form;
mod fsform;
mod identity;
mod invoice_money;
mod invoice_post;
mod idoc_print;
mod idocs;
mod invoice_print;
mod invoice_read;
mod invoice_ubl;
mod invoices;
mod lists;
mod mfg;
mod mail;
mod master;
mod opening;
mod periods;
mod posting;
mod print_load;
mod print_sheet;
mod print_words;
mod profile;
mod recipes;
mod related;
mod reports;
mod reports_book;
mod reports_corr;
mod reports_data;
mod reports_fs;
mod reports_nom;
mod reports_pdf;
mod reports_sheet;
mod reports_trial;
mod s3store;
mod saft;
mod schema5;
mod session;
mod settings;
mod stock;
mod totp;
mod vat;
mod vat_ex;
mod vat_files;
mod vat_fmt;
mod vat_line;

use std::env;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
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
        .route("/v1/auth/token", post(session::token))
        .route("/v1/auth/mfa", post(session::mfa))
        .route("/v1/auth/registration-status", get(registration_status))
        .route("/v1/me", get(session::me).patch(profile::update_name))
        .route("/v1/me/password", post(profile::change_password))
        .route("/v1/me/email", post(profile::change_email))
        .route("/v1/me/totp/start", post(profile::totp_start))
        .route("/v1/me/totp/confirm", post(profile::totp_confirm))
        .route("/v1/me/totp/disable", post(profile::totp_disable))
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
        .route("/v1/accounts", get(lists::accounts).post(master::account_create))
        .route(
            "/v1/accounts/{id}",
            get(master::account_show)
                .patch(master::account_update)
                .delete(master::account_delete),
        )
        .route("/v1/accounts/seed-saft", post(saft::seed_saft))
        .route("/v1/saft/nomenclatures", get(saft::nomenclatures))
        .route("/v1/units", get(saft::units))
        .route(
            "/v1/counterparts",
            get(counterparts::counterparts).post(counterparts::counterpart_create),
        )
        .route(
            "/v1/counterparts/{id}",
            get(counterparts::counterpart_show)
                .patch(counterparts::counterpart_update)
                .delete(counterparts::counterpart_delete),
        )
        .route("/v1/products", get(lists::products).post(master::product_create))
        .route(
            "/v1/products/{id}",
            get(master::product_show)
                .patch(master::product_update)
                .delete(master::product_delete),
        )
        .route(
            "/v1/companies/{id}/locations",
            post(related::location_create),
        )
        .route(
            "/v1/company-locations/{id}",
            patch(related::location_update).delete(related::location_delete),
        )
        .route(
            "/v1/companies/{id}/beneficial-owners",
            post(related::owner_create),
        )
        .route(
            "/v1/beneficial-owners/{id}",
            patch(related::owner_update).delete(related::owner_delete),
        )
        .route(
            "/v1/companies/{id}/ultimate-parents",
            post(related::parent_create),
        )
        .route(
            "/v1/ultimate-parents/{id}",
            patch(related::parent_update).delete(related::parent_delete),
        )
        .route(
            "/v1/companies/{id}/document-series",
            post(related::series_create),
        )
        .route(
            "/v1/document-series/{id}",
            patch(related::series_update).delete(related::series_delete),
        )
        .route(
            "/v1/system-settings",
            get(settings::index).put(settings::update),
        )
        .route("/v1/system-settings/smtp-test", post(mail::smtp_test))
        .route("/v1/system-settings/s3-test", post(s3store::s3_test))
        .route("/v1/system-settings/s3-backup", post(s3store::s3_backup))
        .route("/v1/system-settings/s3-backups", get(s3store::s3_list))
        .route("/v1/system-settings/s3-delete", post(s3store::s3_delete))
        .route("/v1/invoices", get(invoice_read::invoices).post(invoices::create))
        .route("/v1/invoices/next-number", get(invoices::next_number))
        .route("/v1/invoices/bulk/post", post(invoice_post::bulk_post))
        .route("/v1/invoices/bulk/unpost", post(invoice_post::bulk_unpost))
        .route("/v1/invoices/bulk/delete", post(invoices::bulk_delete))
        .route(
            "/v1/invoices/{id}",
            get(invoice_read::invoice_show)
                .patch(invoices::update)
                .delete(invoices::delete),
        )
        .route("/v1/invoices/{id}/post", post(invoice_post::post))
        .route("/v1/invoices/{id}/print", get(invoice_print::print))
        .route("/v1/invoices/{id}/ubl", get(invoice_ubl::export))
        .route("/v1/journal", get(lists::journal))
        .route("/v1/journal/{id}", get(lists::journal_show))
        .route("/v1/accounting-periods", get(periods::index))
        .route("/v1/accounting-periods/close", post(periods::close))
        .route("/v1/accounting-periods/reopen", post(periods::reopen))
        .route(
            "/v1/opening-balances",
            get(opening::index).post(opening::create),
        )
        .route(
            "/v1/opening-balances/{id}",
            patch(opening::update).delete(opening::delete),
        )
        .route("/v1/vat/registers", get(vat::registers))
        .route("/v1/vat/return", get(vat::registers))
        .route("/v1/vat/export", get(vat::export))
        .route("/v1/vat-exemptions", get(vat_ex::index))
        .route("/v1/bank-accounts", get(banks::index).post(banks::create))
        .route(
            "/v1/bank-accounts/{id}",
            patch(banks::update).delete(banks::delete),
        )
        .route("/v1/bank-transactions", get(bank_tx::index))
        .route("/v1/bank-transactions/preview", post(bank_tx::preview))
        .route("/v1/bank-transactions/import", post(bank_tx::import))
        .route("/v1/bank-transactions/{id}/book", post(bank_tx::book))
        .route("/v1/bank-transactions/{id}/reallocate", post(bank_tx::reallocate))
        .route("/v1/bank-transactions/{id}", delete(bank_tx::delete))
        .route(
            "/v1/fixed-asset-categories",
            get(assets::categories).post(assets::category_create),
        )
        .route(
            "/v1/fixed-asset-categories/{id}",
            patch(assets::category_update).delete(assets::category_delete),
        )
        .route("/v1/fixed-assets", get(assets::index).post(assets::create))
        .route("/v1/fixed-assets/depreciation/preview", post(asset_ops::preview))
        .route("/v1/fixed-assets/depreciation/post", post(asset_ops::post_dep))
        .route(
            "/v1/fixed-assets/{id}",
            patch(assets::update).delete(assets::delete),
        )
        .route("/v1/fixed-assets/{id}/revalue", post(asset_ops::revalue))
        .route("/v1/fixed-assets/{id}/move", post(asset_ops::move_asset))
        .route("/v1/fixed-assets/{id}/conserve", post(asset_ops::conserve))
        .route("/v1/fixed-assets/{id}/end-conserve", post(asset_ops::end_conserve))
        .route("/v1/fixed-assets/{id}/dispose", post(asset_ops::dispose))
        .route("/v1/fixed-assets/{id}/events", get(asset_ops::events))
        .route("/v1/inventory/at-location", get(stock::at_location))
        .route("/v1/internal-docs/next-number", get(idocs::next_number))
        .route("/v1/internal-docs", get(idocs::index).post(idocs::create))
        .route(
            "/v1/internal-docs/{id}",
            get(idocs::show).patch(idocs::update).delete(idocs::delete),
        )
        .route("/v1/internal-docs/{id}/confirm", post(idocs::confirm))
        .route("/v1/internal-docs/{id}/print", get(idoc_print::print))
        .route("/v1/recipes", get(recipes::index).post(recipes::create))
        .route(
            "/v1/recipes/{id}",
            get(recipes::show).patch(recipes::update).delete(recipes::delete),
        )
        .route("/v1/production-orders", get(mfg::index).post(mfg::create))
        .route(
            "/v1/production-orders/{id}",
            get(mfg::show).delete(mfg::delete),
        )
        .route("/v1/production-orders/{id}/confirm", post(mfg::confirm))
        .route(
            "/v1/dividend-distributions",
            get(dividends::index).post(dividends::create),
        )
        .route(
            "/v1/dividend-distributions/{id}",
            get(dividends::show).patch(dividends::update).delete(dividends::delete),
        )
        .route("/v1/dividends/{id}/pay", post(dividends::pay))
        .route("/v1/reports", get(reports::index))
        .route("/v1/reports/export", get(reports::export))
        .route("/v1/fs/lines", get(fsform::lines))
        .route("/v1/fs/formulas", get(fsform::index).put(fsform::put))
        .route("/v1/fs/formulas/defaults", post(fsform::defaults))
        .route("/v1/fs/formulas/validate", post(fsform::validate))
        .route("/v1/fs/formulas/clear", post(fsform::clear))
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
