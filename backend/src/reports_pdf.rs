//! PDF с вграден DejaVu, за да се чете кирилицата.
//! Кодовете в текста са номера на глифове (Identity-H).

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::api_err;
use crate::ApiError;
use axum::http::StatusCode;

struct Font {
    bytes: Vec<u8>,
    units: u16,
    advances: Vec<u16>,
    glyphs: HashMap<u32, u16>,
}

pub enum Flow {
    Head(String),
    Line(String),
    Table(Vec<Vec<String>>),
}

/// Портретен PDF от заглавия, редове и таблици. За фактура и протокол.
pub fn render_flow(parts: &[Flow]) -> Result<Vec<u8>, ApiError> {
    let font = font().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let page_w = 595.0;
    let page_h = 842.0;
    let margin = 36.0;
    let bottom = margin + 12.0;
    let usable = page_w - margin * 2.0;
    let mut pages: Vec<String> = Vec::new();
    let mut ops = String::new();
    let mut y = page_h - margin;
    for part in parts {
        match part {
            Flow::Head(text) => {
                paint_line(&mut ops, &mut pages, &mut y, font, text, margin, 13.0, 18.0, page_h, bottom, usable);
            }
            Flow::Line(text) if text.is_empty() => y -= 6.0,
            Flow::Line(text) => {
                paint_line(&mut ops, &mut pages, &mut y, font, text, margin, 9.0, 12.0, page_h, bottom, usable);
            }
            Flow::Table(rows) => {
                let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
                if cols == 0 {
                    continue;
                }
                let widths = flow_widths(cols, usable);
                for row in rows {
                    break_page(&mut ops, &mut pages, &mut y, 13.0, page_h, margin, bottom);
                    let mut x = margin;
                    for (i, cell) in row.iter().enumerate() {
                        let w = widths.get(i).copied().unwrap_or(40.0);
                        draw(&mut ops, font, cell, x, y, 8.0, w);
                        x += w;
                    }
                    y -= 13.0;
                }
                y -= 4.0;
            }
        }
    }
    if pages.is_empty() || !ops.is_empty() {
        pages.push(ops);
    }
    if pages.is_empty() {
        pages.push(String::new());
    }
    Ok(document(&pages, page_w, page_h, font))
}

fn paint_line(
    ops: &mut String,
    pages: &mut Vec<String>,
    y: &mut f32,
    font: &Font,
    text: &str,
    margin: f32,
    size: f32,
    line_h: f32,
    page_h: f32,
    bottom: f32,
    usable: f32,
) {
    break_page(ops, pages, y, line_h, page_h, margin, bottom);
    draw(ops, font, text, margin, *y, size, usable);
    *y -= line_h;
}

fn break_page(ops: &mut String, pages: &mut Vec<String>, y: &mut f32, need: f32, page_h: f32, margin: f32, bottom: f32) {
    if *y - need < bottom {
        pages.push(std::mem::take(ops));
        *y = page_h - margin;
    }
}

fn flow_widths(cols: usize, usable: f32) -> Vec<f32> {
    if cols <= 1 {
        return vec![usable];
    }
    if cols == 2 {
        let half = usable / 2.0;
        return vec![half, half];
    }
    if cols == 6 {
        let no = 28.0;
        let qty = 50.0;
        let unit = 46.0;
        let price = 72.0;
        let total = 78.0;
        let item = (usable - no - qty - unit - price - total).max(80.0);
        return vec![no, item, qty, unit, price, total];
    }
    vec![usable / cols as f32; cols]
}

pub fn render(grid: &[Vec<String>]) -> Result<Vec<u8>, ApiError> {
    let font = font().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let cols = grid.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let landscape = cols > 4;
    let (page_w, page_h) = if landscape { (842.0, 595.0) } else { (595.0, 842.0) };
    let margin = 28.0;
    let size = 8.0;
    let line_h = 11.0;
    let usable = page_w - margin * 2.0;
    let widths = col_widths(cols, usable);
    let mut pages: Vec<String> = Vec::new();
    let mut ops = String::new();
    let mut y = page_h - margin;
    for row in grid {
        if y < margin + line_h {
            pages.push(std::mem::take(&mut ops));
            y = page_h - margin;
        }
        let mut x = margin;
        for (i, cell) in row.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(40.0);
            draw(&mut ops, font, cell, x, y, size, w);
            x += w;
        }
        y -= line_h;
    }
    if pages.is_empty() || !ops.is_empty() {
        pages.push(ops);
    }
    if pages.is_empty() {
        pages.push(String::new());
    }
    Ok(document(&pages, page_w, page_h, font))
}

