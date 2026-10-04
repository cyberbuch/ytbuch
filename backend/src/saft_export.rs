//! GET /v1/saft/export по BG_SAFT_Schema V1.0.1.
//! Заглавката носи Ownership, контакт и IBAN. Месец без банково плащане
//! пак пише Payments с нула записа: схемата иска поне един Payment.

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::companies;
use crate::form::auth_sub;
use crate::saft_gl::{accounts_xml, entries_xml};
use crate::saft_load::{self, Company, Owner, Parent, Snap};
use crate::saft_mode::{annual_xml, ondemand_xml, payments_xml, tax_table, uom_products};
use crate::saft_src::{self, parties_xml};
use crate::saft_xml::{self, clip, text, Addr};
use crate::{api_err, ApiError, AppState};

const NS: &str = "mf:nra:dgti:dxxxx:declaration:v1";

#[derive(Deserialize)]
pub struct Q {
    period: Option<String>,
    mode: Option<String>,
}

pub async fn export(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Q>,
) -> Result<Response, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let period = q.period.as_deref().unwrap_or("").trim();
    let Some((year, month)) = parse_period(period) else {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            "period е задължителен (YYYY-MM-01)",
        ));
    };
    let mode = mode_of(q.mode.as_deref().unwrap_or(""));
    let cid = companies::active_company_id(&st.db, &sub).await?;
    if cid <= 0 {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "няма активна фирма за експорт",
        ));
    }
    let snap = saft_load::load(&st.db, cid).await?;
    let xml = build(&snap, period, year, month, mode);
    let filename = format!("saft_{period}_{mode}.xml");
    let disp = format!("attachment; filename=\"{filename}\"");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .header(header::CONTENT_DISPOSITION, disp)
        .body(Body::from(xml))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

fn build(s: &Snap, period: &str, year: i32, month: i32, mode: &str) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!("<nsSAFT:AuditFile xmlns:nsSAFT=\"{NS}\">"));
    xml.push_str(&header(s, period, year, month, mode));
    match mode {
        "annual" => xml.push_str(&annual_xml(s, year)),
        "ondemand" => xml.push_str(&ondemand_xml(s)),
        _ => xml.push_str(&monthly(s, year, month)),
    }
    xml.push_str("</nsSAFT:AuditFile>");
    xml
}

fn monthly(s: &Snap, year: i32, month: i32) -> String {
    let (sales, buys) = saft_src::invoices_xml(s, year, month);
    let mut xml = saft_xml::open("MasterFilesMonthly");
    xml.push_str(&accounts_xml(s, year, month));
    xml.push_str(&parties_xml(s, year, month));
    xml.push_str(&tax_table());
    xml.push_str(&uom_products(s));
    xml.push_str(&saft_xml::close("MasterFilesMonthly"));
    xml.push_str(&entries_xml(s, year, month));
    xml.push_str(&saft_xml::open("SourceDocumentsMonthly"));
    xml.push_str(&sales);
    xml.push_str(&payments_xml(s, year, month));
    xml.push_str(&buys);
    xml.push_str(&saft_xml::close("SourceDocumentsMonthly"));
    xml
}

