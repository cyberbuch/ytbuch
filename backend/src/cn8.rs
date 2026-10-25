//! Комбинирана номенклатура КН8 (NC8_TARIC от SAF-T BG V1.0.1).
//! Търсене по код, описание, митнически синоними и AI класификация (Mistral + локален каталог).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{cell_str, Db};
use crate::form::auth_sub;
use crate::{api_err, ApiError, AppState};

const CN8_JSON: &str = include_str!("../data/cn8.json");
const MISTRAL_URL: &str = "https://api.mistral.ai/v1/chat/completions";
const MISTRAL_MODEL: &str = "mistral-small-latest";

#[derive(Deserialize)]
struct RawCn8 {
    chapters: HashMap<String, String>,
    headings: HashMap<String, (String, String)>,
    items: Vec<String>,
}

#[derive(Clone)]
pub struct Cn8Entry {
    pub code: String,
    pub name: String,
    pub full_name: String,
    pub heading: String,
    pub heading_name: String,
    pub chapter: String,
    pub chapter_name: String,
    haystack: String,
}

pub struct Cn8Catalog {
    pub entries: Vec<Cn8Entry>,
    pub by_code: HashMap<String, usize>,
}

/// Често срещани търговски думи -> митнически термини и 4/8-цифрени позиции по КН.
const SYNONYMS: &[(&[&str], &str)] = &[
    (&["лаптоп", "ноутбук", "laptop", "notebook", "преносим компютър", "macbook", "thinkpad"], "84713000 8471 автоматични машини за обработка на информация портативни"),
    (&["компютър", "сървър", "десктоп", "работна станция", "pc", "server"], "84714100 84714900 84715000 8471 автоматични машини за обработка на информация системи"),
    (&["клавиатура", "keyboard"], "84716060 8471 клавиатури"),
    (&["мишка", "скенер"], "84716070 8471 входни или изходни единици"),
    (&["диск", "ssd", "hdd", "хард", "памет", "флашка"], "84717050 85235110 8471 запаметяващи единици твърди дискови"),
    (&["монитор", "дисплей", "екран", "monitor"], "85285210 8528 монитори"),
    (&["телефон", "смартфон", "iphone", "samsung", "gsm", "мобилен"], "85171300 85171400 8517 телефонни апарати смартфони"),
    (&["рутер", "суич", "модем", "мрежов", "router", "switch"], "85176200 8517 апарати за приемане преобразуване и предаване"),
    (&["принтер", "копир", "мултифункционал", "printer"], "84433100 84433210 8443 машини за печат"),
    (&["тонер", "касета", "мастило"], "84439990 3215 мастила части"),
    (&["кабел", "проводник", "захранващ кабел", "hdmi", "usb", "lan"], "85444290 85444995 8544 изолирани проводници кабели"),
    (&["батерия", "акумулатор", "ups"], "85076000 8506 първични елементи акумулатори литиевойонни"),
    (&["стол", "кресло", "диван", "табуретка", "пейка"], "94013100 94013900 94016100 94017100 94018000 9401 мебели за сядане"),
    (&["бюро", "офис маса"], "94033011 94031051 9403 бюра"),
    (&["шкаф", "етажерка", "стелаж", "гардероб", "комод", "витрина"], "94033091 94036010 94036090 94031091 9403 шкафове мебели"),
    (&["маса", "трапезна маса", "холна маса", "мебел"], "94036010 94036090 94032080 9403 други мебели от дърво метал"),
    (&["легло", "спалня", "матрак"], "94035000 94032020 94042110 9403 легла мебели спални матраци"),
    (&["лампа", "осветително", "полилей", "прожектор", "крушка", "led"], "94051100 94054239 85395200 9405 осветителни тела светодиодни"),
    (&["хартия", "копирна хартия", "а4", "картон"], "48025620 48025680 4802 хартия и картон"),
    (&["кашон", "кутия", "опаковка", "плик", "чувал", "торбичка", "фолио"], "48191000 48192000 39232100 39201025 4819 кутии кашони торбички амбалаж"),
    (&["тетрадка", "папка", "класьор", "блокнот", "етикет"], "48201030 48203000 48211010 4820 канцеларски артикули класьори"),
    (&["химикал", "писалка", "маркер", "молив"], "96081092 96082000 96091090 9608 химикалки маркери"),
    (&["кафе", "еспресо", "арабика"], "09012100 0901 печено кафе"),
    (&["чай", "билков"], "09023000 21069092 0902 чай"),
    (&["вода", "минерална вода", "изворна"], "22011019 22021000 2201 води минерални газирани"),
    (&["бира", "пиво"], "22030001 2203 бира"),
    (&["вино", "ракия", "уиски", "водка", "алкохол"], "22042198 22082029 22083082 2204 вина спиртни напитки"),
    (&["захар", "сол", "брашно", "ориз", "олио", "зехтин", "масло"], "17019910 25010091 11010015 10063098 15121990 15092000 04051011"),
    (&["хляб", "закуска", "бисквити", "вафла", "шоколад"], "19059030 19053199 18063290 1905 хлебни изделия"),
    (&["мляко", "сирене", "кашкавал", "йогурт"], "04012091 04069086 04032093 0406 сирена и извара"),
    (&["месо", "пилешко", "свинско", "телешко", "колбас", "салам"], "02031955 02071290 02013000 16010099 месо колбаси"),
    (&["бензин", "дизел", "газьол", "гориво", "нафта", "пропан", "газ", "моторно масло"], "27101245 27101943 27101981 27111297 2710 нефтени масла газьоли бензин"),
    (&["електроенергия", "ток", "енергия"], "27160000 2716 електрическа енергия"),
    (&["цимент", "бетон", "хоросан", "лепило за плочки", "мазилка", "шпакловка", "гипс"], "25232900 38245090 32149000 25202000 2523 портландцимент строителни разтвори"),
    (&["тухла", "керемида", "плочки", "фаянс", "теракот", "гранитогрес"], "69041000 69072100 6907 керамични плочки строителни тухли"),
    (&["боя", "лак", "грунд", "разредител", "силикон", "пяна"], "32091000 32089091 32141010 3209 бои и лакове"),
    (&["дъска", "греди", "дървен материал", "шперплат", "пдч", "мдф", "osb", "палет"], "44071190 44101110 44111490 44123300 44152020 4407 дървен материал плочи палети"),
    (&["тръба", "фитинг", "маркуч", "кран", "вентил"], "39172390 73063077 84818099 3917 тръби и маркучи"),
    (&["профил", "ламарина", "арматура", "желязо", "стомана", "алуминий", "винт", "болт", "гайка", "пирон"], "72142000 72104900 73089098 76042990 73181595 7308 метални конструкции винтове болтове"),
    (&["стъкло", "дограма", "врата", "прозорец"], "70052980 39252000 44182190 76101000 врати прозорци и техните рамки"),
    (&["бормашина", "ъглошлайф", "винтоверт", "трион", "чук", "отвертка", "гаечен ключ", "клещи", "инструмент"], "84672199 84672985 82055980 82041100 8467 ръчни инструменти електромеханични"),
    (&["климатик", "вентилатор", "термопомпа"], "84151090 84145995 84186100 8415 машини и апарати за кондициониране на въздуха"),
    (&["хладилник", "фризер", "пералня", "съдомиялна", "печка", "фурна", "бойлер", "кафемашина", "прахосмукачка"], "84181020 84501111 84221100 85166010 85161080 85167100 85081100"),
    (&["автомобил", "кола", "камион", "бус", "влекач", "ремарке", "мотокар"], "87032319 87042191 87163950 84271010 8703 пътнически и товарни автомобили"),
    (&["гума", "автомобилна гума", "резервна част", "филтър", "накладки", "чистачки"], "40111000 99908700 87089997 84212300 4011 нови пневматични гуми резервни части"),
    (&["тениска", "риза", "панталон", "яке", "гащеризон", "работно облекло", "дрехи", "ръкавици"], "61091000 62052000 62034211 62014000 61161020 облекло"),
    (&["обувки", "ботуши", "маратонки", "работни обувки"], "64034000 64039996 64041990 6403 обувки"),
    (&["препарат", "сапун", "веро", "дезинфектант", "тоалетна хартия", "салфетки", "кърпи"], "34025090 34011100 38089490 48181010 48182091 миещи и почистващи препарати"),
    (&["лекарство", "аптечка", "медикамент", "витамин", "маска", "бинт"], "30049000 30065000 30059099 медикаменти"),
    (&["услуга", "консултация", "наем", "абонамент", "поддръжка", "ремонт", "софтуер", "лиценз", "хостинг", "транспорт", "доставка", "куриер", "реклама", "маркетинг", "обучение", "одит", "счетовод", "комисионна", "такса", "застраховка"], "00000000 услуги"),
];