fn col_widths(cols: usize, usable: f32) -> Vec<f32> {
    if cols == 0 {
        return vec![usable];
    }
    if cols == 1 {
        return vec![usable];
    }
    let first = (usable * 0.34).min(220.0);
    let rest = (usable - first) / (cols - 1) as f32;
    let mut out = vec![first];
    out.resize(cols, rest);
    out
}

fn draw(ops: &mut String, font: &Font, text: &str, x: f32, y: f32, size: f32, max_w: f32) {
    let mut cursor = x;
    let limit = x + max_w - 2.0;
    for ch in text.chars() {
        if ch == '\n' || ch == '\r' {
            continue;
        }
        let gid = font.glyphs.get(&(ch as u32)).copied().unwrap_or(0);
        let adv = font.advances.get(gid as usize).copied().unwrap_or(font.advances.first().copied().unwrap_or(500));
        let w = adv as f32 * size / f32::from(font.units.max(1));
        if cursor + w > limit {
            break;
        }
        ops.push_str(&format!("BT /F1 {size:.2} Tf 1 0 0 1 {cursor:.2} {y:.2} Tm <{gid:04X}> Tj ET\n"));
        cursor += w;
    }
}

fn document(pages: &[String], w: f32, h: f32, font: &Font) -> Vec<u8> {
    let n = pages.len();
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let page_base = 3;
    let font_id = page_base + n * 2;
    let cid_id = font_id + 1;
    let desc_id = font_id + 2;
    let file_id = font_id + 3;
    let mut kids = String::new();
    for i in 0..n {
        if i > 0 {
            kids.push(' ');
        }
        kids.push_str(&format!("{} 0 R", page_base + i * 2));
    }
    objs.push(format!("<< /Type /Catalog /Pages 2 0 R >>").into_bytes());
    objs.push(format!("<< /Type /Pages /Count {n} /Kids [{kids}] >>").into_bytes());
    for (i, page) in pages.iter().enumerate() {
        let content_id = page_base + i * 2 + 1;
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] /Contents {content_id} 0 R /Resources << /Font << /F1 {font_id} 0 R >> >> >>"
            )
            .into_bytes(),
        );
        let stream = page.as_bytes();
        let mut body = format!("<< /Length {} >>\nstream\n", stream.len()).into_bytes();
        body.extend_from_slice(stream);
        body.extend_from_slice(b"endstream");
        objs.push(body);
    }
    objs.push(
        format!("<< /Type /Font /Subtype /Type0 /BaseFont /DejaVuSans /Encoding /Identity-H /DescendantFonts [{cid_id} 0 R] >>")
            .into_bytes(),
    );
    objs.push(
        format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /DejaVuSans /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {desc_id} 0 R /CIDToGIDMap /Identity /DW 500 >>"
        )
        .into_bytes(),
    );
    objs.push(
        format!(
            "<< /Type /FontDescriptor /FontName /DejaVuSans /Flags 32 /FontBBox [-1000 -300 2500 1200] /ItalicAngle 0 /Ascent 928 /Descent -236 /CapHeight 700 /StemV 80 /FontFile2 {file_id} 0 R >>"
        )
        .into_bytes(),
    );
    let mut file = format!("<< /Length {} /Length1 {} >>\nstream\n", font.bytes.len(), font.bytes.len()).into_bytes();
    file.extend_from_slice(&font.bytes);
    file.extend_from_slice(b"\nendstream");
    objs.push(file);

    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0u32; objs.len()];
    for (i, obj) in objs.iter().enumerate() {
        offsets[i] = out.len() as u32;
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(obj);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objs.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes(),
    );
    out
}

fn font() -> Result<&'static Font, String> {
    static FONT: OnceLock<Result<Font, String>> = OnceLock::new();
    match FONT.get_or_init(load_font) {
        Ok(font) => Ok(font),
        Err(e) => Err(e.clone()),
    }
}

fn load_font() -> Result<Font, String> {
    const PATHS: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    ];
    let mut last = "няма DejaVuSans.ttf за PDF".to_string();
    for path in PATHS {
        match std::fs::read(path) {
            Ok(bytes) => return parse_font(bytes),
            Err(e) => last = format!("{path}: {e}"),
        }
    }
    Err(last)
}

