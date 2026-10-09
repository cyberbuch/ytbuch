//! UBL 2.1 / Peppol BIS Billing 3.0. Кредитното е CreditNote. Фирмата е доставчик.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::dec::{self, money};
use crate::invoice_read::Line;
use crate::print_load::{self, Loaded, Party};
use crate::print_sheet::{self, xml_escape};
use crate::vat_ex;
use crate::{AppState, ApiError};

pub async fn export(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Response, ApiError> {
    let loaded = print_load::load(&st, &headers, id).await?;
    let xml = document(&loaded);
    let kind = if is_credit(&loaded.inv.document_type) { "credit-note" } else { "invoice" };
    let filename = format!("ubl-{kind}-{}.xml", print_sheet::safe_name(&loaded.inv.number));
    let disp = format!("attachment; filename=\"{filename}\"");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .header(header::CONTENT_DISPOSITION, disp)
        .body(axum::body::Body::from(xml))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

fn document(doc: &Loaded) -> String {
    let inv = &doc.inv;
    let credit = is_credit(&inv.document_type);
    let company = print_load::party(&doc.company);
    let mut customer = print_load::party(&doc.counterpart);
    if customer.name.is_empty() {
        customer.name = inv.counterpart_name.clone();
    }
    let currency = if inv.currency.trim().is_empty() { "EUR".into() } else { inv.currency.trim().to_string() };
    let root = if credit { "CreditNote" } else { "Invoice" };
    let ns = if credit {
        "urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2"
    } else {
        "urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"
    };
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <{root} xmlns=\"{ns}\" \
         xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" \
         xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\">"
    );
    out.push_str(&cbc("UBLVersionID", "2.1"));
    out.push_str(&cbc(
        "CustomizationID",
        "urn:cen.eu:en16931:2017#compliant#urn:fdc:peppol.eu:2017:poacc:billing:3.0",
    ));
    out.push_str(&cbc("ProfileID", "urn:fdc:peppol.eu:2017:poacc:billing:01:1.0"));
    out.push_str(&cbc("ID", &inv.number));
    out.push_str(&cbc("IssueDate", &iso_day(&inv.issue_date)));
    if credit {
        out.push_str(&cbc("CreditNoteTypeCode", "381"));
    } else {
        out.push_str(&cbc("DueDate", &iso_day(&inv.due_date)));
        out.push_str(&cbc("InvoiceTypeCode", doc_code(&inv.document_type)));
    }
    out.push_str(&cbc("Note", inv.notes.trim()));
    out.push_str(&cbc("TaxPointDate", &iso_day(&inv.tax_event_date)));
    out.push_str(&cbc("DocumentCurrencyCode", &currency));
    let tax_cur = needs_eur(&currency, &inv.currency_rate);
    if tax_cur {
        out.push_str(&cbc("TaxCurrencyCode", "EUR"));
    }
    out.push_str(&cbc("BuyerReference", &inv.number));
    if !doc.original_number.trim().is_empty() {
        out.push_str(&open("BillingReference"));
        out.push_str(&open("InvoiceDocumentReference"));
        out.push_str(&cbc("ID", doc.original_number.trim()));
        out.push_str(&cbc("IssueDate", &iso_day(&doc.original_date)));
        out.push_str(&close("InvoiceDocumentReference"));
        out.push_str(&close("BillingReference"));
    }
    out.push_str(&open("AdditionalDocumentReference"));
    out.push_str(&cbc("ID", "Software"));
    out.push_str(&cbc("DocumentDescription", "ytbuch"));
    out.push_str(&close("AdditionalDocumentReference"));
    if tax_cur {
        out.push_str(&open("TaxExchangeRate"));
        out.push_str(&cbc("SourceCurrencyCode", &currency));
        out.push_str(&cbc("TargetCurrencyCode", "EUR"));
        out.push_str(&cbc("CalculationRate", &inv_rate(&inv.currency_rate)));
        out.push_str(&close("TaxExchangeRate"));
    }
    out.push_str(&party("AccountingSupplierParty", &company));
    out.push_str(&party("AccountingCustomerParty", &customer));
    out.push_str(&open("PaymentMeans"));
    out.push_str(&cbc("PaymentMeansCode", pay_code(inv.payment_method.trim())));
    if !company.iban.is_empty() {
        out.push_str(&open("PayeeFinancialAccount"));
        out.push_str(&cbc("ID", &company.iban));
        if !company.bic.is_empty() {
            out.push_str(&open("FinancialInstitutionBranch"));
            out.push_str(&cbc("ID", &company.bic));
            out.push_str(&close("FinancialInstitutionBranch"));
        }
        out.push_str(&close("PayeeFinancialAccount"));
    }
    out.push_str(&close("PaymentMeans"));
    out.push_str(&tax_total(&doc.lines, &currency, &inv.vat_amount, inv.vat_exemption_reason.trim()));
    if tax_cur {
        out.push_str(&open("TaxTotal"));
        out.push_str(&amt("TaxAmount", "EUR", &to_eur(&inv.vat_amount, &inv.currency_rate)));
        out.push_str(&close("TaxTotal"));
    }
    let net = money(dec::or0(&inv.net_amount));
    let total = money(dec::or0(&inv.total_amount));
    out.push_str(&open("LegalMonetaryTotal"));
    out.push_str(&amt("LineExtensionAmount", &currency, &net));
    out.push_str(&amt("TaxExclusiveAmount", &currency, &net));
    out.push_str(&amt("TaxInclusiveAmount", &currency, &total));
    out.push_str(&amt("PayableAmount", &currency, &total));
    out.push_str(&close("LegalMonetaryTotal"));
    for (i, line) in doc.lines.iter().enumerate() {
        out.push_str(&line_xml(line, i, &currency, credit, inv.vat_exemption_reason.trim()));
    }
    out.push_str(&format!("</{root}>"));
    out
}

fn party(tag: &str, p: &Party) -> String {
    let cc = country(&p.vat, &p.country);
    let mut o = open(tag);
    o.push_str(&open("Party"));
    if !p.vat.is_empty() {
        o.push_str(&cbc_attr("EndpointID", &[("schemeID", scheme(&p.vat))], &p.vat));
    }
    if !p.eik.is_empty() {
        o.push_str(&open("PartyIdentification"));
        o.push_str(&cbc_attr("ID", &[("schemeID", "0195")], &p.eik));
        o.push_str(&close("PartyIdentification"));
    }
    o.push_str(&open("PartyName"));
    o.push_str(&cbc("Name", &p.name));
    o.push_str(&close("PartyName"));
    o.push_str(&open("PostalAddress"));
    o.push_str(&cbc("StreetName", &p.address));
    o.push_str(&cbc("CityName", &p.city));
    o.push_str(&cbc("PostalZone", &p.post));
    o.push_str(&open("Country"));
    o.push_str(&cbc("IdentificationCode", &cc));
    o.push_str(&close("Country"));
    o.push_str(&close("PostalAddress"));
    if !p.vat.is_empty() {
        o.push_str(&open("PartyTaxScheme"));
        o.push_str(&cbc("CompanyID", &p.vat));
        o.push_str(&open("TaxScheme"));
        o.push_str(&cbc("ID", "VAT"));
        o.push_str(&close("TaxScheme"));
        o.push_str(&close("PartyTaxScheme"));
    }
    o.push_str(&open("PartyLegalEntity"));
    o.push_str(&cbc("RegistrationName", &p.name));
    if !p.eik.is_empty() {
        o.push_str(&cbc_attr("CompanyID", &[("schemeID", "0195")], &p.eik));
    }
    o.push_str(&close("PartyLegalEntity"));
    if !p.email.is_empty() || !p.phone.is_empty() {
        o.push_str(&open("Contact"));
        o.push_str(&cbc("Telephone", &p.phone));
        o.push_str(&cbc("ElectronicMail", &p.email));
        o.push_str(&close("Contact"));
    }
    o.push_str(&close("Party"));
    o.push_str(&close(tag));
    o
}

fn tax_total(lines: &[Line], currency: &str, vat_total: &str, exemption: &str) -> String {
    let mut groups: Vec<(String, i128, i128)> = Vec::new();
    for line in lines {
        let rate = line.vat_rate.trim().to_string();
        let net = dec::or0(&line.net_amount);
        let vat = dec::or0(&line.vat_amount);
        if let Some(slot) = groups.iter_mut().find(|g| g.0 == rate) {
            slot.1 = slot.1.saturating_add(net);
            slot.2 = slot.2.saturating_add(vat);
        } else {
            groups.push((rate, net, vat));
        }
    }
    let mut o = open("TaxTotal");
    o.push_str(&amt("TaxAmount", currency, vat_total));
    for (rate, net, vat) in groups {
        o.push_str(&open("TaxSubtotal"));
        o.push_str(&amt("TaxableAmount", currency, &money(net)));
        o.push_str(&amt("TaxAmount", currency, &money(vat)));
        o.push_str(&tax_category(&rate, exemption));
        o.push_str(&close("TaxSubtotal"));
    }
    o.push_str(&close("TaxTotal"));
    o
}

fn line_xml(line: &Line, index: usize, currency: &str, credit: bool, exemption: &str) -> String {
    let (tag, qty_tag) = if credit { ("CreditNoteLine", "CreditedQuantity") } else { ("InvoiceLine", "InvoicedQuantity") };
    let rate = if line.vat_rate.trim().is_empty() { "0" } else { line.vat_rate.trim() };
    let cat = tax_id(rate, exemption);
    let mut o = open(tag);
    o.push_str(&cbc("ID", &(index + 1).to_string()));
    o.push_str(&qty_el(qty_tag, &unit_code(&line.unit), line.quantity.trim()));
    o.push_str(&amt("LineExtensionAmount", currency, &line.net_amount));
    o.push_str(&open("Item"));
    o.push_str(&cbc("Name", line.description.trim()));
    o.push_str(&open("ClassifiedTaxCategory"));
    o.push_str(&cbc("ID", cat));
    o.push_str(&cbc("Percent", rate));
    o.push_str(&open("TaxScheme"));
    o.push_str(&cbc("ID", "VAT"));
    o.push_str(&close("TaxScheme"));
    o.push_str(&close("ClassifiedTaxCategory"));
    o.push_str(&close("Item"));
    o.push_str(&open("Price"));
    o.push_str(&amt("PriceAmount", currency, &line.unit_price));
    o.push_str(&close("Price"));
    o.push_str(&close(tag));
    o
}

fn tax_category(rate: &str, exemption: &str) -> String {
    let shown = if rate.is_empty() { "0" } else { rate };
    let cat = tax_id(shown, exemption);
    let mut o = open("TaxCategory");
    o.push_str(&cbc("ID", cat));
    o.push_str(&cbc("Percent", shown));
    if cat != "S" && !exemption.is_empty() {
        let code = vat_ex::ubl_reason(exemption);
        if !code.is_empty() {
            o.push_str(&cbc("TaxExemptionReasonCode", code));
        }
        o.push_str(&cbc("TaxExemptionReason", &vat_ex::legal_en(exemption)));
    }
    o.push_str(&open("TaxScheme"));
    o.push_str(&cbc("ID", "VAT"));
    o.push_str(&close("TaxScheme"));
    o.push_str(&close("TaxCategory"));
    o
}

fn tax_id(rate: &str, exemption: &str) -> &'static str {
    if dec::or0(rate) != 0 {
        return "S";
    }
    if exemption.is_empty() {
        return "Z";
    }
    vat_ex::category(exemption)
}