pub fn catalog() -> &'static Cn8Catalog {
    static CAT: OnceLock<Cn8Catalog> = OnceLock::new();
    CAT.get_or_init(|| {
        let raw: RawCn8 = serde_json::from_str(CN8_JSON).expect("cn8.json");
        let mut entries = Vec::with_capacity(raw.items.len());
        let mut by_code = HashMap::with_capacity(raw.items.len());
        let mut stack: Vec<(u8, String)> = Vec::new();
        let mut prev_hd = String::new();
        for line in raw.items {
            let mut p = line.splitn(4, '|');
            let code = p.next().unwrap_or("").trim().to_string();
            let hd = p.next().unwrap_or("").trim().to_string();
            let lvl: u8 = p.next().unwrap_or("5").trim().parse().unwrap_or(5);
            let desc = p.next().unwrap_or("").trim().to_string();
            if code.is_empty() {
                continue;
            }
            if hd != prev_hd {
                stack.clear();
                prev_hd = hd.clone();
            }
            while stack.last().map(|(l, _)| *l >= lvl).unwrap_or(false) {
                stack.pop();
            }
            let clean = desc.trim_start_matches('-').trim().to_string();
            let (ch, hd_name) = raw
                .headings
                .get(&hd)
                .cloned()
                .unwrap_or_else(|| (String::new(), String::new()));
            let ch_name = raw.chapters.get(&ch).cloned().unwrap_or_default();
            let full_name = build_full_name(&clean, &hd_name, &stack);
            stack.push((lvl, clean.clone()));
            let haystack = format!(
                "{} {} {} {} {} {}",
                code,
                clean.to_lowercase(),
                full_name.to_lowercase(),
                hd,
                hd_name.to_lowercase(),
                ch_name.to_lowercase()
            );
            let idx = entries.len();
            by_code.insert(code.clone(), idx);
            entries.push(Cn8Entry {
                code,
                name: desc,
                full_name,
                heading: hd,
                heading_name: hd_name,
                chapter: ch,
                chapter_name: ch_name,
                haystack,
            });
        }
        Cn8Catalog { entries, by_code }
    })
}