fn header(s: &Snap, period: &str, year: i32, month: i32, mode: &str) -> String {
    let c = &s.company;
    let (start, end, comment) = match mode {
        "annual" => (1, 12, "A"),
        "ondemand" => (month, month, "D"),
        _ => (month, month, "M"),
    };
    let mut xml = saft_xml::open("Header");
    xml.push_str(&text("AuditFileVersion", "007"));
    xml.push_str(&text("AuditFileCountry", "BG"));
    let cc = saft_xml::country(&c.country);
    if let Some(code) = saft_xml::region_code(&c.region, &cc) {
        let code = clip(&code, 9);
        if !code.is_empty() {
            xml.push_str(&text("AuditFileRegion", &code));
        }
    }
    xml.push_str(&text("AuditFileDateCreated", period));
    xml.push_str(&text("SoftwareCompanyName", &filled(&c.soft_name, "ytbuch", 70)));
    xml.push_str(&text("SoftwareID", &filled(&c.soft_id, "YTBUCH", 256)));
    xml.push_str(&text("SoftwareVersion", &filled(&c.soft_ver, "1.0", 18)));
    xml.push_str(&company_xml(c));
    xml.push_str(&ownership(s));
    xml.push_str(&text("DefaultCurrencyCode", &currency_code(&c.currency)));
    xml.push_str(&saft_xml::open("SelectionCriteria"));
    xml.push_str(&text("TaxReportingJurisdiction", "NRA"));
    xml.push_str(&text("PeriodStart", &start.to_string()));
    xml.push_str(&text("PeriodStartYear", &year.to_string()));
    xml.push_str(&text("PeriodEnd", &end.to_string()));
    xml.push_str(&text("PeriodEndYear", &year.to_string()));
    xml.push_str(&text("DocumentType", "F"));
    xml.push_str(&saft_xml::close("SelectionCriteria"));
    xml.push_str(&text("HeaderComment", comment));
    xml.push_str(&text("TaxAccountingBasis", &basis(&c.basis)));
    let entity = clip(&c.entity, 70);
    if !entity.is_empty() {
        xml.push_str(&text("TaxEntity", &entity));
    }
    xml.push_str(&saft_xml::close("Header"));
    xml
}

fn company_xml(c: &Company) -> String {
    let eik = eik_or0(&c.eik);
    let (first, last) = person(&c.mol);
    let mut xml = saft_xml::open("Company");
    xml.push_str(&text("RegistrationNumber", &eik));
    xml.push_str(&text("Name", &shown(&c.name)));
    xml.push_str(&saft_xml::address(
        "Address",
        &Addr {
            street: &c.street,
            number: &c.number,
            extra: &c.extra,
            building: &c.building,
            city: &c.city,
            post: &c.post,
            region: &c.region,
            country: &c.country,
            kind: &c.addr_type,
        },
    ));
    xml.push_str(&saft_xml::open("Contact"));
    xml.push_str(&saft_xml::open("ContactPerson"));
    xml.push_str(&text("FirstName", &first));
    xml.push_str(&text("LastName", &last));
    xml.push_str(&text("OtherTitles", "МОЛ"));
    xml.push_str(&saft_xml::close("ContactPerson"));
    let phone = clip(&c.phone, 18);
    if !phone.is_empty() {
        xml.push_str(&text("Telephone", &phone));
    }
    let email = clip(&c.email, 70);
    if !email.is_empty() {
        xml.push_str(&text("Email", &email));
    }
    let site = c.website.trim();
    if site.starts_with("http://") || site.starts_with("https://") {
        xml.push_str(&text("Website", &clip(site, 256)));
    }
    xml.push_str(&saft_xml::close("Contact"));
    xml.push_str(&saft_xml::open("TaxRegistration"));
    xml.push_str(&text("TaxRegistrationNumber", &eik));
    xml.push_str(&text("TaxType", "100010"));
    let vat = clip(&c.vat, 35);
    if !vat.is_empty() {
        xml.push_str(&text("TaxNumber", &vat));
    }
    xml.push_str(&saft_xml::close("TaxRegistration"));
    let iban = {
        let i = clip(&c.iban, 35);
        if i.is_empty() { "0".into() } else { i }
    };
    xml.push_str(&saft_xml::open("BankAccount"));
    xml.push_str(&text("IBANNumber", &iban));
    xml.push_str(&saft_xml::close("BankAccount"));
    xml.push_str(&saft_xml::close("Company"));
    xml
}

