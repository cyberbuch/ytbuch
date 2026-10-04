//! SAF-T XML помощници по BG_SAFT_Schema V1.0.1.
//! Дължините са в знаци: улица 70, град и област 35, номер и пощенски код 18.

use crate::dec::{self, money};
use crate::print_sheet::xml_escape;

pub fn open(name: &str) -> String {
    format!("<nsSAFT:{name}>")
}

pub fn close(name: &str) -> String {
    format!("</nsSAFT:{name}>")
}

pub fn text(name: &str, value: &str) -> String {
    format!("<nsSAFT:{name}>{}</nsSAFT:{name}>", xml_escape(value))
}

pub fn opt(name: &str, value: &str) -> String {
    let t = value.trim();
    if t.is_empty() {
        String::new()
    } else {
        text(name, t)
    }
}

pub fn clip(s: &str, n: usize) -> String {
    let t = s.trim();
    if n == 0 || t.is_empty() {
        return String::new();
    }
    t.chars().take(n).collect()
}

pub fn amount(name: &str, value: i128, currency: &str) -> String {
    let t = money(value);
    format!(
        "{}{}{}{}{}",
        open(name),
        text("Amount", &t),
        text("CurrencyCode", currency),
        text("CurrencyAmount", &t),
        close(name)
    )
}

pub fn money_of(raw: &str) -> i128 {
    dec::or0(raw)
}

/// Дебит, когато дебитът е по-голям или равен на кредита. Равни страни, и нулата, са дебит 0.00.
pub fn bal(debit_tag: &str, credit_tag: &str, debit: i128, credit: i128) -> String {
    if debit >= credit {
        text(debit_tag, &money(debit - credit))
    } else {
        text(credit_tag, &money(credit - debit))
    }
}

pub fn nra_ok(s: &str) -> bool {
    let b = s.trim().as_bytes();
    if !(3..=5).contains(&b.len()) || !(b'1'..=b'9').contains(&b[0]) {
        return false;
    }
    b[1..].iter().all(|c| c.is_ascii_digit())
}

/// (код на НАП, номер във фирмата). Празен резултат пропуска сметката.
pub fn pack(saft_type: &str, number: &str) -> Option<(String, String)> {
    let firm = number.trim();
    let nra = if nra_ok(saft_type) {
        saft_type.trim().to_string()
    } else if nra_ok(firm) {
        firm.to_string()
    } else {
        return None;
    };
    let tax = if firm.is_empty() { nra.clone() } else { clip(firm, 35) };
    Some((nra, tax))
}

pub fn pref(nra: &str, p: &str) -> bool {
    nra == p || (nra.starts_with(p) && nra.len() > p.len())
}

pub fn same_co(row: i32, cid: i32) -> bool {
    cid <= 0 || row == 0 || row == cid
}

pub fn country(raw: &str) -> String {
    let u = raw.trim().to_ascii_uppercase();
    if u.len() == 2 && u.bytes().all(|c| c.is_ascii_uppercase()) {
        u
    } else {
        "BG".into()
    }
}

pub fn cyr(s: &str) -> bool {
    s.chars().any(|c| (c as u32) > 127)
}

fn region_key(raw: &str) -> String {
    let up: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect();
    if up.len() == 4 && up.starts_with("BG") && up[2..].bytes().all(|c| c.is_ascii_digit()) {
        format!("BG-{}", &up[2..])
    } else {
        up
    }
}

/// Код BG-01…BG-28. Празна или непозната област за BG се пропуска.
pub fn region_code(raw: &str, country: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let key = region_key(t);
    if let Some(rows) = crate::saft::catalog().get("regions") {
        for line in rows {
            if let Some((code, name)) = line.split_once('|') {
                if key == code || t == name {
                    return Some(code.to_string());
                }
            }
        }
    }
    if key.starts_with("BG-") {
        return None;
    }
    let cc = country.trim().to_ascii_uppercase();
    if cc.is_empty() || cc == "BG" {
        return None;
    }
    let c = clip(t, 35);
    if c.is_empty() {
        None
    } else {
        Some(c)
    }
}

pub fn addr_type(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "STREETADDRESS" => Some("StreetAddress"),
        "POSTALADDRESS" => Some("PostalAddress"),
        "BILLINGADDRESS" => Some("BillingAddress"),
        "SHIPTOADDRESS" => Some("ShipToAddress"),
        "SHIPFROMADDRESS" => Some("ShipFromAddress"),
        _ => None,
    }
}

