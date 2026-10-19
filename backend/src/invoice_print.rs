//! Печат на фактура: PDF, DOCX или ODT. Получател и доставчик се разменят при входяща.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde::Deserialize;

use crate::dec::{self, money};
use crate::print_load::{self, Party};
use crate::print_sheet::{self, Block};
use crate::print_words;
use crate::vat_ex;
use crate::{AppState, ApiError};

#[derive(Deserialize)]
pub struct Q {
    format: Option<String>,
    copy: Option<String>,
}

pub async fn print(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Query(q): Query<Q>,
) -> Result<Response, ApiError> {
    let format = q.format.as_deref().unwrap_or("").trim();
    let format = if format.is_empty() { "pdf" } else { format };
    if !matches!(format, "pdf" | "docx" | "odt") {
        return Err(crate::api_err(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "format трябва да е pdf/docx/odt",
        ));
    }
    let loaded = print_load::load(&st, &headers, id).await?;
    let copy = q.copy.as_deref() == Some("1");
    let blocks = blocks(&loaded, copy);
    let bytes = print_sheet::render(&blocks, format)?;
    let name = print_sheet::download_name(&loaded.inv.number, &loaded.inv.document_type, format);
    Ok(print_sheet::attachment(bytes, print_sheet::mime(format), &name))
}

fn blocks(doc: &print_load::Loaded, copy: bool) -> Vec<Block> {
    let inv = &doc.inv;
    let outgoing = inv.direction != "in";
    let company = print_load::party(&doc.company);
    let mut counterpart = print_load::party(&doc.counterpart);
    if counterpart.name.is_empty() {
        counterpart.name = inv.counterpart_name.clone();
    }
    let recipient = if outgoing { counterpart.clone() } else { company.clone() };
    let supplier = if outgoing { company.clone() } else { counterpart };
    let currency = if inv.currency.trim().is_empty() { "EUR" } else { inv.currency.trim() };
    let number = format!("{}{}", print_sheet::doc_prefix(&inv.document_type), inv.number);
    let mut out = Vec::new();
    if inv.status == "cancelled" {
        out.push(Block::Line("АНУЛИРАНА".into()));
    }
    let head_title = if inv.document_type == "09" && !inv.vat_article.trim().is_empty() {
        format!(
            "ПРОТОКОЛ ПО {} ОТ ЗДДС",
            inv.vat_article.trim().to_uppercase()
        )
    } else {
        print_sheet::doc_title(&inv.document_type).to_string()
    };
    out.push(Block::Head(head_title));
    let label = if copy { "КОПИЕ" } else { "ОРИГИНАЛ" };
    out.push(Block::Line(format!("( {label} )")));
    out.push(Block::Line(format!("№ {number}")));
    if !doc.original_number.trim().is_empty() {
        out.push(Block::Line(format!(
            "Към документ № {} / {}",
            doc.original_number,
            print_sheet::bg_date(&doc.original_date)
        )));
    }
    if !inv.issue_date.trim().is_empty() {
        out.push(Block::Line(format!("Дата: {}", print_sheet::bg_date(&inv.issue_date))));
    }
    if !inv.tax_event_date.trim().is_empty() {
        out.push(Block::Line(format!("Дан. дата: {}", print_sheet::bg_date(&inv.tax_event_date))));
    }
    out.push(Block::Table(parties(&recipient, &supplier)));
    out.push(Block::Table(lines(doc)));
    out.push(Block::Totals(totals(inv, currency)));
    if !company.iban.is_empty() {
        let mut bank = format!("IBAN: {}", company.iban);
        if !company.bic.is_empty() {
            bank.push_str(" · BIC: ");
            bank.push_str(&company.bic);
        }
        out.push(Block::Line(format!("БАНКОВА СМЕТКА · {bank}")));
    }
    if !inv.payment_method.trim().is_empty() {
        out.push(Block::Line(format!("Начин на плащане: {}", inv.payment_method.trim())));
    }
    if !inv.due_date.trim().is_empty() {
        out.push(Block::Line(format!("Срок за плащане: {}", inv.due_date.trim())));
    }
    if !inv.notes.trim().is_empty() {
        out.push(Block::Line(format!("Забележка: {}", inv.notes.trim())));
    }
    if !inv.vat_exemption_reason.trim().is_empty() {
        out.push(Block::Line(format!(
            "Основание за неначисляване на ДДС: {}",
            vat_ex::print_label(inv.vat_exemption_reason.trim())
        )));
    }
    let total = money(dec::or0(&inv.total_amount));
    out.push(Block::Line(format!("Словом: {}", print_words::words(&total, currency))));
    out
}