fn is_credit(t: &str) -> bool {
    matches!(t, "03" | "13" | "23")
}

fn doc_code(t: &str) -> &'static str {
    match t {
        "proforma" => "325",
        "02" | "12" => "383",
        "03" | "13" | "23" => "381",
        _ => "380",
    }
}

fn pay_code(method: &str) -> &'static str {
    if method == "в брой" {
        "10"
    } else if method == "карта" {
        "48"
    } else {
        "30"
    }
}

fn unit_code(code: &str) -> String {
    match code.trim() {
        "" | "бр." | "бр" | "брой" => "C62".into(),
        "кг" | "кг." => "KGM".into(),
        "л" | "л." => "LTR".into(),
        "м" | "м." => "MTR".into(),
        "час" | "ч" => "HUR".into(),
        "ден" | "дни" => "DAY".into(),
        "пакет" | "опак." => "PK".into(),
        other if other.len() <= 3 && other.chars().all(|c| c.is_ascii_alphanumeric()) => other.to_string(),
        _ => "C62".into(),
    }
}

fn country(vat: &str, fallback: &str) -> String {
    const EU: &[&str] = &[
        "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "GR", "HR", "HU", "IE", "IT", "LT", "LU",
        "LV", "MT", "NL", "PL", "PT", "RO", "SE", "SI", "SK",
    ];
    let v = vat.trim();
    if v.len() >= 2 {
        let p = if &v[..2] == "EL" { "GR" } else { &v[..2] };
        if EU.contains(&p) {
            return p.to_string();
        }
    }
    let f = fallback.trim();
    if f.is_empty() { "BG".into() } else { f.to_string() }
}

