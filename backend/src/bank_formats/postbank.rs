use super::common::*;
use super::{Parsed, Tx};
use crate::dec::{self, money};

pub fn parse(content: &str) -> Parsed {
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

