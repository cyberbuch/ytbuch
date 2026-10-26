//! Проверка на права по маршрут. Едно middleware пред всички /v1 пътища:
//! пътят се сваля до permission ключ, ролята на потребителя трябва да го
//! съдържа. Супер администраторът и `"*"` в ролята минават навсякъде.
//! Пътища без permission остават достъпни за всеки влязъл потребител:
//! собствен профил, табло, превключвател на фирма, статични каталози.

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

/// Кой permission ключ пази даден път. `None` значи само вход.
/// Правата са грубо по екран: без деление четене/запис — ролята вижда
/// целия модул или нищо.
pub fn required_permission(method: &Method, path: &str) -> Option<&'static str> {
    let s = segments(path);
    if s.first() != Some(&"v1") {
        return None;
    }
    match s.get(1).copied() {
        Some("companies") => {
            // Списъкът с фирми е на превключвателя — четенето е за всеки,
            // създаването и редакцията изискват право. Потребителите на
            // фирма са под /users и се пазят от ролята "users".
            if s.len() == 2 && method == Method::GET {
                None
            } else if s.len() >= 4 && s.get(3) == Some(&"users") {
                Some("users")
            } else {
                Some("companies")
            }
        }
        Some("company-locations")
        | Some("beneficial-owners")
        | Some("ultimate-parents")
        | Some("document-series") => Some("companies"),
        Some("accounts") => Some("accounts"),
        Some("counterparts") | Some("addresses") => Some("counterparts"),
        Some("products") | Some("product-name-mappings") => Some("products"),
        Some("invoices") => Some("invoices"),
        Some("scans") => Some("scans"),
        Some("journal") => Some("journal"),
        Some("accounting-periods") | Some("opening-balances") => Some("periods"),
        Some("vat") => Some("vat"),
        Some("bank-accounts") | Some("bank-transactions") => Some("banking"),
        Some("fixed-assets") | Some("fixed-asset-categories") => Some("assets"),
        Some("inventory") | Some("internal-docs") | Some("material-issues") => Some("stock"),
        Some("stock") => match s.get(2).copied() {
            Some("rules") => Some("companies"),
            _ => Some("stock"),
        },
        Some("recipes") | Some("production-orders") => Some("production"),
        Some("dividend-distributions") | Some("dividends") => Some("dividends"),
        Some("reports") | Some("fs") => Some("reports"),
        Some("audit-logs") => match s.get(2).copied() {
            Some("invoice") => Some("invoices"),
            Some("journal") => Some("journal"),
            Some("internal_doc") | Some("material_issue") => Some("stock"),
            Some("production_order") => Some("production"),
            Some("bank_tx") => Some("banking"),
            _ => Some("reports"),
        },
        Some("saft") => match s.get(2).copied() {
            Some("export") | Some("movement-mappings") => Some("saft"),
            // Номенклатурите са статичен каталог, четат ги много екрана.
            _ => None,
        },
        Some("users") | Some("roles") => Some("users"),
        Some("system-settings") => Some("settings"),
        Some("exchange-rates") => Some("rates"),
        _ => None,
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
    if let Some(key) = required_permission(&method, &path) {
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
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::{is_public, required_permission, stale_session};
    use axum::http::Method;

    fn perm(method: &str, path: &str) -> Option<&'static str> {
        required_permission(&Method::from_bytes(method.as_bytes()).unwrap(), path)
    }

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
        assert_eq!(perm("GET", "/v1/me"), None);
        assert_eq!(perm("POST", "/v1/me/password"), None);
        assert_eq!(perm("GET", "/v1/dashboard"), None);
        assert_eq!(perm("GET", "/v1/companies"), None);
        assert_eq!(perm("GET", "/v1/units"), None);
        assert_eq!(perm("GET", "/v1/saft/nomenclatures"), None);
        assert_eq!(perm("GET", "/v1/vat-exemptions"), None);
        assert_eq!(perm("GET", "/health"), None);
    }

    #[test]
    fn company_writes_need_permission() {
        assert_eq!(perm("POST", "/v1/companies"), Some("companies"));
        assert_eq!(perm("PATCH", "/v1/companies/3"), Some("companies"));
        assert_eq!(perm("DELETE", "/v1/companies/3"), Some("companies"));
        assert_eq!(perm("POST", "/v1/companies/3/locations"), Some("companies"));
        assert_eq!(perm("DELETE", "/v1/document-series/7"), Some("companies"));
        assert_eq!(perm("GET", "/v1/companies/3/users"), Some("users"));
        assert_eq!(perm("POST", "/v1/companies/3/users"), Some("users"));
        assert_eq!(perm("PATCH", "/v1/companies/3/users/5"), Some("users"));
        assert_eq!(perm("DELETE", "/v1/companies/3/users/5"), Some("users"));
    }

    #[test]
    fn modules_map_to_keys() {
        assert_eq!(perm("GET", "/v1/accounts"), Some("accounts"));
        assert_eq!(perm("GET", "/v1/counterparts"), Some("counterparts"));
        assert_eq!(perm("POST", "/v1/counterparts/vies"), Some("counterparts"));
        assert_eq!(perm("GET", "/v1/products"), Some("products"));
        assert_eq!(perm("GET", "/v1/product-name-mappings"), Some("products"));
        assert_eq!(perm("POST", "/v1/invoices/9/post"), Some("invoices"));
        assert_eq!(perm("GET", "/v1/invoices/9/print"), Some("invoices"));
        assert_eq!(perm("POST", "/v1/scans/extract"), Some("scans"));
        assert_eq!(perm("GET", "/v1/journal"), Some("journal"));
        assert_eq!(perm("GET", "/v1/journal/4"), Some("journal"));
        assert_eq!(perm("POST", "/v1/accounting-periods/close"), Some("periods"));
        assert_eq!(perm("POST", "/v1/opening-balances"), Some("periods"));
        assert_eq!(perm("GET", "/v1/vat/registers"), Some("vat"));
        assert_eq!(perm("GET", "/v1/vat/export"), Some("vat"));
        assert_eq!(perm("POST", "/v1/bank-transactions/4/book"), Some("banking"));
        assert_eq!(perm("GET", "/v1/bank-accounts"), Some("banking"));
        assert_eq!(perm("POST", "/v1/fixed-assets/depreciation/post"), Some("assets"));
        assert_eq!(perm("DELETE", "/v1/fixed-asset-categories/2"), Some("assets"));
        assert_eq!(perm("GET", "/v1/inventory/at-location"), Some("stock"));
        assert_eq!(perm("POST", "/v1/internal-docs/5/confirm"), Some("stock"));
        assert_eq!(perm("GET", "/v1/stock/rules"), Some("companies"));
        assert_eq!(perm("POST", "/v1/stock/rebuild"), Some("stock"));
        assert_eq!(perm("GET", "/v1/stock/report"), Some("stock"));
        assert_eq!(perm("POST", "/v1/material-issues/1/confirm"), Some("stock"));
        assert_eq!(perm("POST", "/v1/production-orders/2/confirm"), Some("production"));
        assert_eq!(perm("PUT", "/v1/recipes/1"), Some("production"));
        assert_eq!(perm("POST", "/v1/dividends/3/pay"), Some("dividends"));
        assert_eq!(perm("GET", "/v1/reports"), Some("reports"));
        assert_eq!(perm("GET", "/v1/audit-logs"), Some("reports"));
        assert_eq!(perm("GET", "/v1/audit-logs/invoice/1"), Some("invoices"));
        assert_eq!(perm("GET", "/v1/audit-logs/journal/1"), Some("journal"));
        assert_eq!(perm("PUT", "/v1/fs/formulas"), Some("reports"));
        assert_eq!(perm("GET", "/v1/saft/export"), Some("saft"));
        assert_eq!(perm("POST", "/v1/saft/movement-mappings/defaults"), Some("saft"));
        assert_eq!(perm("GET", "/v1/users"), Some("users"));
        assert_eq!(perm("PATCH", "/v1/roles/2"), Some("users"));
        assert_eq!(perm("PUT", "/v1/system-settings"), Some("settings"));
        assert_eq!(perm("POST", "/v1/system-settings/s3-backup"), Some("settings"));
        assert_eq!(perm("POST", "/v1/exchange-rates/import"), Some("rates"));
        assert_eq!(perm("POST", "/v1/addresses/parse"), Some("counterparts"));
    }

    #[test]
    fn unknown_paths_stay_open() {
        assert_eq!(perm("GET", "/v1/unknown"), None);
        assert_eq!(perm("GET", "/v1/invoices"), Some("invoices"));
    }

    #[test]
    fn old_session_version_is_stale() {
        assert!(stale_session(0, 1), "старият токен отпада след смяна на парола");
        assert!(!stale_session(1, 1), "текущият токен важи");
        assert!(!stale_session(0, 0), "токен без версия важи до първата смяна");
    }
}
