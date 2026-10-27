//! CSV, MT940 и XML банкови извлечения (Postbank, UniCredit, OBB, ISO camt.053).

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
    let trimmed = content.trim().trim_start_matches('\u{feff}');
    if trimmed.contains("<AccountTransactionsHistory") {
        return Some(parse_postbank_xml(trimmed));
    }
    if trimmed.contains("<AccountMovement>") {
        return Some(parse_unicredit_xml(trimmed));
    }
    if trimmed.contains("<IBAN_S>") || trimmed.contains("<POST_DATE>") {
        return Some(parse_obb_xml(trimmed));
    }
    if trimmed.contains("BkToCstmrStmt") || trimmed.contains("<Ntry>") || trimmed.contains(":Ntry>") {
        return Some(parse_camt053(trimmed));
    }
    if trimmed.contains(":20:") || trimmed.contains(":60F:") || trimmed.contains(":61:") {
        return Some(parse_mt940(trimmed));
    }
    let lower = trimmed.to_lowercase();
    if (trimmed.contains(';') || trimmed.contains(',') || trimmed.contains('|'))
        && (lower.contains("iban") || lower.contains("дата") || lower.contains("date"))
    {
        return Some(parse_csv(trimmed));
    }
    None
}

fn parse_csv(content: &str) -> Parsed {
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
    // Срезът до 10 байта е само при ASCII — иначе не е на граница на знак.
    if t.len() >= 10
        && t.as_bytes()[..10].iter().all(u8::is_ascii)
        && t.as_bytes().get(4) == Some(&b'-')
        && t.as_bytes().first().is_some_and(u8::is_ascii_digit)
    {
        return t[..10].to_string();
    }
    let sep = if t.contains('/') { '/' } else if t.contains('.') { '.' } else if t.contains('-') { '-' } else { return String::new() };
    let p: Vec<&str> = t.split(sep).collect();
    if p.len() != 3 { return String::new(); }
    let (y, m, d) = if p[0].len() == 4 { (p[0], p[1], p[2]) } else if p[2].len() == 4 { (p[2], p[1], p[0]) } else { return String::new() };
    if y.len() != 4 { return String::new(); }
    format!("{y}-{:0>2}-{:0>2}", m.trim(), d.trim().chars().take(2).collect::<String>())
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn xml_tag(block: &str, name: &str) -> String {
    let close = format!("</{name}>");
    let Some(end) = block.find(&close) else {
        return String::new();
    };
    let head = &block[..end];
    let open = format!("<{name}");
    let Some(pos) = head.rfind(&open) else {
        return String::new();
    };
    let after = &head[pos + open.len()..];
    if after.starts_with(" nil=\"true\"") || after.starts_with('/') {
        return String::new();
    }
    let Some(gt) = after.find('>') else {
        return String::new();
    };
    xml_unescape(after[gt + 1..].trim())
}

fn xml_attr(block: &str, name: &str, attr: &str) -> String {
    let open = format!("<{name}");
    let Some(pos) = block.find(&open) else {
        return String::new();
    };
    let rest = &block[pos + open.len()..];
    let Some(gt) = rest.find('>') else {
        return String::new();
    };
    let tag_head = &rest[..gt];
    let needle = format!("{attr}=\"");
    let Some(apos) = tag_head.find(&needle) else {
        return String::new();
    };
    let val = &tag_head[apos + needle.len()..];
    val.find('"').map(|i| val[..i].to_string()).unwrap_or_default()
}

fn xml_blocks<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut out = Vec::new();
    let mut cur = xml;
    while let Some(s) = cur.find(&open) {
        let rest = &cur[s + open.len()..];
        let ok = rest.starts_with('>') || rest.starts_with(' ') || rest.starts_with('\t') || rest.starts_with('\r') || rest.starts_with('\n');
        if !ok {
            cur = rest;
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let body = &rest[gt + 1..];
        let Some(e) = body.find(&close) else { break };
        out.push(&body[..e]);
        cur = &body[e + close.len()..];
    }
    out
}

fn join_desc(parts: &[String]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" — ")
}

