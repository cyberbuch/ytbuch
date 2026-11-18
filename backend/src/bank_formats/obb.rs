use super::common::*;
use super::{Parsed, Tx};
use crate::dec::{self, money};

pub fn parse(content: &str) -> Parsed {
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