fn build_full_name(clean: &str, hd_name: &str, stack: &[(u8, String)]) -> String {
    let low = clean.to_lowercase();
    let generic = low == "други"
        || low.starts_with("други,")
        || low.starts_with("от ")
        || low.starts_with("със ")
        || low.starts_with("с ")
        || low.starts_with("без ")
        || low.starts_with("за ")
        || clean.chars().count() < 18;
    if !generic || hd_name.is_empty() {
        return clean.to_string();
    }
    let short_hd = hd_name.split(';').next().unwrap_or(hd_name).trim();
    if let Some((_, parent)) = stack.last() {
        if !parent.is_empty() && parent != clean {
            return format!("{short_hd} — {parent} — {clean}");
        }
    }
    format!("{short_hd} — {clean}")
}

pub fn name_of(code: &str) -> Option<String> {
    let cat = catalog();
    let idx = cat.by_code.get(code.trim())?;
    Some(cat.entries[*idx].full_name.clone())
}

fn entry_json(e: &Cn8Entry) -> Value {
    json!({
        "code": e.code,
        "name": e.full_name,
        "raw_name": e.name,
        "heading": e.heading,
        "heading_name": e.heading_name,
        "chapter": e.chapter,
        "chapter_name": e.chapter_name,
    })
}

fn stem(w: &str) -> String {
    let chars: Vec<char> = w.chars().collect();
    let n = chars.len();
    if n <= 4 {
        return w.to_string();
    }
    for suf in ["ите", "ове", "ата", "ята", "ия", "ен", "на", "но", "ни", "та", "те", "и", "а", "о", "е", "я"] {
        let sc = suf.chars().count();
        if n > sc + 3 && w.ends_with(suf) {
            return chars[..n - sc].iter().collect();
        }
    }
    w.to_string()
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .map(|w| w.to_string())
        .collect()
}

