//! ППДДС: период, вид документ, Windows-1251 и фиксирана ширина.

use crate::invoice_money::{cents, money};

pub struct Period {
    pub iso: String,
    pub nap: String,
}

#[derive(Debug)]
pub enum PeriodBad {
    Empty,
    Format,
}

pub fn parse_period(raw: &str) -> Result<Period, PeriodBad> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(PeriodBad::Empty);
    }
    let (ys, ms) = if t.len() == 7 && t.as_bytes().get(4) == Some(&b'-') {
        (&t[..4], &t[5..7])
    } else if t.len() == 6 && t.bytes().all(|b| b.is_ascii_digit()) {
        (&t[..4], &t[4..6])
    } else {
        return Err(PeriodBad::Format);
    };
    if !ys.bytes().all(|b| b.is_ascii_digit()) || !ms.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PeriodBad::Format);
    }
    let (Ok(y), Ok(m)) = (ys.parse::<i32>(), ms.parse::<i32>()) else {
        return Err(PeriodBad::Format);
    };
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) {
        return Err(PeriodBad::Format);
    }
    Ok(Period {
        iso: format!("{y:04}-{m:02}"),
        nap: format!("{y:04}{m:02}"),
    })
}

pub fn doc_type(t: &str) -> String {
    match t {
        "invoice" => "01".into(),
        "debit_note" => "02".into(),
        "credit_note" => "03".into(),
        "proforma" => "01".into(),
        _ if t.len() == 1 && t.bytes().all(|b| b.is_ascii_digit()) => format!("0{t}"),
        _ if t.len() >= 2 => t[..2].to_string(),
        _ => "01".into(),
    }
}

pub fn is_credit_doc(t: &str) -> bool {
    matches!(t, "03" | "13" | "23" | "29")
}

pub fn sales_excluded(t: &str) -> bool {
    matches!(t, "04" | "11" | "12" | "13")
}

pub fn purch_excluded(t: &str) -> bool {
    matches!(t, "05" | "11" | "12" | "13" | "94")
}

pub fn rate_kind(raw: &str) -> &'static str {
    match cents(raw) {
        None => "20",
        Some(0) => "0",
        Some(900) => "9",
        _ => "20",
    }
}

pub fn nap_date(iso: &str) -> String {
    if iso.len() >= 10 && iso.is_char_boundary(10) {
        format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[0..4])
    } else {
        "          ".into()
    }
}

pub fn with_bg(v: &str) -> String {
    let t = v.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.starts_with("BG") {
        return t.to_string();
    }
    if t.len() >= 2 && t.starts_with("bg") {
        return format!("BG{}", &t[2..]);
    }
    format!("BG{t}")
}

pub fn cp1251(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let u = ch as u32;
        let b = if u < 128 {
            u as u8
        } else if (0x0410..=0x044F).contains(&u) {
            (u - 848) as u8
        } else {
            match u {
                0x0401 => 168,
                0x0451 => 184,
                0x201C => 147,
                0x201D => 148,
                0x201E => 132,
                0x2013 => 150,
                0x2014 => 151,
                0x2116 => 185,
                _ => 63,
            }
        };
        out.push(b);
    }
    out
}

pub fn pad_right(src: &[u8], n: usize) -> Vec<u8> {
    let mut out = vec![b' '; n];
    let m = src.len().min(n);
    out[..m].copy_from_slice(&src[..m]);
    out
}

pub fn pad_left(src: &[u8], n: usize) -> Vec<u8> {
    let mut out = vec![b' '; n];
    if src.len() >= n {
        out.copy_from_slice(&src[src.len() - n..]);
    } else {
        let start = n - src.len();
        out[start..].copy_from_slice(src);
    }
    out
}

pub fn sym(s: &str, n: usize) -> Vec<u8> {
    pad_right(&cp1251(s), n)
}

pub fn num_str(s: &str, n: usize) -> Vec<u8> {
    pad_left(&cp1251(s), n)
}

pub fn amt(cents: i64) -> Vec<u8> {
    num_str(&money(cents), 15)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_forms() {
        let p = parse_period("202610").unwrap();
        assert_eq!(p.iso, "2026-10");
        assert_eq!(p.nap, "202610");
        assert!(matches!(parse_period(""), Err(PeriodBad::Empty)));
        assert!(matches!(parse_period("2026-13"), Err(PeriodBad::Format)));
        assert!(matches!(parse_period("2026/10"), Err(PeriodBad::Format)));
    }

    #[test]
    fn cyrillic_byte() {
        assert_eq!(cp1251("Бага"), vec![0xC1, 0xE0, 0xE3, 0xE0]);
        assert_eq!(cp1251("A"), vec![b'A']);
    }
}