fn parse_postbank_xml(content: &str) -> Parsed {
    let mut result = Parsed {
        format_name: "PostBank XML".into(),
        account_iban: String::new(),
        account_currency: String::new(),
        txs: Vec::new(),
    };
    for b in xml_blocks(content, "AccountTransactionsHistoryModel") {
        let iban = xml_tag(b, "AccountNumber");
        if result.account_iban.is_empty() && !iban.is_empty() {
            result.account_iban = iban;
        }
        let local_block = xml_blocks(b, "AmountLocalCurrency").first().copied().unwrap_or("");
        let tx_block = xml_blocks(b, "TransactionAmount").first().copied().unwrap_or("");
        let mut raw_val = xml_tag(local_block, "Value");
        let mut cur = xml_tag(local_block, "Currency");
        if raw_val.is_empty() || norm_money(&raw_val) == "0.00" {
            raw_val = xml_tag(tx_block, "Value");
            cur = xml_tag(tx_block, "Currency");
        }
        if result.account_currency.is_empty() && !cur.is_empty() {
            result.account_currency = cur.clone();
        }
        let amt = norm_money(&raw_val);
        if amt.is_empty() || amt == "0.00" {
            continue;
        }
        let dir = xml_tag(b, "FlowDirection");
        let amount = if dir.eq_ignore_ascii_case("Outbound") || dir.eq_ignore_ascii_case("Debit") {
            format!("-{}", amt.trim_start_matches('-'))
        } else {
            amt.trim_start_matches('-').to_string()
        };
        let mut date = flex_date(&xml_tag(b, "BookingDate"));
        if date.is_empty() {
            date = flex_date(&xml_tag(b, "ValueDate"));
        }
        if date.is_empty() {
            continue;
        }
        let rem = xml_tag(b, "RemittanceInformation");
        let ttype = xml_tag(b, "TransactionType");
        let description = if !rem.is_empty() { rem } else { ttype };
        result.txs.push(Tx {
            date,
            amount,
            currency: if cur.is_empty() { "EUR".into() } else { cur },
            description,
            contra_name: xml_tag(b, "CorrespondentName"),
            contra_iban: xml_tag(b, "CorrespondentAccount"),
            reference: xml_tag(b, "BorderNumber"),
            is_duplicate: false,
        });
    }
    if result.account_currency.is_empty() {
        result.account_currency = "EUR".into();
    }
    result
}

fn parse_unicredit_xml(content: &str) -> Parsed {
    let mut result = Parsed {
        format_name: "UniCredit XML".into(),
        account_iban: String::new(),
        account_currency: String::new(),
        txs: Vec::new(),
    };
    for b in xml_blocks(content, "AccountMovement") {
        let acc = xml_blocks(b, "Account").first().copied().unwrap_or("");
        let iban = xml_tag(acc, "IBAN");
        let acc_ccy = xml_tag(acc, "CCY");
        if result.account_iban.is_empty() && !iban.is_empty() {
            result.account_iban = iban;
        }
        if result.account_currency.is_empty() && !acc_ccy.is_empty() {
            result.account_currency = acc_ccy;
        }
        let amt = norm_money(&xml_tag(b, "Amount"));
        if amt.is_empty() || amt == "0.00" {
            continue;
        }
        let mtype = xml_tag(b, "MovementType");
        let amount = if mtype.eq_ignore_ascii_case("Debit") {
            format!("-{}", amt.trim_start_matches('-'))
        } else {
            amt.trim_start_matches('-').to_string()
        };
        let mut date = flex_date(&xml_tag(b, "PaymentDate"));
        if date.is_empty() {
            date = flex_date(&xml_tag(b, "ValueDate"));
        }
        if date.is_empty() {
            continue;
        }
        let ccy = xml_tag(b, "CCY");
        let reason = xml_tag(b, "Reason");
        let nar = xml_tag(b, "NarrativeI02");
        let nar0 = xml_tag(b, "Narrative");
        let description = join_desc(&[reason, nar, nar0]);
        result.txs.push(Tx {
            date,
            amount,
            currency: if ccy.is_empty() { result.account_currency.clone() } else { ccy },
            description,
            contra_name: xml_tag(b, "OppositeSideName"),
            contra_iban: xml_tag(b, "OppositeSideAccount"),
            reference: xml_tag(b, "DocumentReference"),
            is_duplicate: false,
        });
    }
    if result.account_currency.is_empty() {
        result.account_currency = "EUR".into();
    }
    result
}

