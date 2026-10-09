//! XLSX и ODS от таблица. ZIP е без компрезия, за да мине кирилицата като UTF-8.
//! Числата са числови клетки; `bg` ги показва в български формат (1 200,00).

/// Стойност за числови клетки: нормализиран низ и брой десетични знаци.
struct Num {
    value: String,
    dp: usize,
    bg_text: String,
}

pub fn xlsx(grid: &[Vec<String>], bg: bool) -> Vec<u8> {
    let sheet = worksheet(grid);
    let styles = xlsx_styles(grid, bg);
    pack(&[
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
        ("_rels/.rels", ROOT_RELS.as_bytes()),
        ("xl/workbook.xml", WORKBOOK.as_bytes()),
        ("xl/_rels/workbook.xml.rels", WORKBOOK_RELS.as_bytes()),
        ("xl/styles.xml", styles.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ])
}

pub fn ods(grid: &[Vec<String>], bg: bool) -> Vec<u8> {
    let content = ods_content(grid, bg);
    pack(&[
        ("mimetype", b"application/vnd.oasis.opendocument.spreadsheet"),
        ("META-INF/manifest.xml", ODS_MANIFEST.as_bytes()),
        ("content.xml", content.as_bytes()),
    ])
}

fn worksheet(grid: &[Vec<String>]) -> String {
    let styles = style_index(grid);
    let mut rows = String::new();
    for (r, row) in grid.iter().enumerate() {
        let n = r + 1;
        rows.push_str(&format!("<row r=\"{n}\">"));
        for (c, cell) in row.iter().enumerate() {
            let r#ref = format!("{}{n}", col_name(c));
            match number(cell) {
                Some(num) => {
                    let s = styles.get(&num.dp).copied().unwrap_or(0);
                    rows.push_str(&format!(
                        "<c r=\"{ref}\" s=\"{s}\"><v>{value}</v></c>",
                        r#ref = r#ref,
                        value = num.value,
                    ));
                }
                None => rows.push_str(&format!(
                    "<c r=\"{ref}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{text}</t></is></c>",
                    r#ref = r#ref,
                    text = xml(cell),
                )),
            }
        }
        rows.push_str("</row>");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
         <sheetData>{rows}</sheetData></worksheet>"
    )
}

fn ods_content(grid: &[Vec<String>], bg: bool) -> String {
    let mut rows = String::new();
    for row in grid {
        rows.push_str("<table:table-row>");
        for cell in row {
            match number(cell) {
                Some(num) => {
                    let style = if num.dp == 4 { "Num4" } else { "Num2" };
                    let text = if bg { &num.bg_text } else { cell };
                    rows.push_str(&format!(
                        "<table:table-cell table:style-name=\"{style}\" office:value-type=\"float\" \
                         office:value=\"{value}\"><text:p>{text}</text:p></table:table-cell>",
                        value = num.value,
                        text = xml(text),
                    ));
                }
                None => rows.push_str(&format!(
                    "<table:table-cell office:value-type=\"string\"><text:p>{}</text:p></table:table-cell>",
                    xml(cell)
                )),
            }
        }
        rows.push_str("</table:table-row>");
    }
    let locale = if bg { " number:language=\"bg\" number:country=\"BG\"" } else { "" };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-content \
         xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
         xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
         xmlns:number=\"urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0\" \
         xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" \
         xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" office:version=\"1.2\">\
         <office:automatic-styles>\
         <number:number-style style:name=\"N2\"{locale}>\
         <number:number number:decimal-places=\"2\" number:min-decimal-places=\"2\" number:grouping=\"true\"/>\
         </number:number-style>\
         <number:number-style style:name=\"N4\"{locale}>\
         <number:number number:decimal-places=\"4\" number:min-decimal-places=\"4\" number:grouping=\"true\"/>\
         </number:number-style>\
         <style:style style:name=\"Num2\" style:family=\"table-cell\" style:data-style-name=\"N2\"/>\
         <style:style style:name=\"Num4\" style:family=\"table-cell\" style:data-style-name=\"N4\"/>\
         </office:automatic-styles>\
         <office:body><office:spreadsheet>\
         <table:table table:name=\"Spravka\">{rows}</table:table>\
         </office:spreadsheet></office:body></office:document-content>"
    )
}

