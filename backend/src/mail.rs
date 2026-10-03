//! SMTP през lettre. Няма отделен Python процес.

use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::Utc;
use lettre::message::{header::ContentType, Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, SmtpTransport, Transport};
use serde_json::{json, Value};

use crate::form::auth_sub;
use crate::settings::{self, SmtpCfg};
use crate::{api_err, AppState, ApiError};

fn smtp_text(err: impl std::fmt::Display) -> String {
    let text = err.to_string();
    let low = text.to_lowercase();
    if low.contains("authenticat") || low.contains("535") {
        return "Грешка при автентикация. Проверете потребител/парола.".into();
    }
    if low.contains("connect") || low.contains("timed out") || low.contains("dns") {
        return format!("Не може да се свърже: {text}");
    }
    format!("SMTP грешка: {text}")
}

pub fn send_html(cfg: &SmtpCfg, to: &str, subject: &str, html: &str) -> Result<(), String> {
    if cfg.host.is_empty() {
        return Err("SMTP host е задължителен".into());
    }
    let to = to.trim();
    if to.is_empty() {
        return Err("Получател е задължителен".into());
    }
    let from_email = if cfg.from_email.is_empty() {
        cfg.username.trim()
    } else {
        cfg.from_email.trim()
    };
    if from_email.is_empty() {
        return Err("smtp.from_email е задължителен".into());
    }
    let from_addr: Address = from_email
        .parse()
        .map_err(|_| "smtp.from_email е невалиден".to_string())?;
    let to_addr: Address = to
        .parse()
        .map_err(|_| "Получателят е невалиден".to_string())?;
    let email = Message::builder()
        .from(Mailbox::new(Some(cfg.from_name.clone()), from_addr))
        .to(Mailbox::new(None, to_addr))
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(html.to_string())
        .map_err(|e| format!("Грешка: {e}"))?;
    let mut builder = if cfg.port == 465 {
        SmtpTransport::relay(&cfg.host)
            .map_err(|e| smtp_text(e))?
            .port(465)
    } else if cfg.use_tls {
        SmtpTransport::starttls_relay(&cfg.host)
            .map_err(|e| smtp_text(e))?
            .port(cfg.port)
    } else {
        SmtpTransport::builder_dangerous(&cfg.host).port(cfg.port)
    };
    builder = builder.timeout(Some(Duration::from_secs(30)));
    if !cfg.username.is_empty() {
        builder = builder.credentials(Credentials::new(cfg.username.clone(), cfg.password.clone()));
    }
    builder.build().send(&email).map_err(smtp_text)?;
    Ok(())
}

pub async fn smtp_test(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let to = body
        .get("to")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if to.is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "Получател е задължителен"));
    }
    let map = settings::load(&st.db).await?;
    let cfg = settings::smtp_cfg(&map);
    let app = settings::app_name(&map);
    let subject = format!("Тестов имейл — {app}");
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let html = format!(
        "<!DOCTYPE html><html><body><h2>{app}</h2><p>SMTP работи. Изпратено: {now}</p></body></html>"
    );
    let sent = tokio::task::spawn_blocking(move || send_html(&cfg, &to, &subject, &html))
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sent.map_err(|msg| api_err(StatusCode::BAD_REQUEST, msg))?;
    Ok(Json(json!({ "message": "Имейлът е изпратен" })))
}