fn parse_obb_xml(content: &str) -> Parsed {
    let mut result = Parsed {
        format_name: "OBB XML".into(),
        account_iban: xml_tag(content, "IBAN_S"),
        account_currency: xml_tag(content, "CURRENCY"),
        txs: Vec::new(),
    };
    if result.account_currency.is_empty() {
        result.account_currency = "EUR".into();
    }
    let mut blocks = xml_blocks(content, "TRANSACTION");
    if blocks.is_empty() {
        blocks = xml_blocks(content, "ROW");
    }
    if blocks.is_empty() {
        blocks = xml_blocks(content, "RECORD");
    }
    if blocks.is_empty() && content.contains("<POST_DATE>") {
        blocks.push(content);
    }
    for b in blocks {
        let date = flex_date(&xml_tag(b, "POST_DATE"));
        if date.is_empty() {
            continue;
        }
        let d = norm_money(&xml_tag(b, "AMOUNT_D"));
        let c = norm_money(&xml_tag(b, "AMOUNT_C"));
        let amount = if !c.is_empty() && c != "0.00" {
            c.trim_start_matches('-').to_string()
        } else if !d.is_empty() && d != "0.00" {
            format!("-{}", d.trim_start_matches('-'))
        } else {
            continue;
        };
        let description = join_desc(&[
            xml_tag(b, "REM_I"),
            xml_tag(b, "REM_II"),
            xml_tag(b, "TR_NAME"),
        ]);
        result.txs.push(Tx {
            date,
            amount,
            currency: result.account_currency.clone(),
            description,
            contra_name: xml_tag(b, "NAME_R"),
            contra_iban: xml_tag(b, "IBAN_R"),
            reference: xml_tag(b, "REFERENCE"),
            is_duplicate: false,
        });
    }
    result
}