fn ownership(s: &Snap) -> String {
    let mut xml = saft_xml::open("Ownership");
    xml.push_str(&text("IsPartOfGroup", &group_code(&s.company.group)));
    let mut names = Vec::new();
    let mut egns = Vec::new();
    let mut latin = Vec::new();
    let mut foreign = Vec::new();
    for o in owners_of(s) {
        push_owner(o, &mut names, &mut egns, &mut latin, &mut foreign);
    }
    for n in &names {
        xml.push_str(&text("BeneficialOwnerNameCyrillicBG", n));
    }
    for e in &egns {
        xml.push_str(&text("BeneficialOwnerEGN", e));
    }
    for n in &latin {
        xml.push_str(&text("BeneficialOwnerNameLatinForeign", n));
    }
    for c in &foreign {
        xml.push_str(&text("BeneficialCountryForeign", c));
    }
    for c in &foreign {
        xml.push_str(&text("BeneficialCountryForeignCode", c));
    }
    let (bg_names, bg_uic, f_cyr, f_lat, f_cc) = parents_of(s);
    for n in &bg_names {
        xml.push_str(&text("UltimateOwnerNameCyrillicBG", n));
    }
    for u in &bg_uic {
        xml.push_str(&text("UltimateOwnerUICBG", u));
    }
    for n in &f_cyr {
        xml.push_str(&text("UltimateOwnerNameCyrillicForeign", n));
    }
    for n in &f_lat {
        xml.push_str(&text("UltimateOwnerNameLatinForeign", n));
    }
    for c in &f_cc {
        xml.push_str(&text("CountryForeign", c));
    }
    xml.push_str(&saft_xml::close("Ownership"));
    xml
}

fn owners_of(s: &Snap) -> Vec<&Owner> {
    s.owners
        .iter()
        .filter(|o| saft_xml::same_co(o.company_id, s.company_id))
        .collect()
}

fn push_owner(o: &Owner, names: &mut Vec<String>, egns: &mut Vec<String>, latin: &mut Vec<String>, foreign: &mut Vec<String>) {
    let cc = saft_xml::country(&o.country);
    if o.country.trim().is_empty() || cc == "BG" {
        let name = clip(&join_name(&o.first_bg, &o.last_bg), 70);
        if !name.is_empty() {
            names.push(name);
        }
        let egn = clip(&o.egn, 35);
        if !egn.is_empty() {
            egns.push(egn);
        }
        return;
    }
    let mut name = join_name(&o.first_lat, &o.last_lat);
    if name.is_empty() {
        name = join_name(&o.first_bg, &o.last_bg);
    }
    let name = clip(&name, 70);
    if !name.is_empty() {
        latin.push(name);
        foreign.push(cc);
    }
}

fn parents_of(s: &Snap) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut bg_names = Vec::new();
    let mut bg_uic = Vec::new();
    let mut f_cyr = Vec::new();
    let mut f_lat = Vec::new();
    let mut f_cc = Vec::new();
    let mut bg_lat = String::new();
    for p in s.parents.iter().filter(|p| saft_xml::same_co(p.company_id, s.company_id)) {
        push_parent(p, &mut bg_names, &mut bg_uic, &mut f_cyr, &mut f_lat, &mut f_cc, &mut bg_lat);
    }
    if bg_names.is_empty() && f_cyr.is_empty() {
        let name = shown(&s.company.name);
        bg_names.push(name.clone());
        bg_uic.push(eik_or0(&s.company.eik));
        f_cyr.push(name.clone());
        f_lat.push(name);
        f_cc.push("BG".into());
    } else if f_cyr.is_empty() {
        let name = bg_names[0].clone();
        let latin = if bg_lat.is_empty() { name.clone() } else { bg_lat };
        f_cyr.push(name);
        f_lat.push(latin);
        f_cc.push("BG".into());
    } else if bg_names.is_empty() {
        bg_names.push(f_cyr[0].clone());
        bg_uic.push("0".into());
    }
    (bg_names, bg_uic, f_cyr, f_lat, f_cc)
}

