use super::common::*;
use super::{Parsed, Tx};
use crate::dec::{self, money};

pub fn parse(content: &str) -> Parsed {
    let mut result = Parsed { format_name: "CSV".into(), account_iban: String::new(), account_currency: String::new(), txs: Vec::new() };
    let delim = if content.contains('|') { '|' } else if content.contains(';') { ';' } else { ',' };
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
        let has_amt = lower.contains("дебит")
            || lower.contains("кредит")
            || lower.contains("сума")
            || lower.contains("amount")
            || lower.contains("плащания")
            || lower.contains("постъпления");
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
    let mut contra_col = None;
    for (c, h) in heads.iter().enumerate() {
        if date_col == 0 && (h.contains("дата") || h.contains("date")) {
            date_col = c;
        }
        if h.contains("дебит") || h.contains("debit") || h.contains("плащания") { debit_col = Some(c); }
        if h.contains("кредит") || h.contains("credit") || h.contains("постъпления") { credit_col = Some(c); }
        if h.contains("сума") || h.contains("amount") { amount_col = Some(c); }
        if contra_col.is_none() && (h.contains("кореспондент") || h.contains("контрагент") || h.contains("наредител")) {
            contra_col = Some(c);
        }
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
            contra_name: contra_col.and_then(|c| cols.get(c)).unwrap_or(&"").trim().to_string(),
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