fn scheme(vat: &str) -> &'static str {
    match country(vat, "BG").as_str() {
        "DE" => "9930",
        "AT" => "9914",
        "FR" => "9957",
        "IT" => "9906",
        "NL" => "9944",
        "PL" => "9945",
        "PT" => "9946",
        "RO" => "9948",
        "ES" => "9920",
        "SE" => "9955",
        "SK" => "9929",
        "SI" => "9928",
        "GR" => "9933",
        "HU" => "9910",
        "CZ" => "9932",
        "HR" => "9934",
        "DK" => "9917",
        "FI" | "IE" => "9935",
        _ => "9947",
    }
}

fn needs_eur(currency: &str, rate: &str) -> bool {
    let r = rate.trim();
    !currency.is_empty() && currency != "EUR" && !r.is_empty() && r != "1" && r != "1.00" && r != "1.000000"
}

fn to_eur(amount: &str, rate: &str) -> String {
    let a = dec::or0(amount);
    let r = dec::or0(rate);
    money(dec::div(a, r).unwrap_or(a))
}

fn inv_rate(rate: &str) -> String {
    let r = dec::or0(rate);
    let one = dec::parse("1").unwrap_or(0);
    if r == 0 {
        return "1".into();
    }
    dec::text(dec::div(one, r).unwrap_or(one), 6)
}