fn parse_camt053(content: &str) -> Parsed {
    let mut result = Parsed {
        format_name: "ISO camt.053".into(),
        account_iban: String::new(),
        account_currency: String::new(),
        txs: Vec::new(),
    };
    if let Some(acct) = xml_blocks(content, "Acct").first() {
        result.account_iban = xml_tag(acct, "IBAN");
        result.account_currency = xml_tag(acct, "Ccy");
    }
    for ntry in xml_blocks(content, "Ntry") {
        let amt = norm_money(&xml_tag(ntry, "Amt"));
        if amt.is_empty() || amt == "0.00" {
            continue;
        }
        let mut ccy = xml_attr(ntry, "Amt", "Ccy");
        if ccy.is_empty() {
            ccy = result.account_currency.clone();
        }
        if result.account_currency.is_empty() && !ccy.is_empty() {
            result.account_currency = ccy.clone();
        }
        let ind = xml_tag(ntry, "CdtDbtInd");
        let is_debit = ind.eq_ignore_ascii_case("DBIT") || ind.eq_ignore_ascii_case("D");
        let amount = if is_debit {
            format!("-{}", amt.trim_start_matches('-'))
        } else {
            amt.trim_start_matches('-').to_string()
        };
        let book = xml_blocks(ntry, "BookgDt").first().copied().unwrap_or(ntry);
        let val = xml_blocks(ntry, "ValDt").first().copied().unwrap_or(ntry);
        let mut date = flex_date(&xml_tag(book, "Dt"));
        if date.is_empty() {
            date = flex_date(&xml_tag(book, "DtTm"));
        }
        if date.is_empty() {
            date = flex_date(&xml_tag(val, "Dt"));
        }
        if date.is_empty() {
            continue;
        }
        let mut reference = xml_tag(ntry, "AcctSvcrRef");
        if reference.is_empty() {
            reference = xml_tag(ntry, "EndToEndId");
        }
        if reference == "NOTPROVIDED" {
            reference.clear();
        }
        let description = join_desc(&[xml_tag(ntry, "Ustrd"), xml_tag(ntry, "AddtlNtryInf")]);
        let mut contra_name = String::new();
        let mut contra_iban = String::new();
        if let Some(rltd) = xml_blocks(ntry, "RltdPties").first() {
            let party_tag = if is_debit { "Cdtr" } else { "Dbtr" };
            let acct_tag = if is_debit { "CdtrAcct" } else { "DbtrAcct" };
            if let Some(p) = xml_blocks(rltd, party_tag).first() {
                contra_name = xml_tag(p, "Nm");
            }
            if let Some(a) = xml_blocks(rltd, acct_tag).first() {
                contra_iban = xml_tag(a, "IBAN");
            }
        }
        result.txs.push(Tx {
            date,
            amount,
            currency: if ccy.is_empty() { "EUR".into() } else { ccy },
            description,
            contra_name,
            contra_iban,
            reference,
            is_duplicate: false,
        });
    }
    if result.account_currency.is_empty() {
        result.account_currency = "EUR".into();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_postbank_and_unicredit_xml() {
        let pb = r#"<?xml version="1.0" encoding="utf-8"?>
        <AccountTransactionsHistory>
          <AccountTransactionsHistoryModel>
            <TransactionAmount><Value>500.0</Value><Currency>BGN</Currency></TransactionAmount>
            <AmountLocalCurrency><Value>255.65</Value><Currency>EUR</Currency></AmountLocalCurrency>
            <BorderNumber>1767964</BorderNumber>
            <FlowDirection>Outbound</FlowDirection>
            <TransactionType>Вътрешно банков превод</TransactionType>
            <CorrespondentAccount>BG74BPBI79404068675201</CorrespondentAccount>
            <CorrespondentName>ДИМИТЪР СОЛУНКИН</CorrespondentName>
            <RemittanceInformation>Zaplata</RemittanceInformation>
            <BookingDate>2025-12-29T00:00:00</BookingDate>
            <AccountNumber>BG93BPBI79401071567501</AccountNumber>
          </AccountTransactionsHistoryModel>
        </AccountTransactionsHistory>"#;
        let p = detect(pb).expect("postbank");
        assert_eq!(p.format_name, "PostBank XML");
        assert_eq!(p.account_iban, "BG93BPBI79401071567501");
        assert_eq!(p.txs.len(), 1);
        assert_eq!(p.txs[0].amount, "-255.65");
        assert_eq!(p.txs[0].currency, "EUR");
        assert_eq!(p.txs[0].reference, "1767964");

        let uc = r#"<Items><AccountMovement>
          <Account><CCY>EUR</CCY><IBAN>BG43UNCR70001525183064</IBAN></Account>
          <Amount>1600</Amount><CCY>BGN</CCY><MovementType>Credit</MovementType>
          <OppositeSideAccount>BG23UBBS81551011107543</OppositeSideAccount>
          <OppositeSideName>СМАРТ ПРОФИЛ ООД</OppositeSideName>
          <PaymentDate>2025-09-30T00:00:00</PaymentDate>
          <Reason>Превод</Reason><DocumentReference>REF123</DocumentReference>
          <NarrativeI02>Ф-РА 13</NarrativeI02>
        </AccountMovement></Items>"#;
        let u = detect(uc).expect("unicredit");
        assert_eq!(u.format_name, "UniCredit XML");
        assert_eq!(u.account_iban, "BG43UNCR70001525183064");
        assert_eq!(u.txs.len(), 1);
        assert_eq!(u.txs[0].amount, "1600.00");
        assert_eq!(u.txs[0].contra_name, "СМАРТ ПРОФИЛ ООД");
        assert_eq!(u.txs[0].reference, "REF123");
    }

    #[test]
    fn parses_obb_and_camt053_xml() {
        let obb = r#"<STATEMENT><IBAN_S>BG11UBBS88881010101010</IBAN_S>
          <TRANSACTION><POST_DATE>15/01/2026</POST_DATE><AMOUNT_D>120.50</AMOUNT_D>
          <NAME_R>ТЕСТ ООД</NAME_R><IBAN_R>BG22UNCR70001511111111</IBAN_R>
          <REM_I>Фактура 5</REM_I><REFERENCE>OBB-1</REFERENCE></TRANSACTION></STATEMENT>"#;
        let o = detect(obb).expect("obb");
        assert_eq!(o.format_name, "OBB XML");
        assert_eq!(o.account_iban, "BG11UBBS88881010101010");
        assert_eq!(o.txs.len(), 1);
        assert_eq!(o.txs[0].date, "2026-01-15");
        assert_eq!(o.txs[0].amount, "-120.50");

        let camt = r#"<Document><BkToCstmrStmt><Stmt>
          <Acct><Id><IBAN>BG99BNBG96611020345678</IBAN></Id><Ccy>EUR</Ccy></Acct>
          <Ntry><Amt Ccy="EUR">350.00</Amt><CdtDbtInd>CRDT</CdtDbtInd>
          <BookgDt><Dt>2026-02-10</Dt></BookgDt><AcctSvcrRef>CAMT-9</AcctSvcrRef>
          <NtryDtls><TxDtls><RltdPties><Dbtr><Nm>КЛИЕНТ АД</Nm></Dbtr>
          <DbtrAcct><Id><IBAN>BG00TEST1234</IBAN></Id></DbtrAcct></RltdPties>
          <RmtInf><Ustrd>Плащане по ф-ра 99</Ustrd></RmtInf></TxDtls></NtryDtls></Ntry>
        </Stmt></BkToCstmrStmt></Document>"#;
        let c = detect(camt).expect("camt");
        assert_eq!(c.format_name, "ISO camt.053");
        assert_eq!(c.account_iban, "BG99BNBG96611020345678");
        assert_eq!(c.txs.len(), 1);
        assert_eq!(c.txs[0].amount, "350.00");
        assert_eq!(c.txs[0].contra_name, "КЛИЕНТ АД");
        assert_eq!(c.txs[0].reference, "CAMT-9");
    }
}
