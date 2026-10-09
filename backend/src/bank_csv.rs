//! CSV и кратък MT940. XML извлеченията (OBB, camt.053, Postbank) не се четат тук.

use crate::dec::{self, money};

#[derive(Clone)]
pub struct Tx {
    pub date: String,
    pub amount: String,
    pub currency: String,
    pub description: String,
    pub contra_name: String,
    pub contra_iban: String,
    pub reference: String,
    pub is_duplicate: bool,
}

pub struct Parsed {
    pub format_name: String,
    pub account_iban: String,
    pub account_currency: String,
    pub txs: Vec<Tx>,
}

pub fn detect(content: &str) -> Option<Parsed> {
    let trimmed = content.trim();
    if trimmed.contains(":20:") || trimmed.contains(":60F:") || trimmed.contains(":61:") {
        return Some(parse_mt940(trimmed));
    }
    let lower = trimmed.to_lowercase();
    if (trimmed.contains(';') || trimmed.contains(','))
        && (lower.contains("iban") || lower.contains("дата") || lower.contains("date"))
    {
        return Some(parse_csv(trimmed));
    }
    None
}

fn parse_csv(content: &str) -> Parsed {
    let mut result = Parsed { format_name: "CSV".into(), account_iban: String::new(), account_currency: String::new(), txs: Vec::new() };
    let delim = if content.contains(';') { ';' } else { ',' };
    for line in content.lines() {
        if line.contains("IBAN:") || line.contains("IBAN;") || line.contains("IBAN,") {
            for part in line.split(':') {
                let t = part.trim().trim_matches(|c| c == ';' || c == ',');
                if t.starts_with("BG") || t.starts_with("LT") {
                    result.account_iban = t.to_string();
                }
            }
        }
        if line.contains("Валута:") || line.contains("Currency:") {
            for part in line.split(':') {
                let t = part.trim();
                if t.len() == 3 && t.chars().all(|c| c.is_ascii_alphabetic()) {
                    result.account_currency = t.to_uppercase();
                }
            }
        }
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut header = None;
    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        let has_date = lower.contains("дата") || lower.contains("date");
        let has_amt = lower.contains("дебит") || lower.contains("кредит") || lower.contains("сума") || lower.contains("amount");
        if has_date && has_amt {
            header = Some(i);
            break;
        }
    }
    let Some(header_idx) = header else {
        if result.account_currency.is_empty() { result.account_currency = "EUR".into(); }
        return result;
    };
    let heads: Vec<String> = lines[header_idx].split(delim).map(|s| s.trim().to_lowercase()).collect();
    let mut date_col = 0usize;
    let mut debit_col = None;
    let mut credit_col = None;
    let mut amount_col = None;
    let mut desc_col = None;
    let mut ref_col = None;
    for (c, h) in heads.iter().enumerate() {
        if date_col == 0 && (h.contains("дата") || h.contains("date")) {
            date_col = c;
        }
        if h.contains("дебит") || h.contains("debit") { debit_col = Some(c); }
        if h.contains("кредит") || h.contains("credit") { credit_col = Some(c); }
        if h.contains("сума") || h.contains("amount") { amount_col = Some(c); }
        if desc_col.is_none() && (h.contains("описание") || h.contains("description") || h.contains("основание") || h.len() > 15) {
            desc_col = Some(c);
        }
        if ref_col.is_none() && (h.contains("референ") || h.contains("reference") || h.contains("id")) {
            ref_col = Some(c);
        }
    }
    if desc_col.is_none() && !heads.is_empty() {
        desc_col = Some(heads.len() - 1);
    }
    for line in lines.iter().skip(header_idx + 1) {
        let line = line.trim();
        if line.is_empty() { continue; }
        let cols: Vec<&str> = line.split(delim).collect();
        if cols.len() <= date_col { continue; }
        let date = flex_date(cols[date_col]);
        if date.is_empty() { continue; }
        let amount = if let (Some(d), Some(c)) = (debit_col, credit_col) {
            let debit = cols.get(d).map(|s| norm_money(s)).unwrap_or_default();
            let credit = cols.get(c).map(|s| norm_money(s)).unwrap_or_default();
            if !credit.is_empty() && credit != "0.00" {
                credit
            } else if !debit.is_empty() && debit != "0.00" {
                let bare = debit.trim_start_matches('-');
                format!("-{bare}")
            } else {
                String::new()
            }
        } else if let Some(a) = amount_col {
            cols.get(a).map(|s| norm_money(s)).unwrap_or_default()
        } else {
            String::new()
        };
        if amount.is_empty() || amount == "0.00" { continue; }
        result.txs.push(Tx {
            date,
            amount,
            currency: result.account_currency.clone(),
            description: desc_col.and_then(|c| cols.get(c)).unwrap_or(&"").trim().to_string(),
            contra_name: String::new(),
            contra_iban: String::new(),
            reference: ref_col.and_then(|c| cols.get(c)).unwrap_or(&"").trim().to_string(),
            is_duplicate: false,
        });
    }
    if result.account_currency.is_empty() {
        result.account_currency = "EUR".into();
    }
    for tx in &mut result.txs {
        if tx.currency.is_empty() {
            tx.currency = result.account_currency.clone();
        }
    }
    result
}

fn parse_mt940(content: &str) -> Parsed {
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

fn norm_money(raw: &str) -> String {
    let t = raw.trim().replace(' ', "");
    if t.is_empty() { return String::new(); }
    let t = if t.contains(',') && t.contains('.') { t.replace(',', "") } else { t.replace(',', ".") };
    match dec::parse(&t) {
        Some(v) => money(v),
        None => String::new(),
    }
}

fn flex_date(raw: &str) -> String {
    let t = raw.trim();
    if t.len() >= 10 && t.as_bytes().get(4) == Some(&b'-') && t.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return t[..10].to_string();
    }
    let sep = if t.contains('/') { '/' } else if t.contains('.') { '.' } else if t.contains('-') { '-' } else { return String::new() };
    let p: Vec<&str> = t.split(sep).collect();
    if p.len() != 3 { return String::new(); }
    let (y, m, d) = if p[0].len() == 4 { (p[0], p[1], p[2]) } else if p[2].len() == 4 { (p[2], p[1], p[0]) } else { return String::new() };
    if y.len() != 4 { return String::new(); }
    format!("{y}-{:0>2}-{:0>2}", m.trim(), d.trim().chars().take(2).collect::<String>())
}