fn parse_font(bytes: Vec<u8>) -> Result<Font, String> {
    let tables = tables(&bytes)?;
    let head = tables.get("head").copied().ok_or("ttf: няма head")?;
    let hhea = tables.get("hhea").copied().ok_or("ttf: няма hhea")?;
    let hmtx = tables.get("hmtx").copied().ok_or("ttf: няма hmtx")?;
    let maxp = tables.get("maxp").copied().ok_or("ttf: няма maxp")?;
    let cmap = tables.get("cmap").copied().ok_or("ttf: няма cmap")?;
    let units = u16_at(&bytes, head.0 + 18).unwrap_or(2048).max(1);
    let glyphs_n = u16_at(&bytes, maxp.0 + 4).unwrap_or(1) as usize;
    let metrics = u16_at(&bytes, hhea.0 + 34).unwrap_or(1) as usize;
    let mut advances = vec![500u16; glyphs_n.max(1)];
    for i in 0..glyphs_n {
        let src = if i < metrics { i } else { metrics.saturating_sub(1) };
        if let Some(v) = u16_at(&bytes, hmtx.0 + src * 4) {
            advances[i] = v;
        }
    }
    let glyphs = cmap_map(&bytes, cmap.0)?;
    Ok(Font { bytes, units, advances, glyphs })
}

fn tables(bytes: &[u8]) -> Result<HashMap<String, (usize, usize)>, String> {
    let n = u16_at(bytes, 4).ok_or("ttf: къс файл")? as usize;
    let mut out = HashMap::new();
    for i in 0..n {
        let at = 12 + i * 16;
        let tag = bytes.get(at..at + 4).ok_or("ttf: къс файл")?;
        let off = u32_at(bytes, at + 8).ok_or("ttf")? as usize;
        let len = u32_at(bytes, at + 12).ok_or("ttf")? as usize;
        out.insert(String::from_utf8_lossy(tag).into_owned(), (off, len));
    }
    Ok(out)
}

fn cmap_map(bytes: &[u8], cmap: usize) -> Result<HashMap<u32, u16>, String> {
    let n = u16_at(bytes, cmap + 2).ok_or("cmap")? as usize;
    let mut best: Option<(u16, usize)> = None;
    for i in 0..n {
        let rec = cmap + 4 + i * 8;
        let off = u32_at(bytes, rec + 4).ok_or("cmap")? as usize;
        let at = cmap + off;
        let format = u16_at(bytes, at).unwrap_or(0);
        if format == 4 || format == 12 {
            let rank = if format == 4 { 0 } else { 1 };
            if best.map(|(r, _)| rank < r).unwrap_or(true) {
                best = Some((rank, at));
            }
        }
    }
    let (_, at) = best.ok_or("cmap: няма format 4/12")?;
    let format = u16_at(bytes, at).unwrap_or(0);
    if format == 4 {
        Ok(cmap4(bytes, at))
    } else {
        Ok(cmap12(bytes, at))
    }
}

fn cmap4(bytes: &[u8], at: usize) -> HashMap<u32, u16> {
    let seg_count = u16_at(bytes, at + 6).unwrap_or(0) as usize / 2;
    let ends = at + 14;
    let starts = ends + seg_count * 2 + 2;
    let deltas = starts + seg_count * 2;
    let offsets = deltas + seg_count * 2;
    let mut map = HashMap::new();
    for i in 0..seg_count {
        let end = u16_at(bytes, ends + i * 2).unwrap_or(0);
        let start = u16_at(bytes, starts + i * 2).unwrap_or(0);
        let delta = i16_at(bytes, deltas + i * 2).unwrap_or(0);
        let roff = u16_at(bytes, offsets + i * 2).unwrap_or(0);
        let mut code = start;
        while code <= end {
            let gid = if roff == 0 {
                (i32::from(code) + i32::from(delta)) as u16
            } else {
                let pos = offsets + i * 2 + roff as usize + usize::from(code - start) * 2;
                let raw = u16_at(bytes, pos).unwrap_or(0);
                if raw == 0 {
                    0
                } else {
                    (i32::from(raw) + i32::from(delta)) as u16
                }
            };
            if gid != 0 {
                map.insert(u32::from(code), gid);
            }
            if code == u16::MAX {
                break;
            }
            code += 1;
        }
    }
    map
}

fn cmap12(bytes: &[u8], at: usize) -> HashMap<u32, u16> {
    let groups = u32_at(bytes, at + 12).unwrap_or(0) as usize;
    let mut map = HashMap::new();
    for i in 0..groups {
        let g = at + 16 + i * 12;
        let start = u32_at(bytes, g).unwrap_or(0);
        let end = u32_at(bytes, g + 4).unwrap_or(0);
        let glyph = u32_at(bytes, g + 8).unwrap_or(0);
        let mut code = start;
        let mut gid = glyph;
        while code <= end && gid <= u32::from(u16::MAX) {
            if gid != 0 {
                map.insert(code, gid as u16);
            }
            if code == u32::MAX {
                break;
            }
            code += 1;
            gid += 1;
        }
    }
    map
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn i16_at(bytes: &[u8], at: usize) -> Option<i16> {
    Some(i16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_starts() {
        let bytes = render(&[vec!["Бага ООД".into(), "1200.00".into()]]).expect("pdf");
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.windows(5).any(|w| w == b"%%EOF"));
    }
}