pub struct Addr<'a> {
    pub street: &'a str,
    pub number: &'a str,
    pub extra: &'a str,
    pub building: &'a str,
    pub city: &'a str,
    pub post: &'a str,
    pub region: &'a str,
    pub country: &'a str,
    pub kind: &'a str,
}

pub fn address(tag: &str, a: &Addr<'_>) -> String {
    let cc = country(a.country);
    let mut out = open(tag);
    out.push_str(&opt("StreetName", &clip(a.street, 70)));
    out.push_str(&opt("Number", &clip(a.number, 18)));
    out.push_str(&opt("AdditionalAddressDetail", &clip(a.extra, 70)));
    out.push_str(&opt("Building", &clip(a.building, 35)));
    out.push_str(&text("City", &clip(a.city, 35)));
    out.push_str(&opt("PostalCode", &clip(a.post, 18)));
    if let Some(code) = region_code(a.region, &cc) {
        out.push_str(&text("Region", &code));
    }
    out.push_str(&text("Country", &cc));
    if let Some(kind) = addr_type(a.kind) {
        out.push_str(&text("AddressType", kind));
    }
    out.push_str(&close(tag));
    out
}

pub fn ymd(y: i32, m: i32, d: i32) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

pub struct Window {
    pub fy_start: String,
    pub start: String,
    pub next: String,
    pub prefix: String,
}

pub fn window(year: i32, month: i32, fys: i32) -> Window {
    let start_m = if (1..=12).contains(&fys) { fys } else { 1 };
    let fy = if month < start_m { year - 1 } else { year };
    let (ny, nm) = if month >= 12 { (year + 1, 1) } else { (year, month + 1) };
    Window {
        fy_start: ymd(fy, start_m, 1),
        start: ymd(year, month, 1),
        next: ymd(ny, nm, 1),
        prefix: format!("{year:04}-{month:02}"),
    }
}

/// `o` е движение преди месеца, `p` е месецът. Датата е YYYY-MM-DD.
pub fn bucket(date: &str, w: &Window) -> Option<char> {
    let date = &date[..date.len().min(10)];
    if date.len() < 10 {
        return None;
    }
    if date >= w.fy_start.as_str() && date < w.start.as_str() {
        Some('o')
    } else if date >= w.start.as_str() && date < w.next.as_str() {
        Some('p')
    } else {
        None
    }
}

pub fn unit_name(code: &str) -> String {
    if let Some(rows) = crate::saft::catalog().get("units") {
        for line in rows {
            let mut p = line.split('|');
            let c = p.next().unwrap_or("");
            let name = p.next().unwrap_or("");
            if c == code && !name.is_empty() {
                return clip(name, 256);
            }
        }
    }
    if code.is_empty() {
        "Единица".into()
    } else {
        clip(code, 256)
    }
}

pub fn stock_moves() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(rows) = crate::saft::catalog().get("stock_movements") else {
        return out;
    };
    for line in rows {
        let mut p = line.split('|');
        let code = p.next().unwrap_or("").trim();
        let name = p.next().unwrap_or("").trim();
        if !code.is_empty() {
            out.push((clip(code, 9), clip(name, 256)));
        }
    }
    out
}

pub fn or_text(value: &str, fallback: &str) -> String {
    let t = value.trim();
    if t.is_empty() {
        fallback.to_string()
    } else {
        t.to_string()
    }
}

pub fn shown_name(name: &str, n: usize) -> String {
    let t = clip(name, n);
    if t.is_empty() {
        "—".into()
    } else {
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sofia_is_bg22() {
        assert_eq!(region_code("София", "BG").as_deref(), Some("BG-22"));
        assert_eq!(region_code("BG22", "BG").as_deref(), Some("BG-22"));
        assert_eq!(region_code("Sofia", "BG"), None);
        assert_eq!(region_code("", "BG"), None);
        assert_eq!(region_code("Attica", "GR").as_deref(), Some("Attica"));
    }

    #[test]
    fn nra_falls_back_to_firm_number() {
        assert!(pack("Asset", "411").unwrap().0 == "411");
        assert!(pack("4532", "702").unwrap().0 == "4532");
        assert!(pack("Liability", "12").is_none());
        assert!(pack("0453", "0453").is_none());
    }

    #[test]
    fn equal_sides_are_debit() {
        let zero = bal("OpeningDebitBalance", "OpeningCreditBalance", 0, 0);
        assert!(zero.contains("OpeningDebitBalance"));
        assert!(zero.contains("0.00"));
        let credit = bal("OpeningDebitBalance", "OpeningCreditBalance", 50_000_000, 200_000_000);
        assert!(credit.contains("OpeningCreditBalance"));
        assert!(credit.contains("150.00"));
    }
}
