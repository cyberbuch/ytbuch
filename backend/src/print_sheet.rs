//! Печат на документ: PDF, DOCX и ODT. Името е номерът и видът на файла.
//! Генерацията е в пакета `report`.

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::report;
use crate::{api_err, ApiError};

pub use crate::report::Block;

pub fn render(blocks: &[Block], format: &str) -> Result<Vec<u8>, ApiError> {
    match format {
        "pdf" => report::pdf(blocks),
        "docx" | "odt" => report::office(blocks, format),
        _ => Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "format трябва да е pdf/docx/odt")),
    }
}

pub fn attachment(bytes: Vec<u8>, mime: &str, filename: &str) -> Response {
    let disp = format!("attachment; filename=\"{filename}\"");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_DISPOSITION, disp)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

pub fn mime(format: &str) -> &'static str {
    match format {
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "odt" => "application/vnd.oasis.opendocument.text",
        _ => "application/pdf",
    }
}

pub fn download_name(number: &str, document_type: &str, ext: &str) -> String {
    format!("{}_{}.{}", safe_name(number), file_slug(document_type), ext)
}

pub fn safe_name(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| match c {
            '"' | '/' | '\\' | ':' | '*' | '?' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    let t = t.trim();
    if t.is_empty() { "document".into() } else { t.to_string() }
}

pub fn file_slug(t: &str) -> &'static str {
    match t {
        "02" => "debit_note",
        "03" => "credit_note",
        "04" => "call_off_dispatch",
        "05" => "call_off_receipt",
        "07" => "customs_declaration",
        "09" => "protocol",
        "11" => "invoice_cash",
        "12" => "debit_note_cash",
        "13" => "credit_note_cash",
        "23" => "credit_note_126b",
        "29" => "protocol_126b",
        "50" => "fuel_protocol",
        "81" => "sales_report",
        "82" => "sales_report_special",
        "83" => "sales_report_fuel",
        "84" => "sales_report_bread",
        "85" => "sales_report_flour",
        "91" => "vat_protocol",
        "92" => "vat_protocol_credit",
        "93" => "vat_protocol_151v",
        "94" => "vat_protocol_151v_scheme",
        "95" => "food_protocol",
        "proforma" => "proforma",
        _ => "invoice",
    }
}

pub fn doc_title(t: &str) -> &'static str {
    match t {
        "02" => "ДЕБИТНО ИЗВЕСТИЕ",
        "03" => "КРЕДИТНО ИЗВЕСТИЕ",
        "04" | "05" => "РЕГИСТЪР НА СТОКИ ПОД РЕЖИМ СКЛАДИРАНЕ ДО ПОИСКВАНЕ",
        "07" => "МИТНИЧЕСКА ДЕКЛАРАЦИЯ",
        "09" => "ПРОТОКОЛ",
        "11" => "ФАКТУРА - КАСОВА ОТЧЕТНОСТ",
        "12" => "ДЕБИТНО ИЗВЕСТИЕ - КАСОВА ОТЧЕТНОСТ",
        "13" => "КРЕДИТНО ИЗВЕСТИЕ - КАСОВА ОТЧЕТНОСТ",
        "23" => "КРЕДИТНО ИЗВЕСТИЕ ПО ЧЛ. 126Б, АЛ. 1 ОТ ЗДДС",
        "29" => "ПРОТОКОЛ ПО ЧЛ. 126Б, АЛ. 2 И 7 ОТ ЗДДС",
        "81" | "83" | "84" | "85" => "ОТЧЕТ ЗА ИЗВЪРШЕНИТЕ ПРОДАЖБИ",
        "82" => "ОТЧЕТ ЗА ПРОДАЖБИ ПРИ СПЕЦИАЛЕН РЕД НА ОБЛАГАНЕ",
        "proforma" => "ПРОФОРМА ФАКТУРА",
        _ => "ФАКТУРА",
    }
}

pub fn doc_prefix(t: &str) -> &'static str {
    match t {
        "02" | "12" => "ДИ-",
        "03" | "13" | "23" => "КИ-",
        "proforma" => "ПФ-",
        _ => "",
    }
}

pub fn bg_date(raw: &str) -> String {
    let s = raw.trim();
    let b = s.as_bytes();
    if s.len() >= 10 && b.get(4) == Some(&b'-') && b.get(7) == Some(&b'-') {
        return format!("{}.{}.{}", &s[8..10], &s[5..7], &s[0..4]);
    }
    if s.len() >= 8 && s.as_bytes().iter().take(8).all(u8::is_ascii_digit) {
        return format!("{}.{}.{}", &s[6..8], &s[4..6], &s[0..4]);
    }
    s.to_string()
}

pub fn xml_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c.is_control() && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Block> {
        vec![
            Block::Head("ФАКТУРА".into()),
            Block::Line("№ 0000000001".into()),
            Block::Table(vec![
                vec!["No".into(), "Стока".into()],
                vec!["1".into(), "Консултация".into()],
            ]),
        ]
    }

    #[test]
    fn names_match_the_screen() {
        assert_eq!(download_name("0000000001", "01", "pdf"), "0000000001_invoice.pdf");
        assert_eq!(download_name("0000000002", "03", "docx"), "0000000002_credit_note.docx");
        assert_eq!(download_name("a/b:c", "proforma", "odt"), "a_b_c_proforma.odt");
        assert_eq!(download_name("   ", "01", "pdf"), "document_invoice.pdf");
        assert_eq!(doc_prefix("03"), "КИ-");
        assert_eq!(bg_date("2026-10-01"), "01.10.2026");
    }

    #[test]
    fn docx_and_odt_are_archives() {
        let docx = render(&sample(), "docx").expect("docx");
        let odt = render(&sample(), "odt").expect("odt");
        assert_eq!(&docx[..2], b"PK");
        assert_eq!(&odt[..2], b"PK");
    }
}
