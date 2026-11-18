use super::common::*;
use super::{Parsed, Tx};
use crate::dec::{self, money};

pub fn parse(content: &str) -> Parsed {
    let mut result = Parsed { format_name: "MT940".into(), account_iban: String::new(), account_currency: String::new(), txs: Vec::new() };
    let mut pending: Option<Tx> = None;
    for raw in content.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix(":25:") {
            let t = rest.trim();
            if t.starts_with("BG") || t.starts_with("LT") {
                result.account_iban = t.to_string();
            }
        }
        if line.starts_with(":60F:") {
            let bytes = line.as_bytes();
            for i in 0..bytes.len().saturating_sub(2) {
                if bytes[i].is_ascii_alphabetic() && bytes[i + 1].is_ascii_alphabetic() && bytes[i + 2].is_ascii_alphabetic() {
                    let cur = &line[i..i + 3];
                    if cur.chars().all(|c| c.is_ascii_uppercase()) && cur != "F" {
                        result.account_currency = cur.to_string();
                        break;
                    }
                }
            }
        }
        if let Some(rest) = line.strip_prefix(":61:") {
            if let Some(tx) = pending.take() {
                result.txs.push(tx);
            }
            if let Some(tx) = mt_line(rest, &result.account_currency) {
                pending = Some(tx);
            }
        } else if let Some(rest) = line.strip_prefix(":86:") {
            if let Some(tx) = pending.as_mut() {
                let extra = rest.trim();
                if !extra.is_empty() {
                    if !tx.description.is_empty() { tx.description.push(' '); }
                    tx.description.push_str(extra);
                }
            }
        }
    }
    if let Some(tx) = pending.take() {
        result.txs.push(tx);
    }
    if result.account_currency.is_empty() {
        result.account_currency = "EUR".into();
    }
    result
}

fn mt_line(rest: &str, currency: &str) -> Option<Tx> {
    let chars: Vec<char> = rest.chars().collect();
    if chars.len() < 7 { return None; }
    let y = rest.get(0..2)?;
    let m = rest.get(2..4)?;
    let d = rest.get(4..6)?;
    let date = format!("20{y}-{m}-{d}");
    let mut i = 6;
    if chars.get(i).map(|c| c.is_ascii_digit()).unwrap_or(false) && chars.get(i + 1).map(|c| c.is_ascii_digit()).unwrap_or(false)
        && chars.get(i + 2).map(|c| c.is_ascii_digit()).unwrap_or(false) && chars.get(i + 3).map(|c| c.is_ascii_digit()).unwrap_or(false)
    {
        i += 4;
    }
    let sign = *chars.get(i)?;
    if sign != 'C' && sign != 'D' && sign != 'c' && sign != 'd' { return None; }
    i += 1;
    let mut amt = String::new();
    while let Some(c) = chars.get(i) {
        if c.is_ascii_digit() || *c == ',' || *c == '.' { amt.push(*c); i += 1; } else { break; }
    }
    let amount = norm_money(&amt);
    if amount.is_empty() || amount == "0.00" { return None; }
    let amount = if sign.eq_ignore_ascii_case(&'D') { format!("-{}", amount.trim_start_matches('-')) } else { amount };
    let reference = rest.get(i..).unwrap_or("").trim().to_string();
    Some(Tx { date, amount, currency: currency.to_string(), description: String::new(), contra_name: String::new(), contra_iban: String::new(), reference, is_duplicate: false })
}

