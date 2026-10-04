//! Основания за неначисляване на ДДС. Каталогът е статичен, като в bagabuch.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::form::auth_sub;
use crate::{AppState, ApiError};

const CATALOG: &[&str] = &[
    "VATEX-EU-SM|Освободена доставка - малко предприятие|Small business exemption|чл. 113 от ЗДДС|Art. 113 VAT Act|E|vatex-eu-e|Нерегистрирано по ЗДДС лице под прага",
    "VATEX-EU-G|Износ на стоки|Export of goods|чл. 28 от ЗДДС|Art. 28 VAT Act|G|vatex-eu-g|Доставка на стоки извън територията на ЕС",
    "VATEX-EU-IC|Вътреобщностна доставка на стоки|Intra-Community supply of goods|чл. 7, ал. 1 от ЗДДС|Art. 7(1) VAT Act|K|vatex-eu-ic|Доставка към регистриран по ДДС получател в друга държава-членка",
    "VATEX-EU-O|Обратно начисляване на ДДС|Reverse charge|чл. 82, ал. 2-5 от ЗДДС|Art. 82(2-5) VAT Act|O|vatex-eu-o|Данъкът се дължи от получателя",
    "VATEX-EU-AE|Специален режим на обратно начисляване|VAT Reverse Charge|чл. 163а от ЗДДС|Art. 163a VAT Act|AE|vatex-eu-ae|Обратно начисляване при инвестиционно злато и други",
    "VATEX-EU-E-39|Освободена доставка - здравеопазване|Exempt - healthcare|чл. 39 от ЗДДС|Art. 39 VAT Act|E|vatex-eu-e|Здравни услуги и свързани стоки",
    "VATEX-EU-E-40|Освободена доставка - социални услуги|Exempt - social services|чл. 40 от ЗДДС|Art. 40 VAT Act|E|vatex-eu-e|Социални услуги",
    "VATEX-EU-E-41|Освободена доставка - образование|Exempt - education|чл. 41 от ЗДДС|Art. 41 VAT Act|E|vatex-eu-e|Образователни услуги",
    "VATEX-EU-E-42|Освободена доставка - културни услуги|Exempt - cultural services|чл. 42 от ЗДДС|Art. 42 VAT Act|E|vatex-eu-e|Културни и спортни услуги",
    "VATEX-EU-E-44|Освободена доставка - финансови услуги|Exempt - financial services|чл. 44 от ЗДДС|Art. 44 VAT Act|E|vatex-eu-e|Финансови услуги",
    "VATEX-EU-E-45|Освободена доставка - застраховки|Exempt - insurance|чл. 45 от ЗДДС|Art. 45 VAT Act|E|vatex-eu-e|Застрахователни и презастрахователни услуги",
    "VATEX-EU-E-45-1|Земя (нерегулирана)|Land (non-regulated)|чл. 45, ал. 1 от ЗДДС|Art. 45(1) VAT Act|E|vatex-eu-e|Прехвърляне на право на собственост върху земя",
    "VATEX-EU-E-45-3|Сгради (които не са нови)|Buildings (not new)|чл. 45, ал. 3 от ЗДДС|Art. 45(3) VAT Act|E|vatex-eu-e|Доставка на сгради или части от тях, които не са нови",
    "VATEX-EU-E-45-4|Отдаване под наем за жилищни нужди|Rental for residential purposes|чл. 45, ал. 4 от ЗДДС|Art. 45(4) VAT Act|E|vatex-eu-e|Наем на сграда за жилищни нужди на физическо лице",
    "VATEX-EU-E-46|Освободена доставка - недвижими имоти|Exempt - immovable property|чл. 45-46 от ЗДДС|Art. 45-46 VAT Act|E|vatex-eu-e|Доставка на недвижим имот, наем",
    "VATEX-EU-E-47|Освободена доставка - хазартни услуги|Exempt - gambling|чл. 47 от ЗДДС|Art. 47 VAT Act|E|vatex-eu-e|Хазартни услуги",
    "VATEX-EU-Z-29|Нулева ставка - международен транспорт|Zero rate - international transport|чл. 29 от ЗДДС|Art. 29 VAT Act|Z|vatex-eu-z|Международен транспорт на пътници и стоки",
    "VATEX-EU-Z-30|Нулева ставка - услуги за плавателни съдове|Zero rate - maritime services|чл. 30 от ЗДДС|Art. 30 VAT Act|Z|vatex-eu-z|Доставки за плавателни съдове",
    "VATEX-EU-Z-31|Нулева ставка - услуги за въздухоплавателни средства|Zero rate - aircraft services|чл. 31 от ЗДДС|Art. 31 VAT Act|Z|vatex-eu-z|Доставки за въздухоплавателни средства",
    "VATEX-EU-Z-32|Нулева ставка - транзитни стоки|Zero rate - transit goods|чл. 32 от ЗДДС|Art. 32 VAT Act|Z|vatex-eu-z|Доставки в режим транзит",
    "VATEX-EU-Z-33|Нулева ставка - митнически режими|Zero rate - customs regimes|чл. 33 от ЗДДС|Art. 33 VAT Act|Z|vatex-eu-z|Доставки, свързани с митнически режими",
    "VATEX-EU-Z-173|Нулева ставка - дипломатически мисии|Zero rate - diplomatic missions|чл. 173 от ЗДДС|Art. 173 VAT Act|Z|vatex-eu-z|Доставки за дипломатически и консулски представителства",
    "VATEX-EU-S-21|Услуги към лица извън ЕС|Services to non-EU persons|чл. 21, ал. 2 от ЗДДС|Art. 21(2) VAT Act|O|vatex-eu-o|Място на изпълнение извън ЕС",
    "VATEX-EU-S-22|Услуги, свързани с недвижим имот извън БГ|Services related to immovable property outside BG|чл. 21, ал. 4, т. 1 от ЗДДС|Art. 21(4)(1) VAT Act|O|vatex-eu-o|Недвижим имот извън България",
];