fn expand_query(raw: &str) -> (Vec<String>, Vec<String>) {
    let low = raw.to_lowercase();
    let mut words = tokenize(&low);
    let mut boosted_codes = Vec::new();
    for (triggers, expansion) in SYNONYMS {
        if triggers.iter().any(|t| low.contains(t)) {
            for tok in tokenize(expansion) {
                if tok.chars().all(|c| c.is_ascii_digit()) && (tok.len() == 4 || tok.len() == 8) {
                    boosted_codes.push(tok);
                } else {
                    words.push(tok);
                }
            }
        }
    }
    let mut stems = Vec::new();
    for w in &words {
        let s = stem(w);
        if !stems.contains(&s) {
            stems.push(s);
        }
    }
    (stems, boosted_codes)
}

pub fn rank_entries(query: &str, is_service: bool, limit: usize) -> (usize, Vec<&'static Cn8Entry>) {
    let cat = catalog();
    let q = query.trim();
    if is_service && q.is_empty() {
        if let Some(&idx) = cat.by_code.get("00000000") {
            return (1, vec![&cat.entries[idx]]);
        }
    }
    if q.is_empty() {
        let take = limit.min(cat.entries.len());
        return (cat.entries.len(), cat.entries.iter().take(take).collect());
    }
    let q_low = q.to_lowercase();
    let (stems, boosted) = expand_query(q);
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (idx, e) in cat.entries.iter().enumerate() {
        let mut score = 0i32;
        if e.code == q_low {
            score += 2000;
        } else if e.code.starts_with(&q_low) || e.heading == q_low {
            score += 900;
        }
        for b in &boosted {
            if &e.code == b {
                score += 650;
            } else if &e.heading == b || e.code.starts_with(b.as_str()) {
                score += 320;
            }
        }
        if e.haystack.contains(&q_low) {
            score += 280;
        }
        let fn_low = e.full_name.to_lowercase();
        for s in &stems {
            if s.chars().count() < 2 {
                continue;
            }
            if fn_low.contains(s.as_str()) {
                score += 95;
            } else if e.haystack.contains(s.as_str()) {
                score += 45;
            }
        }
        if is_service && e.code == "00000000" {
            score += 1200;
        }
        if score > 0 {
            if e.name.trim_start_matches('-').trim().eq_ignore_ascii_case("други") {
                score -= 8;
            }
            scored.push((score, idx));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let total = scored.len();
    let out = scored
        .into_iter()
        .take(limit)
        .map(|(_, idx)| &cat.entries[idx])
        .collect();
    (total, out)
}

pub fn search_noms(query: &str, limit: usize, offset: usize) -> (usize, Vec<Value>) {
    let (total, entries) = rank_entries(query, false, offset.saturating_add(limit));
    let items = entries.into_iter().skip(offset).take(limit).map(entry_json).collect();
    (total, items)
}

/// Предлага мерна единица от SAF-T Unit of Measure според текста на артикула.
pub fn guess_saft_unit(text: &str, is_service: bool, current: &str) -> String {
    let low = format!(" {} ", text.to_lowercase());
    for (needles, code) in [
        (&[" кг", " kg", "килограм"][..], "KGM"),
        (&[" гр.", " грам", " g "], "GRM"),
        (&[" тон", " т.", " tne"], "TNE"),
        (&[" л.", " литър", " литра", " l ", " ltr", " мл", " ml"], "LTR"),
        (&[" кв.м", " м2", " m2", "квадратен метър"], "MTK"),
        (&[" куб.м", " м3", " m3", "кубичен метър"], "MTQ"),
        (&[" метър", " метра", " л.м", " м."], "MTR"),
        (&[" чифт", " двойка"], "PR"),
        (&[" комплект", " к-т", " набор", " пакет"], "SET"),
        (&[" кутия", " кашон"], "SET"),
        (&[" квтч", " kwh", " mwh", "електроенергия"], "KWH"),
        (&[" час", " часа", " ч."], "HUR"),
        (&[" месец", " месечен", " абонамент"], "MON"),
        (&[" ден", " дни", " нощувк"], "DAY"),
    ] {
        if needles.iter().any(|n| low.contains(n)) {
            return code.to_string();
        }
    }
    if is_service {
        return if current.trim().is_empty() || current == "H87" {
            "C62".into()
        } else {
            current.trim().to_string()
        };
    }
    if !current.trim().is_empty() && current != "C62" {
        return current.trim().to_string();
    }
    "H87".into()
}

/// Предлага вид материален запас по SAF-T Nom_Inventory_Types (10, 20, 30, 40, 50).
pub fn guess_product_type(text: &str, is_service: bool, current: &str) -> String {
    if is_service {
        return String::new();
    }
    if matches!(current.trim(), "10" | "20" | "30" | "40" | "50") {
        return current.trim().to_string();
    }
    let low = text.to_lowercase();
    if [
        "суровин", "материал", "цимент", "бетон", "боя", "лак", "лепило", "дъск", "профил",
        "ламарин", "тръб", "кабел", "винт", "болт", "хартия", "тонер", "гориво", "дизел",
        "бензин", "резервна част", "консуматив", "опаковк", "кашон", "фолио", "препарат",
    ]
    .iter()
    .any(|w| low.contains(w))
    {
        return "10".into();
    }
    if low.contains("собствено производство") || low.contains("готова продукция") {
        return "20".into();
    }
    if low.contains("незавършено") {
        return "40".into();
    }
    "30".into()
}

fn guess_is_service(text: &str, flag: bool) -> bool {
    if flag {
        return true;
    }
    let low = text.to_lowercase();
    [
        "услуг", "консултац", "абонамент", "наем", "поддръжк", "ремонт", "транспорт",
        "доставк", "куриер", "хостинг", "лиценз", "одит", "счетовод", "обучение", "комисион",
    ]
    .iter()
    .any(|w| low.contains(w))
}

async fn mistral_key(db: &Db) -> Option<String> {
    let rows = db
        .query("SELECT mistral_key FROM system_settings WHERE id = 1")
        .await
        .ok()?;
    let key = rows.first().map(|row| cell_str(row, 0)).unwrap_or_default().trim().to_string();
    if key.is_empty() { None } else { Some(key) }
}

#[derive(Deserialize)]
struct AiReply {
    cn8_code: Option<String>,
    hs4: Option<String>,
    keywords: Option<Vec<String>>,
    unit: Option<String>,
    product_type: Option<String>,
    is_service: Option<bool>,
}

async fn ask_mistral(
    key: &str,
    name: &str,
    desc: &str,
    candidates: &[&Cn8Entry],
) -> Option<AiReply> {
    let mut cand_lines = String::new();
    for c in candidates.iter().take(35) {
        cand_lines.push_str(&format!("{} | {}\n", c.code, c.full_name));
    }
    let prompt = format!(
        "Ти си експерт по Комбинираната митническа номенклатура на ЕС и България (КН8 / NC8_TARIC) и SAF-T BG.\n\
         За подадения артикул избери най-подходящия 8-цифрен код по КН8, 4-цифрената позиция по ХС/КН (hs4), \
         ключови думи на български от митническата номенклатура (keywords), подходяща SAF-T мерна единица (unit: H87 за брой/парче, C62 за единица, KGM за кг, GRM за грам, TNE за тон, LTR за литър, MTR за метър, MTK за м2, MTQ за м3, PR за чифт, SET за комплект/пакет, KWH за електроенергия, HUR за час, MON за месец) \
         и вид материален запас по SAF-T (product_type: \"10\" Материали, \"20\" Продукция, \"30\" Стоки, \"40\" Незавършено производство, \"50\" Инвестиция в материален запас, или \"\" за услуга).\n\
         За услуги кодът по КН8 винаги е \"00000000\".\n\
         Върни САМО валиден JSON обект: {{\"cn8_code\":\"...\",\"hs4\":\"...\",\"keywords\":[\"...\"],\"unit\":\"...\",\"product_type\":\"...\",\"is_service\":false}}.\n\n\
         Артикул: {name}\n\
         Описание: {desc}\n\n\
         Кандидати от каталога NC8_TARIC:\n{cand_lines}"
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .ok()?;
    let body = json!({
        "model": MISTRAL_MODEL,
        "temperature": 0.0,
        "response_format": { "type": "json_object" },
        "messages": [{ "role": "user", "content": prompt }],
    });
    let resp = client
        .post(MISTRAL_URL)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: Value = resp.json().await.ok()?;
    let content = v["choices"][0]["message"]["content"].as_str()?;
    let trimmed = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str(trimmed).ok()
}

fn known_unit(code: &str) -> bool {
    crate::saft::catalog()
        .get("units")
        .map(|rows| rows.iter().any(|line| line.split('|').next() == Some(code)))
        .unwrap_or(false)
}

pub async fn suggest(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    auth_sub(&st, &headers)?;
    let name = body["name"].as_str().unwrap_or("").trim();
    let desc = body["description"].as_str().unwrap_or("").trim();
    let cur_unit = body["unit"].as_str().unwrap_or("").trim();
    let cur_ptype = body["product_type"].as_str().unwrap_or("").trim();
    let flag_srv = body["is_service"].as_bool().unwrap_or(false)
        || body["is_service"].as_i64() == Some(1);
    let combined = format!("{name} {desc}").trim().to_string();
    if combined.is_empty() {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "въведете име или описание на артикула",
        ));
    }
    let mut is_service = guess_is_service(&combined, flag_srv);
    let (_, mut cands) = rank_entries(&combined, is_service, 35);
    let cat = catalog();
    if cands.is_empty() {
        let fallback_code = if is_service { "00000000" } else { "0" };
        if let Some(&idx) = cat.by_code.get(fallback_code) {
            cands.push(&cat.entries[idx]);
        }
    }
    let mut chosen = cands[0];
    let mut unit = guess_saft_unit(&combined, is_service, cur_unit);
    let mut product_type = guess_product_type(&combined, is_service, cur_ptype);
    let mut source = "catalog";

    if let Some(key) = mistral_key(&st.db).await {
        if let Some(ai) = ask_mistral(&key, name, desc, &cands).await {
            source = "ai";
            if let Some(srv) = ai.is_service {
                is_service = srv;
            }
            if let Some(u) = ai.unit.as_deref() {
                let u = u.trim().to_uppercase();
                if !u.is_empty() {
                    unit = u;
                }
            }
            if let Some(pt) = ai.product_type.as_deref() {
                if matches!(pt.trim(), "10" | "20" | "30" | "40" | "50") {
                    product_type = pt.trim().to_string();
                }
            }
            let kw = ai.keywords.unwrap_or_default().join(" ");
            let hs4 = ai
                .hs4
                .as_deref()
                .unwrap_or("")
                .chars()
                .filter(|c| c.is_ascii_digit())
                .take(4)
                .collect::<String>();
            let raw_code: String = ai
                .cn8_code
                .as_deref()
                .unwrap_or("")
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if let Some(&idx) = cat.by_code.get(&raw_code) {
                chosen = &cat.entries[idx];
            } else {
                let prefix = if hs4.len() == 4 {
                    hs4
                } else if raw_code.len() >= 4 {
                    raw_code[..4].to_string()
                } else {
                    String::new()
                };
                let (_, drilled) = rank_entries(&format!("{combined} {prefix} {kw}"), is_service, 20);
                if let Some(&first) = drilled.first() {
                    chosen = first;
                    cands = drilled;
                }
            }
        }
    }
    if is_service {
        product_type.clear();
    }
    if !known_unit(&unit) {
        unit = guess_saft_unit(&combined, is_service, "");
    }
    let mut out_cands = vec![entry_json(chosen)];
    for c in cands.into_iter().take(15) {
        if c.code != chosen.code {
            out_cands.push(entry_json(c));
        }
    }
    Ok(Json(json!({
        "cn8_code": chosen.code,
        "cn8_name": chosen.full_name,
        "unit": unit,
        "product_type": product_type,
        "is_service": is_service,
        "source": source,
        "candidates": out_cands,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_and_matches_common_goods() {
        let cat = catalog();
        assert!(cat.entries.len() > 9800);
        assert!(cat.by_code.contains_key("00000000"));
        assert!(cat.by_code.contains_key("84713000"));

        let (_, laptops) = rank_entries("Лаптоп Lenovo ThinkPad 14 инча", false, 5);
        assert_eq!(laptops[0].code, "84713000");

        let (_, chairs) = rank_entries("Офис стол въртящ се", false, 5);
        assert!(chairs[0].code.starts_with("9401"));

        let (_, desks) = rank_entries("Дървено офис бюро", false, 5);
        assert!(desks[0].code.starts_with("9403"));

        let (_, srv) = rank_entries("Счетоводно обслужване", true, 5);
        assert_eq!(srv[0].code, "00000000");
    }

    #[test]
    fn saft_unit_and_type_heuristics() {
        assert_eq!(guess_saft_unit("Портланд цимент 25 кг", false, "C62"), "KGM");
        assert_eq!(guess_saft_unit("Дизелово гориво 50 литра", false, "C62"), "LTR");
        assert_eq!(guess_saft_unit("Лаптоп Lenovo", false, "C62"), "H87");
        assert_eq!(guess_product_type("Портланд цимент 25 кг", false, ""), "10");
        assert_eq!(guess_product_type("Лаптоп за препродажба", false, ""), "30");
    }
}
