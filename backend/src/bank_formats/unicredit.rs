use super::common::*;
use super::{Parsed, Tx};
use crate::dec::{self, money};

pub fn parse(content: &str) -> Parsed {
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