fn iso_day(raw: &str) -> String {
    let s = raw.trim();
    if s.len() >= 10 && s.as_bytes().get(4) == Some(&b'-') {
        s[..10].to_string()
    } else {
        s.to_string()
    }
}

fn cbc(name: &str, text: &str) -> String {
    let t = text.trim();
    if t.is_empty() {
        return String::new();
    }
    format!("<cbc:{name}>{}</cbc:{name}>", xml_escape(t))
}

fn cbc_attr(name: &str, attrs: &[(&str, &str)], text: &str) -> String {
    let t = text.trim();
    if t.is_empty() {
        return String::new();
    }
    let mut a = String::new();
    for (k, v) in attrs {
        a.push_str(&format!(" {k}=\"{}\"", xml_escape(v)));
    }
    format!("<cbc:{name}{a}>{}</cbc:{name}>", xml_escape(t))
}

fn amt(name: &str, currency: &str, amount: &str) -> String {
    format!(
        "<cbc:{name} currencyID=\"{}\">{}</cbc:{name}>",
        xml_escape(currency),
        xml_escape(&money(dec::or0(amount)))
    )
}

fn qty_el(name: &str, unit: &str, qty: &str) -> String {
    let v = if qty.is_empty() { "0" } else { qty };
    format!("<cbc:{name} unitCode=\"{}\">{}</cbc:{name}>", xml_escape(unit), xml_escape(v))
}

fn open(name: &str) -> String {
    format!("<cac:{name}>")
}

fn close(name: &str) -> String {
    format!("</cac:{name}>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        assert_eq!(doc_code("01"), "380");
        assert_eq!(doc_code("03"), "381");
        assert_eq!(doc_code("proforma"), "325");
        assert_eq!(pay_code("в брой"), "10");
        assert_eq!(pay_code("bank"), "30");
        assert_eq!(unit_code("HUR"), "HUR");
        assert_eq!(unit_code("бр."), "C62");
        assert_eq!(country("BG207123456", ""), "BG");
        assert_eq!(scheme("DE123"), "9930");
        assert!(needs_eur("BGN", "1.95583"));
        assert!(!needs_eur("EUR", "1"));
    }
}
