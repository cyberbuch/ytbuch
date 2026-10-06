//! Пакет report: един HTML5 документ към PDF и офис документи.
//! PDF минава през ironpress, DOCX и ODT през ferrodoc.
//! Шрифтът DejaVu е вграден в бинаря, за да няма зависимости от системата.

use std::io::{Cursor, Read, Write};

use axum::http::StatusCode;
use ferrodoc::Format;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{api_err, ApiError};

const FONT: &[u8] = include_bytes!("../data/DejaVuSans.ttf");
const FONT_NAME: &str = "DejaVu";

const STYLE: &str = r#"
@page { size: A4; margin: 18mm 14mm; }
body { font-family: DejaVu, sans-serif; font-size: 10pt; color: #17211c; }
h1 { font-size: 15pt; text-align: center; margin: 0 0 6pt; }
p { margin: 0 0 3pt; }
p.gap { height: 6pt; }
table { width: 100%; border-collapse: collapse; margin: 0 0 8pt; }
th, td { border: 1px solid #999; padding: 3pt 5pt; text-align: left; vertical-align: top; }
th { background: #eef6f2; font-weight: bold; }
table.totals { width: 60%; margin-left: auto; }
table.totals td { border: none; border-top: 1px solid #999; }
table.totals td+td { text-align: right; }
"#;

pub enum Block {
    Head(String),
    Line(String),
    Table(Vec<Vec<String>>),
    /// Таблица „етикет — сума“. Сумата е вдясно, до края на листа.
    Totals(Vec<Vec<String>>),
}

pub fn html(blocks: &[Block]) -> String {
    let mut body = String::new();
    for block in blocks {
        match block {
            Block::Head(t) => body.push_str(&format!("<h1>{}</h1>", esc(t))),
            Block::Line(t) if t.is_empty() => body.push_str("<p class=\"gap\"></p>"),
            Block::Line(t) => body.push_str(&format!("<p>{}</p>", esc(t))),
            Block::Table(rows) => body.push_str(&table(rows)),
            Block::Totals(rows) => body.push_str(&totals(rows)),
        }
    }
    format!(
        "<!doctype html><html lang=\"bg\"><head><meta charset=\"utf-8\">\
         <style>{STYLE}</style></head><body>{body}</body></html>"
    )
}

pub fn pdf(blocks: &[Block]) -> Result<Vec<u8>, ApiError> {
    ironpress::HtmlConverter::new()
        .add_font(FONT_NAME, FONT.to_vec())
        .sanitize(false)
        .convert(&html(blocks))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub fn office(blocks: &[Block], format: &str) -> Result<Vec<u8>, ApiError> {
    let to = match format {
        "docx" => Format::Docx,
        "odt" => Format::Odt,
        _ => return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "format трябва да е docx или odt")),
    };
    let bytes = ferrodoc::convert(html(blocks).as_bytes(), Format::Html, to)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if format == "odt" {
        return odt_align_totals(bytes, &totals_tables(blocks));
    }
    Ok(bytes)
}

/// PDF от таблица с редове. За справки от дневника.
pub fn grid_pdf(grid: &[Vec<String>]) -> Result<Vec<u8>, ApiError> {
    pdf(&[Block::Table(grid.to_vec())])
}

fn table(rows: &[Vec<String>]) -> String {
    let mut out = String::from("<table>");
    for (i, row) in rows.iter().enumerate() {
        out.push_str("<tr>");
        let tag = if i == 0 { "th" } else { "td" };
        for cell in row {
            out.push_str(&format!("<{tag}>{}</{tag}>", esc(cell)));
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}

fn totals(rows: &[Vec<String>]) -> String {
    let mut out = String::from("<table class=\"totals\">");
    for row in rows {
        out.push_str("<tr>");
        for (i, cell) in row.iter().enumerate() {
            if i + 1 == row.len() {
                out.push_str(&format!("<td align=\"right\">{}</td>", esc(cell)));
            } else {
                out.push_str(&format!("<td>{}</td>", esc(cell)));
            }
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}

fn esc(s: &str) -> String {
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

/// Поредни номера на тоталите сред таблиците в документа.
fn totals_tables(blocks: &[Block]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut n = 0;
    for block in blocks {
        match block {
            Block::Table(_) => n += 1,
            Block::Totals(_) => {
                out.push(n);
                n += 1;
            }
            _ => {}
        }
    }
    out
}

const RIGHT_STYLE: &str = "TotalsRight";

/// ferrodoc-odt не записва подравняване на клетките (widths and alignments
/// are dropped), затова то се добавя в content.xml след генерацията.
fn odt_align_totals(bytes: Vec<u8>, tables: &[usize]) -> Result<Vec<u8>, ApiError> {
    if tables.is_empty() {
        return Ok(bytes);
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(odt_err)?;
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(odt_err)?;
        let name = file.name().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(odt_err)?;
        entries.push((name, data));
    }
    for (name, data) in entries.iter_mut() {
        if name == "content.xml" {
            let xml = String::from_utf8_lossy(data).into_owned();
            *data = align_right_cells(&xml, tables).into_bytes();
        }
    }
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in &entries {
        let method = if name.as_str() == "mimetype" {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        };
        writer
            .start_file(name.as_str(), SimpleFileOptions::default().compression_method(method))
            .map_err(odt_err)?;
        writer.write_all(data).map_err(odt_err)?;
    }
    Ok(writer.finish().map_err(odt_err)?.into_inner())
}

fn odt_err(e: impl std::fmt::Display) -> ApiError {
    api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("odt: {e}"))
}

/// Последната клетка на всеки ред в избраните таблици става дясно подравнена.
fn align_right_cells(xml: &str, tables: &[usize]) -> String {
    let mut inserts: Vec<(usize, String)> = Vec::new();
    for (index, (start, end)) in table_spans(xml).into_iter().enumerate() {
        if !tables.contains(&index) {
            continue;
        }
        for (row_start, row_end) in spans_within(xml, start, end, "<table:table-row", "</table:table-row>") {
            if let Some(pos) = last_tag(xml, row_start, row_end, "<table:table-cell") {
                inserts.push((
                    pos + "<table:table-cell".len(),
                    format!(" table:style-name=\"{RIGHT_STYLE}\""),
                ));
            }
        }
    }
    if inserts.is_empty() {
        return xml.to_string();
    }
    let style = format!(
        "<style:style style:name=\"{RIGHT_STYLE}\" style:family=\"table-cell\">\
         <style:paragraph-properties fo:text-align=\"end\"/></style:style>"
    );
    if let Some(pos) = xml.find("<office:automatic-styles>") {
        inserts.push((pos + "<office:automatic-styles>".len(), style));
    }
    inserts.sort_by_key(|(pos, _)| *pos);
    let mut out = String::with_capacity(xml.len() + inserts.len() * 128);
    let mut last = 0;
    for (pos, text) in inserts {
        out.push_str(&xml[last..pos]);
        out.push_str(&text);
        last = pos;
    }
    out.push_str(&xml[last..]);
    out
}

/// Граници на таблици `table:table` в документа.
fn table_spans(xml: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(start) = find_tag(xml, pos, xml.len(), "<table:table") {
        let Some(close) = xml[start..].find("</table:table>") else {
            break;
        };
        let end = start + close + "</table:table>".len();
        out.push((start, end));
        pos = end;
    }
    out
}

/// Граници на `open...close` в диапазона [from, to).
fn spans_within(xml: &str, from: usize, to: usize, open: &str, close: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut pos = from;
    while let Some(start) = find_tag(xml, pos, to, open) {
        let Some(rel) = xml[start..to].find(close) else {
            break;
        };
        let end = start + rel;
        out.push((start, end));
        pos = end + close.len();
    }
    out
}

fn last_tag(xml: &str, from: usize, to: usize, tag: &str) -> Option<usize> {
    let mut pos = from;
    let mut last = None;
    while let Some(start) = find_tag(xml, pos, to, tag) {
        last = Some(start);
        pos = start + tag.len();
    }
    last
}

/// Позиция на отварящ таг; `tag` да не е префикс на по-дълъг елемент.
fn find_tag(xml: &str, from: usize, to: usize, tag: &str) -> Option<usize> {
    let mut pos = from;
    while let Some(rel) = xml[pos..to].find(tag) {
        let at = pos + rel;
        let after = at + tag.len();
        match xml.as_bytes().get(after) {
            Some(b' ') | Some(b'>') | Some(b'/') => return Some(at),
            _ => pos = after,
        }
    }
    None
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
                vec!["1".into(), "Консултация <с&д>".into()],
            ]),
        ]
    }

    #[test]
    fn html_is_html5_and_escapes() {
        let h = html(&sample());
        assert!(h.starts_with("<!doctype html>"));
        assert!(h.contains("ФАКТУРА"));
        assert!(h.contains("Консултация &lt;с&amp;д&gt;"));
        assert!(h.contains("<table>"));
    }

    #[test]
    fn totals_are_right_aligned() {
        let blocks = vec![Block::Totals(vec![
            vec!["Данъчна основа".into(), "250.00 EUR".into()],
            vec!["ОБЩО ЗА ПЛАЩАНЕ".into(), "300.00 EUR".into()],
        ])];
        let h = html(&blocks);
        assert!(h.contains("<table class=\"totals\">"));
        assert!(h.contains("<td align=\"right\">300.00 EUR</td>"));
    }

    #[test]
    fn pdf_is_pdf() {
        let pdf = pdf(&sample()).expect("pdf");
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.windows(5).any(|w| w == b"%%EOF"));
    }

    #[test]
    fn office_files_are_archives() {
        let docx = office(&sample(), "docx").expect("docx");
        let odt = office(&sample(), "odt").expect("odt");
        assert_eq!(&docx[..2], b"PK");
        assert_eq!(&odt[..2], b"PK");
        assert!(docx.windows(12).any(|w| w == b"word/document") || docx.windows(10).any(|w| w == b"[Content_T"));
        assert!(odt.windows(8).any(|w| w == b"mimetype"));
    }

    #[test]
    fn odt_totals_are_right_aligned() {
        let blocks = vec![
            Block::Table(vec![vec!["No".into(), "Стока".into()]]),
            Block::Totals(vec![
                vec!["Данъчна основа".into(), "250.00 EUR".into()],
                vec!["ОБЩО ЗА ПЛАЩАНЕ".into(), "300.00 EUR".into()],
            ]),
        ];
        let bytes = office(&blocks, "odt").expect("odt");
        let mut archive = ZipArchive::new(Cursor::new(&bytes)).expect("zip");
        let mut data = Vec::new();
        archive
            .by_name("content.xml")
            .expect("content.xml")
            .read_to_end(&mut data)
            .expect("read");
        let xml = String::from_utf8_lossy(&data);
        assert!(xml.contains("fo:text-align=\"end\""));
        assert!(xml.contains(&format!("table:style-name=\"{RIGHT_STYLE}\"")));
    }
}
