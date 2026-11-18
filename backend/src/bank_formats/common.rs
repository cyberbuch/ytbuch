//! Общи помощни функции за всички банкови формати.

use crate::dec::{self, money};

pub fn norm_money(raw: &str) -> String {
    let t = raw.trim().replace(' ', "");
    if t.is_empty() { return String::new(); }
    let t = if t.contains(',') && t.contains('.') { t.replace(',', "") } else { t.replace(',', ".") };
    match dec::parse(&t) {
        Some(v) => money(v),
        None => String::new(),
    }
}

pub fn flex_date(raw: &str) -> String {
    let t = raw.trim();
    // Срезът до 10 байта е само при ASCII — иначе не е на граница на знак.
    if t.len() >= 10
        && t.as_bytes()[..10].iter().all(u8::is_ascii)
        && t.as_bytes().get(4) == Some(&b'-')
        && t.as_bytes().first().is_some_and(u8::is_ascii_digit)
    {
        return t[..10].to_string();
    }
    let sep = if t.contains('/') { '/' } else if t.contains('.') { '.' } else if t.contains('-') { '-' } else { return String::new() };
    let p: Vec<&str> = t.split(sep).collect();
    if p.len() != 3 { return String::new(); }
    let (y, m, d) = if p[0].len() == 4 { (p[0], p[1], p[2]) } else if p[2].len() == 4 { (p[2], p[1], p[0]) } else { return String::new() };
    if y.len() != 4 { return String::new(); }
    format!("{y}-{:0>2}-{:0>2}", m.trim(), d.trim().chars().take(2).collect::<String>())
}

pub fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

pub fn xml_tag(block: &str, name: &str) -> String {
    let close = format!("</{name}>");
    let Some(end) = block.find(&close) else {
        return String::new();
    };
    let head = &block[..end];
    let open = format!("<{name}");
    let Some(pos) = head.rfind(&open) else {
        return String::new();
    };
    let after = &head[pos + open.len()..];
    if after.starts_with(" nil=\"true\"") || after.starts_with('/') {
        return String::new();
    }
    let Some(gt) = after.find('>') else {
        return String::new();
    };
    xml_unescape(after[gt + 1..].trim())
}

pub fn xml_attr(block: &str, name: &str, attr: &str) -> String {
    let open = format!("<{name}");
    let Some(pos) = block.find(&open) else {
        return String::new();
    };
    let rest = &block[pos + open.len()..];
    let Some(gt) = rest.find('>') else {
        return String::new();
    };
    let tag_head = &rest[..gt];
    let needle = format!("{attr}=\"");
    let Some(apos) = tag_head.find(&needle) else {
        return String::new();
    };
    let val = &tag_head[apos + needle.len()..];
    val.find('"').map(|i| val[..i].to_string()).unwrap_or_default()
}

pub fn xml_blocks<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut out = Vec::new();
    let mut cur = xml;
    while let Some(s) = cur.find(&open) {
        let rest = &cur[s + open.len()..];
        let ok = rest.starts_with('>') || rest.starts_with(' ') || rest.starts_with('\t') || rest.starts_with('\r') || rest.starts_with('\n');
        if !ok {
            cur = rest;
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let body = &rest[gt + 1..];
        let Some(e) = body.find(&close) else { break };
        out.push(&body[..e]);
        cur = &body[e + close.len()..];
    }
    out
}

pub fn join_desc(parts: &[String]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" — ")
}