fn parties(rec: &Party, sup: &Party) -> Vec<Vec<String>> {
    let mut rows = vec![
        vec!["ПОЛУЧАТЕЛ".into(), "ДОСТАВЧИК".into()],
        vec![rec.name.clone(), sup.name.clone()],
        vec![place(&rec.address, &rec.city), place(&sup.address, &sup.city)],
        vec![format!("ЕИК: {}", rec.eik), format!("ЕИК: {}", sup.eik)],
    ];
    let mut sup_vat = format!("ДДС №: {}", sup.vat);
    if !sup.mol.is_empty() {
        sup_vat.push_str(" · МОЛ: ");
        sup_vat.push_str(&sup.mol);
    }
    rows.push(vec![format!("ДДС №: {}", rec.vat), sup_vat]);
    if !rec.contact.is_empty() {
        rows.push(vec![format!("Лице за контакт: {}", rec.contact), String::new()]);
    }
    rows
}

fn unit_display(code: &str) -> String {
    match code.trim() {
        "" | "C62" | "EA" | "PCE" | "H87" => "бр.".into(),
        "SET" => "к-т".into(),
        "PR" => "чифт".into(),
        "DZN" => "дуз.".into(),
        "GRO" => "грос".into(),
        "MIL" => "хил.".into(),
        "CEN" => "стотици".into(),
        "KGM" => "кг".into(),
        "GRM" => "г".into(),
        "MGM" => "мг".into(),
        "TNE" => "т".into(),
        "DTN" => "дт".into(),
        "LBR" => "lb".into(),
        "ONZ" => "oz".into(),
        "CTM" => "ct".into(),
        "MTR" => "м".into(),
        "CMT" => "см".into(),
        "MMT" => "мм".into(),
        "KMT" => "км".into(),
        "DMT" => "дм".into(),
        "INH" => "in".into(),
        "FOT" => "ft".into(),
        "YRD" => "yd".into(),
        "NMI" => "nmi".into(),
        "MTK" => "м²".into(),
        "CMK" => "см²".into(),
        "MMK" => "мм²".into(),
        "LTR" | "LT" => "л".into(),
        "MLT" => "мл".into(),
        "HLT" => "хл".into(),
        "HUR" => "час".into(),
        "DAY" => "ден".into(),
        "MON" => "месец".into(),
        "WEE" => "седм.".into(),
        "PK" => "пакет".into(),
        "E48" | "NAR" => "бр.".into(),
        other => other.to_string(),
    }
}

fn lines(doc: &print_load::Loaded) -> Vec<Vec<String>> {
    let mut rows = vec![vec![
        "No".into(),
        "Стока / услуга".into(),
        "К-во".into(),
        "Мярка".into(),
        "Ед. цена".into(),
        "Общо".into(),
    ]];
    for (i, line) in doc.lines.iter().enumerate() {
        rows.push(vec![
            (i + 1).to_string(),
            line.description.clone(),
            line.quantity.clone(),
            unit_display(&line.unit),
            money(dec::or0(&line.unit_price)),
            money(dec::or0(&line.net_amount)),
        ]);
    }
    rows
}

fn totals(inv: &crate::invoice_read::Inv, currency: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    if inv.document_type == "07" && !inv.customs_statistical.trim().is_empty() {
        rows.push(vec![
            "Статистическа стойност".into(),
            format!("{} {currency}", money(dec::or0(&inv.customs_statistical))),
        ]);
        rows.push(vec![
            "Мито".into(),
            format!("{} {currency}", money(dec::or0(&inv.customs_duty))),
        ]);
    }
    rows.push(vec![
        "Данъчна основа".into(),
        format!("{} {currency}", money(dec::or0(&inv.net_amount))),
    ]);
    rows.push(vec![
        "ДДС".into(),
        format!("{} {currency}", money(dec::or0(&inv.vat_amount))),
    ]);
    if dec::or0(&inv.discount_amount) > 0 {
        rows.push(vec![
            "Отстъпка".into(),
            format!("{} {currency}", money(dec::or0(&inv.discount_amount))),
        ]);
    }
    rows.push(vec![
        "ОБЩО ЗА ПЛАЩАНЕ".into(),
        format!("{} {currency}", money(dec::or0(&inv.total_amount))),
    ]);
    rows
}

fn place(address: &str, city: &str) -> String {
    match (address.is_empty(), city.is_empty()) {
        (true, true) => String::new(),
        (false, true) => address.to_string(),
        (true, false) => city.to_string(),
        (false, false) => format!("{address} {city}"),
    }
}