fn col_name(mut index: usize) -> String {
    let mut out = String::new();
    loop {
        out.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    out
}

/// Разпознава суми и количества: цифри, по избор групирани с интервал,
/// и задължителна десетична част. Дати и номера остават текст.
fn number(s: &str) -> Option<Num> {
    let t = s.trim();
    let (neg, body) = t.strip_prefix('-').map(|b| (true, b)).unwrap_or((false, t));
    let (int_part, frac) = body.split_once('.')?;
    if frac.is_empty() || frac.len() > 6 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !int_part.chars().all(|c| c.is_ascii_digit() || c == ' ') {
        return None;
    }
    let int_digits: String = int_part.chars().filter(|c| c.is_ascii_digit()).collect();
    if int_digits.is_empty() {
        return None;
    }
    let sign = if neg { "-" } else { "" };
    let mut grouped = String::new();
    for (i, c) in int_digits.chars().enumerate() {
        if i > 0 && (int_digits.len() - i).is_multiple_of(3) {
            grouped.push(' ');
        }
        grouped.push(c);
    }
    Some(Num {
        value: format!("{sign}{int_digits}.{frac}"),
        dp: frac.len(),
        bg_text: format!("{sign}{grouped},{frac}"),
    })
}

/// Индекс на стила за всеки брой десетични знаци; 0 е подразбиращият се.
fn style_index(grid: &[Vec<String>]) -> std::collections::HashMap<usize, usize> {
    let mut out = std::collections::HashMap::new();
    for row in grid {
        for cell in row {
            if let Some(num) = number(cell) {
                if !out.contains_key(&num.dp) {
                    let s = out.len() + 1;
                    out.insert(num.dp, s);
                }
            }
        }
    }
    out
}

fn xlsx_styles(grid: &[Vec<String>], bg: bool) -> String {
    let index = style_index(grid);
    let mut entries: Vec<(&usize, &usize)> = index.iter().collect();
    entries.sort_by_key(|(_, style)| **style);
    let mut num_fmts = String::new();
    let mut xfs = String::new();
    let mut count = 0;
    for (dp, style) in entries {
        let id = 176 + style;
        let locale = if bg { "[$-402]" } else { "" };
        num_fmts.push_str(&format!(
            "<numFmt numFmtId=\"{id}\" formatCode=\"{locale}#,##0.{}\"/>",
            "0".repeat(*dp)
        ));
        xfs.push_str(&format!(
            "<xf numFmtId=\"{id}\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\"/>"
        ));
        count += 1;
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
         <numFmts count=\"{count}\">{num_fmts}</numFmts>\
         <fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>\
         <fills count=\"1\"><fill><patternFill patternType=\"none\"/></fill></fills>\
         <borders count=\"1\"><border/></borders>\
         <cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
         <cellXfs count=\"{total}\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>{xfs}</cellXfs>\
         </styleSheet>",
        total = count + 1
    )
}

fn xml(s: &str) -> String {
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

pub(crate) fn pack(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let name_b = name.as_bytes();
        out.extend_from_slice(&0x04034b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_b);
        out.extend_from_slice(data);

        central.extend_from_slice(&0x02014b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_b);
    }
    let central_at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x06054b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&central_at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
<Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>\
</Types>";

const ROOT_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
</Relationships>";

const WORKBOOK: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
<sheets><sheet name=\"Spravka\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>";

const WORKBOOK_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>\
</Relationships>";

const ODS_MANIFEST: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\">\
<manifest:file-entry manifest:full-path=\"/\" manifest:media-type=\"application/vnd.oasis.opendocument.spreadsheet\"/>\
<manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/>\
</manifest:manifest>";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_known() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn xlsx_is_zip() {
        let bytes = xlsx(&[vec!["YT".into(), "1.20".into()]], false);
        assert_eq!(&bytes[..2], b"PK");
    }

    #[test]
    fn numbers_are_numeric_cells() {
        assert!(number("1200.00").is_some());
        assert!(number("-250.5").is_some());
        assert!(number("1 200.00").is_some());
        // дати, номера и текст остават текст
        assert!(number("01.10.2026").is_none());
        assert!(number("411").is_none());
        assert!(number("0000000001").is_none());
        assert!(number("Данъчна основа").is_none());
    }

    #[test]
    fn bg_format_uses_comma_and_spaces() {
        let num = number("1234567.89").expect("number");
        assert_eq!(num.value, "1234567.89");
        assert_eq!(num.bg_text, "1 234 567,89");
        assert_eq!(number("-250.00").expect("neg").bg_text, "-250,00");
    }

    #[test]
    fn xlsx_and_ods_carry_numbers() {
        let grid = vec![vec!["Сума".into(), "1200.00".into()]];
        let xlsx_bg = xlsx(&grid, true);
        let sheet = String::from_utf8_lossy(&xlsx_bg);
        assert!(sheet.contains("<v>1200.00</v>"));
        let ods_bg = ods(&grid, true);
        let content = String::from_utf8_lossy(&ods_bg);
        assert!(content.contains("office:value-type=\"float\""));
        assert!(content.contains("office:value=\"1200.00\""));
        assert!(content.contains("1 200,00"));
        assert!(content.contains("number:language=\"bg\""));
    }
}