fn ascii_lower(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c })
        .collect()
}

fn find(code: &str) -> Option<&'static str> {
    if code.is_empty() {
        return None;
    }
    let prefix = format!("{code}|");
    CATALOG.iter().copied().find(|line| line.starts_with(&prefix))
}

fn field(line: &str, idx: usize) -> &str {
    line.split('|').nth(idx).unwrap_or("")
}

pub fn print_label(code: &str) -> String {
    let Some(line) = find(code) else {
        return code.to_string();
    };
    let name = field(line, 1);
    let legal = field(line, 3);
    if name.is_empty() {
        return code.to_string();
    }
    if legal.is_empty() { name.to_string() } else { format!("{name} — {legal}") }
}

pub fn ubl_reason(code: &str) -> &'static str {
    find(code).map(|line| field(line, 6)).unwrap_or("")
}

pub fn legal_en(code: &str) -> String {
    let Some(line) = find(code) else {
        return String::new();
    };
    let en = field(line, 4);
    if en.is_empty() { field(line, 3).to_string() } else { en.to_string() }
}

/// Празно или непознато основание е `Z`, както в bagabuch.
pub fn category(code: &str) -> &'static str {
    let Some(line) = find(code) else {
        return "Z";
    };
    let c = field(line, 5);
    match c {
        "E" => "E",
        "G" => "G",
        "K" => "K",
        "O" => "O",
        "AE" => "AE",
        "Z" => "Z",
        "" => "Z",
        _ => "Z",
    }
}

fn item(line: &str) -> Value {
    json!({
        "code": field(line, 0),
        "name": field(line, 1),
        "name_en": field(line, 2),
        "legal_basis": field(line, 3),
        "legal_basis_en": field(line, 4),
        "ubl_category_code": field(line, 5),
        "ubl_exemption_code": field(line, 6),
        "description": field(line, 7),
    })
}

#[derive(Deserialize, Default)]
pub(crate) struct SearchQ {
    search: Option<String>,
}

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SearchQ>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let needle = ascii_lower(q.search.as_deref().unwrap_or("").trim());
    let items: Vec<Value> = CATALOG
        .iter()
        .copied()
        .filter(|line| needle.is_empty() || ascii_lower(line).contains(&needle))
        .map(item)
        .collect();
    let count = items.len();
    Ok(Json(json!({ "items": items, "count": count })))
}
