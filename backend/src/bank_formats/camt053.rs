use super::common::*;
use super::{Parsed, Tx};
use crate::dec::{self, money};

pub fn parse(content: &str) -> Parsed {
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

