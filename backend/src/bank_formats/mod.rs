//! Банкови извлечения — по един модул за формат. Добавянето на нов формат е
//! нов файл тук + ред в `detect` (и по избор в `FORMATS` за ръчен избор).
//! Примерите за всеки формат са в secret/bank-xml.

pub mod camt053;
pub mod common;
pub mod csv;
pub mod mt940;
pub mod obb;
pub mod postbank;
pub mod unicredit;

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

/// Идентификатори на форматите за ръчен избор (вид автоматизация).
pub const FORMATS: [(&str, &str); 7] = [
    ("auto", "Автоматично"),
    ("camt053", "ISO camt.053"),
    ("postbank", "Postbank XML"),
    ("unicredit", "UniCredit XML"),
    ("obb", "OBB XML"),
    ("mt940", "MT940"),
    ("csv", "CSV"),
];

/// Разпознава формата по съдържанието.
pub fn detect(content: &str) -> Option<Parsed> {
    let trimmed = content.trim().trim_start_matches('\u{feff}');
    if trimmed.contains("<AccountTransactionsHistory") {
        return Some(postbank::parse(trimmed));
    }
    if trimmed.contains("<AccountMovement>") {
        return Some(unicredit::parse(trimmed));
    }
    if trimmed.contains("<IBAN_S>") || trimmed.contains("<POST_DATE>") {
        return Some(obb::parse(trimmed));
    }
    if trimmed.contains("BkToCstmrStmt") || trimmed.contains("<Ntry>") || trimmed.contains(":Ntry>") {
        return Some(camt053::parse(trimmed));
    }
    if trimmed.contains(":20:") || trimmed.contains(":60F:") || trimmed.contains(":61:") {
        return Some(mt940::parse(trimmed));
    }
    let lower = trimmed.to_lowercase();
    if (trimmed.contains(';') || trimmed.contains(',') || trimmed.contains('|'))
        && (lower.contains("iban") || lower.contains("дата") || lower.contains("date"))
    {
        return Some(csv::parse(trimmed));
    }
    None
}

/// Ръчен избор на формат (вид автоматизация на банковата сметка).
pub fn parse_as(format: &str, content: &str) -> Option<Parsed> {
    let trimmed = content.trim().trim_start_matches('\u{feff}');
    match format {
        "camt053" => Some(camt053::parse(trimmed)),
        "postbank" => Some(postbank::parse(trimmed)),
        "unicredit" => Some(unicredit::parse(trimmed)),
        "obb" => Some(obb::parse(trimmed)),
        "mt940" => Some(mt940::parse(trimmed)),
        "csv" => Some(csv::parse(trimmed)),
        _ => detect(trimmed),
    }
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
