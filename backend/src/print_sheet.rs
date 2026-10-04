//! Печат на документ: PDF, DOCX и ODT. Името е номерът и видът на файла.

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::reports_pdf::{self, Flow};
use crate::reports_sheet;
use crate::{api_err, ApiError};

pub enum Block {
    Head(String),
    Line(String),
    Table(Vec<Vec<String>>),
}

pub fn render(blocks: &[Block], format: &str) -> Result<Vec<u8>, ApiError> {
    match format {
        "pdf" => reports_pdf::render_flow(&flow(blocks)),
        "docx" => Ok(docx(blocks)),
        "odt" => Ok(odt(blocks)),
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

fn flow(blocks: &[Block]) -> Vec<Flow> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Head(s) => Flow::Head(s.clone()),
            Block::Line(s) => Flow::Line(s.clone()),
            Block::Table(rows) => Flow::Table(rows.clone()),
        })
        .collect()
}

fn docx(blocks: &[Block]) -> Vec<u8> {
    let mut body = String::new();
    for block in blocks {
        match block {
            Block::Head(t) => body.push_str(&para(t, true)),
            Block::Line(t) => body.push_str(&para(t, false)),
            Block::Table(rows) => body.push_str(&docx_table(rows)),
        }
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
         <w:body>{body}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/></w:sectPr></w:body></w:document>"
    );
    reports_sheet::pack(&[
        ("[Content_Types].xml", DOCX_TYPES.as_bytes()),
        ("_rels/.rels", DOCX_RELS.as_bytes()),
        ("word/document.xml", xml.as_bytes()),
    ])
}

fn para(text: &str, bold: bool) -> String {
    let rpr = if bold { "<w:rPr><w:b/></w:rPr>" } else { "" };
    format!("<w:p><w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", xml_escape(text))
}

fn docx_table(rows: &[Vec<String>]) -> String {
    let mut out = String::from("<w:tbl>");
    for row in rows {
        out.push_str("<w:tr>");
        for cell in row {
            out.push_str(&format!(
                "<w:tc><w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:tc>",
                xml_escape(cell)
            ));
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    out
}

fn odt(blocks: &[Block]) -> Vec<u8> {
    let mut body = String::new();
    for block in blocks {
        match block {
            Block::Head(t) => body.push_str(&format!("<text:h text:outline-level=\"1\">{}</text:h>", xml_escape(t))),
            Block::Line(t) => body.push_str(&format!("<text:p>{}</text:p>", xml_escape(t))),
            Block::Table(rows) => body.push_str(&odt_table(rows)),
        }
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-content xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
         xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
         xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" office:version=\"1.2\">\
         <office:body><office:text>{body}</office:text></office:body></office:document-content>"
    );
    reports_sheet::pack(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("META-INF/manifest.xml", ODT_MANIFEST.as_bytes()),
        ("content.xml", xml.as_bytes()),
    ])
}

fn odt_table(rows: &[Vec<String>]) -> String {
    let mut out = String::from("<table:table>");
    for row in rows {
        out.push_str("<table:table-row>");
        for cell in row {
            out.push_str(&format!("<table:table-cell><text:p>{}</text:p></table:table-cell>", xml_escape(cell)));
        }
        out.push_str("</table:table-row>");
    }
    out.push_str("</table:table>");
    out
}

const DOCX_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
</Types>";

const DOCX_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
</Relationships>";

const ODT_MANIFEST: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\">\
<manifest:file-entry manifest:full-path=\"/\" manifest:media-type=\"application/vnd.oasis.opendocument.text\"/>\
<manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/>\
</manifest:manifest>";

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
    fn docx_and_odt_keep_cyrillic() {
        let docx = render(&sample(), "docx").expect("docx");
        let odt = render(&sample(), "odt").expect("odt");
        assert_eq!(&docx[..2], b"PK");
        assert_eq!(&odt[..2], b"PK");
        assert_eq!(&odt[30..38], b"mimetype");
        assert!(docx.windows(14).any(|w| w == "ФАКТУРА".as_bytes()));
        assert!(odt.windows(14).any(|w| w == "ФАКТУРА".as_bytes()));
    }

    #[test]
    fn pdf_is_pdf() {
        let pdf = render(&sample(), "pdf").expect("pdf");
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.windows(5).any(|w| w == b"%%EOF"));
    }
}