fn push_parent(
    p: &Parent,
    bg_names: &mut Vec<String>,
    bg_uic: &mut Vec<String>,
    f_cyr: &mut Vec<String>,
    f_lat: &mut Vec<String>,
    f_cc: &mut Vec<String>,
    bg_lat: &mut String,
) {
    let cc = saft_xml::country(&p.country);
    let name = clip(&p.name_bg, 70);
    let latin = {
        let l = clip(&p.name_lat, 70);
        if l.is_empty() { name.clone() } else { l }
    };
    let uic = {
        let u = clip(&p.uic, 35);
        if u.is_empty() { "0".into() } else { u }
    };
    if p.country.trim().is_empty() || cc == "BG" {
        if name.is_empty() {
            return;
        }
        if bg_lat.is_empty() {
            *bg_lat = latin;
        }
        bg_names.push(name);
        bg_uic.push(uic);
        return;
    }
    f_cyr.push(if name.is_empty() { latin.clone() } else { name });
    f_lat.push(latin);
    f_cc.push(cc);
}

fn parse_period(raw: &str) -> Option<(i32, i32)> {
    let b = raw.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || &b[8..] != b"01" {
        return None;
    }
    let year: i32 = std::str::from_utf8(&b[..4]).ok()?.parse().ok()?;
    let month: i32 = std::str::from_utf8(&b[5..7]).ok()?.parse().ok()?;
    if (1..=12).contains(&month) {
        Some((year, month))
    } else {
        None
    }
}

fn mode_of(raw: &str) -> &'static str {
    match raw.trim() {
        "annual" => "annual",
        "ondemand" => "ondemand",
        _ => "monthly",
    }
}

fn basis(raw: &str) -> String {
    match raw.trim() {
        "A" | "P" | "BANK" | "INSURANCE" => raw.trim().to_string(),
        _ => "A".into(),
    }
}

fn group_code(raw: &str) -> String {
    match raw.trim() {
        "1" | "2" | "3" | "4" | "5" => raw.trim().to_string(),
        _ => "1".into(),
    }
}

fn currency_code(raw: &str) -> String {
    let u = raw.trim().to_ascii_uppercase();
    if u.len() == 3 && u.bytes().all(|c| c.is_ascii_uppercase()) {
        u
    } else {
        "EUR".into()
    }
}

fn filled(value: &str, fallback: &str, n: usize) -> String {
    let t = clip(value, n);
    if t.is_empty() { clip(fallback, n) } else { t }
}

fn person(mol: &str) -> (String, String) {
    let t = mol.trim();
    if t.is_empty() {
        return ("—".into(), "—".into());
    }
    let mut parts = t.split_whitespace();
    let first = clip(parts.next().unwrap_or("—"), 35);
    let rest = parts.collect::<Vec<_>>().join(" ");
    let last = if rest.is_empty() { first.clone() } else { clip(&rest, 70) };
    let first = if first.is_empty() { "—".into() } else { first };
    (first, if last.is_empty() { "—".into() } else { last })
}

fn join_name(first: &str, last: &str) -> String {
    format!("{} {}", first.trim(), last.trim()).trim().to_string()
}

fn shown(name: &str) -> String {
    let t = clip(name, 70);
    if t.is_empty() { "—".into() } else { t }
}

fn eik_or0(eik: &str) -> String {
    let t = clip(eik, 35);
    if t.is_empty() { "0".into() } else { t }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_is_first_of_month() {
        assert_eq!(parse_period("2026-10-01"), Some((2026, 10)));
        assert_eq!(parse_period("2026-10-02"), None);
        assert_eq!(parse_period(""), None);
        assert_eq!(mode_of("nope"), "monthly");
        assert_eq!(mode_of("annual"), "annual");
        assert_eq!(group_code(""), "1");
        assert_eq!(group_code("3"), "3");
        assert_eq!(person("Иван Петров"), ("Иван".into(), "Петров".into()));
    }
}
