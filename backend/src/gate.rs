//! Проверка на права по маршрут. Едно middleware пред всички /v1 пътища:
//! пътят се сваля до permission ключ, ролята на потребителя трябва да го
//! съдържа. Супер администраторът и `"*"` в ролята минават навсякъде.
//! Някои пътища са за всеки влязъл потребител: собствен профил, табло,
//! превключвател на фирма, статични каталози. Непознатите пътища се
//! отказват (404) — deny by default, за да не остава нов забравен маршрут
//! отворен само защото не е в картата на правата.

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use serde_json::Value;

use crate::auth;
use crate::identity;
use crate::{api_err, AppState, ApiError};

fn segments(path: &str) -> Vec<&str> {
    path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect()
}

/// Публични пътища: вход, 2FA, регистрация и забравена парола.
/// Всичко друго под /v1 иска валиден токен — иначе нов ендпойнт, който
/// не е в картата на правата, би останал отворен за всеки.
pub fn is_public(path: &str) -> bool {
    matches!(
        path,
        "/v1/auth/token"
            | "/v1/auth/mfa"
            | "/v1/auth/register"
            | "/v1/auth/forgot-password"
            | "/v1/auth/reset-password"
            | "/v1/auth/registration-status"
    )
}

/// Достъпът до даден /v1 път.
#[derive(Debug, PartialEq, Eq)]
pub enum Access {
    /// Всеки влязъл потребител (профил, табло, статични каталози).
    Any,
    /// Изисква permission ключ от ролята.
    Key(&'static str),
    /// Непознат път — отказва се, вместо да минава без проверка.
    Unknown,
}

/// Кой permission ключ пази даден път.
/// Правата са грубо по екран: без деление четене/запис — ролята вижда
/// целия модул или нищо.
pub fn required_permission(method: &Method, path: &str) -> Access {
    let s = segments(path);
    if s.first() != Some(&"v1") {
        return Access::Any;
    }
    match s.get(1).copied() {
        Some("companies") => {
            // Списъкът с фирми е на превключвателя — четенето е за всеки,
            // създаването и редакцията изискват право. Потребителите на
            // фирма са под /users и се пазят от ролята "users".
            if s.len() == 2 && method == Method::GET {
                Access::Any
            } else if s.len() >= 4 && s.get(3) == Some(&"users") {
                Access::Key("users")
            } else {
                Access::Key("companies")
            }
        }
        Some("company-locations")
        | Some("beneficial-owners")
        | Some("ultimate-parents")
        | Some("document-series") => Access::Key("companies"),
        Some("accounts") => Access::Key("accounts"),
        Some("counterparts") | Some("addresses") => Access::Key("counterparts"),
        Some("products") | Some("product-name-mappings") => Access::Key("products"),
        Some("invoices") => Access::Key("invoices"),
        Some("scans") => Access::Key("scans"),
        Some("journal") => Access::Key("journal"),
        Some("accounting-periods") | Some("opening-balances") => Access::Key("periods"),
        Some("vat") => Access::Key("vat"),
        Some("bank-accounts") | Some("bank-transactions") | Some("bank-matching-rules") => Access::Key("banking"),
        Some("fixed-assets") | Some("fixed-asset-categories") => Access::Key("assets"),
        Some("inventory") | Some("internal-docs") | Some("material-issues") => Access::Key("stock"),
        Some("stock") => match s.get(2).copied() {
            Some("rules") => Access::Key("companies"),
            _ => Access::Key("stock"),
        },
        Some("recipes") | Some("production-orders") => Access::Key("production"),
        Some("dividend-distributions") | Some("dividends") => Access::Key("dividends"),
        Some("reports") | Some("fs") => Access::Key("reports"),
        Some("audit-logs") => match s.get(2).copied() {
            Some("invoice") => Access::Key("invoices"),
            Some("journal") => Access::Key("journal"),
            Some("internal_doc") | Some("material_issue") => Access::Key("stock"),
            Some("production_order") => Access::Key("production"),
            Some("bank_tx") => Access::Key("banking"),
            _ => Access::Key("reports"),
        },
        Some("saft") => match s.get(2).copied() {
            Some("export") | Some("movement-mappings") => Access::Key("saft"),
            // Номенклатурите са статичен каталог, четат ги много екрана.
            _ => Access::Any,
        },
        Some("users") | Some("roles") => Access::Key("users"),
        Some("system-settings") => Access::Key("settings"),
        Some("exchange-rates") => Access::Key("rates"),
        // За всеки влязъл: профил, табло, превключвател на фирма, каталози.
        Some("me") | Some("dashboard") | Some("active-company") | Some("units")
        | Some("vat-exemptions") => Access::Any,
        _ => Access::Unknown,
    }
}

fn allowed(perms: &[Value], key: &str) -> bool {
    perms
        .iter()
        .any(|p| p.as_str() == Some("*") || p.as_str() == Some(key))
}

/// Токенът е от приключила сесия, когато версията му не е текущата.
/// Смяната на паролата расти версията и така отпада старият токен.
fn stale_session(claims_ver: i64, user_ver: i64) -> bool {
    claims_ver != user_ver
}

pub async fn gate(
    State(st): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    // Всичко извън /v1 (health, ready) и публичните входни пътища минават.
    if !path.starts_with("/v1/") || is_public(&path) {
        return Ok(next.run(req).await);
    }
    let claims = auth::claims(&st.jwt_secret, req.headers())?;
    if claims.mfa == 1 {
        return Err(api_err(StatusCode::UNAUTHORIZED, "невалиден токен"));
    }
    let Some(user) = identity::by_login(&st.db, &claims.sub).await? else {
        return Err(api_err(StatusCode::UNAUTHORIZED, "потребителят не съществува"));
    };
    if user.is_active == 0 {
        return Err(api_err(StatusCode::FORBIDDEN, "потребителят е неактивен"));
    }
    // Смяната на паролата приключва сесиите, издадени преди нея.
    if stale_session(claims.ver, user.ver) {
        return Err(api_err(
            StatusCode::UNAUTHORIZED,
            "сесията е приключила — влезте отново",
        ));
    }
    match required_permission(&method, &path) {
        Access::Unknown => {
            // Непознат /v1 път — отказ, вместо да мине само с вход.
            return Err(api_err(StatusCode::NOT_FOUND, "непознат път"));
        }
        Access::Any => {}
        Access::Key(key) => {
            if user.is_super_admin != 1 {
                // Правата идват от ролята на потребителя към активната фирма.
                let company = crate::companies::resolve_active(&st.db, &claims.sub).await?;
                let perms = crate::members::perms_for(&st.db, &user, company).await?;
                if !allowed(&perms, key) {
                    return Err(api_err(
                        StatusCode::FORBIDDEN,
                        format!("липсва право: {key}"),
                    ));
                }
            }
        }
    }
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::{is_public, required_permission, stale_session, Access};
    use axum::http::Method;

    fn perm(method: &str, path: &str) -> Access {
        required_permission(&Method::from_bytes(method.as_bytes()).unwrap(), path)
    }

    const ANY: Access = Access::Any;

    #[test]
    fn only_auth_entry_points_are_public() {
        assert!(is_public("/v1/auth/token"));
        assert!(is_public("/v1/auth/mfa"));
        assert!(is_public("/v1/auth/register"));
        assert!(is_public("/v1/auth/forgot-password"));
        assert!(is_public("/v1/auth/reset-password"));
        assert!(is_public("/v1/auth/registration-status"));
        assert!(!is_public("/v1/me"));
        assert!(!is_public("/v1/dashboard"));
        assert!(!is_public("/v1/system-settings"));
        assert!(!is_public("/v1/unknown"));
        assert!(!is_public("/v1/auth/other"));
    }

    #[test]
    fn public_and_own_paths_stay_open() {
        assert_eq!(perm("GET", "/v1/me"), ANY);
        assert_eq!(perm("POST", "/v1/me/password"), ANY);
        assert_eq!(perm("GET", "/v1/dashboard"), ANY);
        assert_eq!(perm("GET", "/v1/active-company"), ANY);
        assert_eq!(perm("GET", "/v1/companies"), ANY);
        assert_eq!(perm("GET", "/v1/units"), ANY);
        assert_eq!(perm("GET", "/v1/saft/nomenclatures"), ANY);
        assert_eq!(perm("GET", "/v1/vat-exemptions"), ANY);
        assert_eq!(perm("GET", "/health"), ANY);
    }

    #[test]
    fn company_writes_need_permission() {
        assert_eq!(perm("POST", "/v1/companies"), Access::Key("companies"));
        assert_eq!(perm("PATCH", "/v1/companies/3"), Access::Key("companies"));
        assert_eq!(perm("DELETE", "/v1/companies/3"), Access::Key("companies"));
        assert_eq!(perm("POST", "/v1/companies/3/locations"), Access::Key("companies"));
        assert_eq!(perm("DELETE", "/v1/document-series/7"), Access::Key("companies"));
        assert_eq!(perm("GET", "/v1/companies/3/users"), Access::Key("users"));
        assert_eq!(perm("POST", "/v1/companies/3/users"), Access::Key("users"));
        assert_eq!(perm("PATCH", "/v1/companies/3/users/5"), Access::Key("users"));
        assert_eq!(perm("DELETE", "/v1/companies/3/users/5"), Access::Key("users"));
    }

    #[test]
    fn modules_map_to_keys() {
        assert_eq!(perm("GET", "/v1/accounts"), Access::Key("accounts"));
        assert_eq!(perm("GET", "/v1/counterparts"), Access::Key("counterparts"));
        assert_eq!(perm("POST", "/v1/counterparts/vies"), Access::Key("counterparts"));
        assert_eq!(perm("GET", "/v1/products"), Access::Key("products"));
        assert_eq!(perm("GET", "/v1/product-name-mappings"), Access::Key("products"));
        assert_eq!(perm("POST", "/v1/invoices/9/post"), Access::Key("invoices"));
        assert_eq!(perm("GET", "/v1/invoices/9/print"), Access::Key("invoices"));
        assert_eq!(perm("POST", "/v1/scans/extract"), Access::Key("scans"));
        assert_eq!(perm("GET", "/v1/journal"), Access::Key("journal"));
        assert_eq!(perm("GET", "/v1/journal/4"), Access::Key("journal"));
        assert_eq!(perm("POST", "/v1/accounting-periods/close"), Access::Key("periods"));
        assert_eq!(perm("POST", "/v1/opening-balances"), Access::Key("periods"));
        assert_eq!(perm("GET", "/v1/vat/registers"), Access::Key("vat"));
        assert_eq!(perm("GET", "/v1/vat/export"), Access::Key("vat"));
        assert_eq!(perm("POST", "/v1/bank-transactions/4/book"), Access::Key("banking"));
        assert_eq!(perm("GET", "/v1/bank-accounts"), Access::Key("banking"));
        assert_eq!(perm("POST", "/v1/fixed-assets/depreciation/post"), Access::Key("assets"));
        assert_eq!(perm("DELETE", "/v1/fixed-asset-categories/2"), Access::Key("assets"));
        assert_eq!(perm("GET", "/v1/inventory/at-location"), Access::Key("stock"));
        assert_eq!(perm("POST", "/v1/internal-docs/5/confirm"), Access::Key("stock"));
        assert_eq!(perm("GET", "/v1/stock/rules"), Access::Key("companies"));
        assert_eq!(perm("POST", "/v1/stock/rebuild"), Access::Key("stock"));
        assert_eq!(perm("GET", "/v1/stock/report"), Access::Key("stock"));
        assert_eq!(perm("POST", "/v1/material-issues/1/confirm"), Access::Key("stock"));
        assert_eq!(perm("POST", "/v1/production-orders/2/confirm"), Access::Key("production"));
        assert_eq!(perm("PUT", "/v1/recipes/1"), Access::Key("production"));
        assert_eq!(perm("POST", "/v1/dividends/3/pay"), Access::Key("dividends"));
        assert_eq!(perm("GET", "/v1/reports"), Access::Key("reports"));
        assert_eq!(perm("GET", "/v1/audit-logs"), Access::Key("reports"));
        assert_eq!(perm("GET", "/v1/audit-logs/invoice/1"), Access::Key("invoices"));
        assert_eq!(perm("GET", "/v1/audit-logs/journal/1"), Access::Key("journal"));
        assert_eq!(perm("PUT", "/v1/fs/formulas"), Access::Key("reports"));
        assert_eq!(perm("GET", "/v1/saft/export"), Access::Key("saft"));
        assert_eq!(perm("POST", "/v1/saft/movement-mappings/defaults"), Access::Key("saft"));
        assert_eq!(perm("GET", "/v1/users"), Access::Key("users"));
        assert_eq!(perm("PATCH", "/v1/roles/2"), Access::Key("users"));
        assert_eq!(perm("PUT", "/v1/system-settings"), Access::Key("settings"));
        assert_eq!(perm("POST", "/v1/system-settings/s3-backup"), Access::Key("settings"));
        assert_eq!(perm("POST", "/v1/exchange-rates/import"), Access::Key("rates"));
        assert_eq!(perm("POST", "/v1/addresses/parse"), Access::Key("counterparts"));
    }

    #[test]
    fn unknown_paths_are_denied() {
        // Deny by default: нов забравен рут не бива да минава само с вход.
        assert_eq!(perm("GET", "/v1/unknown"), Access::Unknown);
        assert_eq!(perm("POST", "/v1/auth/other"), Access::Unknown);
        assert_eq!(perm("GET", "/v1/evil"), Access::Unknown);
        // известните остават по картата
        assert_eq!(perm("GET", "/v1/invoices"), Access::Key("invoices"));
    }

    #[test]
    fn old_session_version_is_stale() {
        assert!(stale_session(0, 1), "старият токен отпада след смяна на парола");
        assert!(!stale_session(1, 1), "текущият токен важи");
        assert!(!stale_session(0, 0), "токен без версия важи до първата смяна");
    }
}
