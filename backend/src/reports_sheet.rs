//! XLSX и ODS от таблица. ZIP е без компресия, за да мине кирилицата като UTF-8.

pub fn xlsx(grid: &[Vec<String>]) -> Vec<u8> {
    let sheet = worksheet(grid);
    pack(&[
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
        ("_rels/.rels", ROOT_RELS.as_bytes()),
        ("xl/workbook.xml", WORKBOOK.as_bytes()),
        ("xl/_rels/workbook.xml.rels", WORKBOOK_RELS.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ])
}

pub fn ods(grid: &[Vec<String>]) -> Vec<u8> {
    let content = ods_content(grid);
    pack(&[
        ("mimetype", b"application/vnd.oasis.opendocument.spreadsheet"),
        ("META-INF/manifest.xml", ODS_MANIFEST.as_bytes()),
        ("content.xml", content.as_bytes()),
    ])
}

fn worksheet(grid: &[Vec<String>]) -> String {
    let mut rows = String::new();
    for (r, row) in grid.iter().enumerate() {
        let n = r + 1;
        rows.push_str(&format!("<row r=\"{n}\">"));
        for (c, cell) in row.iter().enumerate() {
            let r#ref = format!("{}{n}", col_name(c));
            rows.push_str(&format!(
                "<c r=\"{ref}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{text}</t></is></c>",
                r#ref = r#ref,
                text = xml(cell),
            ));
        }
        rows.push_str("</row>");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
         <sheetData>{rows}</sheetData></worksheet>"
    )
}

fn ods_content(grid: &[Vec<String>]) -> String {
    let mut rows = String::new();
    for row in grid {
        rows.push_str("<table:table-row>");
        for cell in row {
            rows.push_str(&format!(
                "<table:table-cell office:value-type=\"string\"><text:p>{}</text:p></table:table-cell>",
                xml(cell)
            ));
        }
        rows.push_str("</table:table-row>");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-content \
         xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
         xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" \
         xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" office:version=\"1.2\">\
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
        let bytes = xlsx(&[vec!["Бага".into(), "1.20".into()]]);
        assert_eq!(&bytes[..2], b"PK");
    }
}
