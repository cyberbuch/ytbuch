//! Общи данни за справките: дневник, салда и обороти.
//! Крайното салдо е начално плюс оборот, без нетиране.
//! Контрагентът се филтрира по записа: редът на дневника няма counterpart_id.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, Db};
use crate::dec;
use crate::ApiError;

#[derive(Clone)]
pub struct Acc {
    pub id: i32,
    pub number: String,
    pub name: String,
}

#[derive(Clone)]
pub struct Entry {
    pub id: i32,
    pub day: String,
    pub doc: String,
    pub desc: String,
    pub cp_id: i32,
    pub cp_name: String,
}

#[derive(Clone)]
pub struct Line {
    pub entry_id: i32,
    pub account_id: i32,
    pub debit: bool,
    pub amount: i128,
}

#[derive(Clone)]
pub struct Open {
    pub account_id: i32,
    pub cp_id: i32,
    pub debit: i128,
    pub credit: i128,
}

pub struct Books {
    pub company_id: i32,
    pub accounts: Vec<Acc>,
    pub entries: Vec<Entry>,
    pub lines: Vec<Line>,
    pub openings: Vec<Open>,
    pub from: String,
    pub to: String,
    pub fiscal: String,
    pub account: String,
    pub cpid: i32,
    pub firm_name: String,
    pub firm_eik: String,
}

pub struct Turn {
    pub od: HashMap<i32, i128>,
    pub oc: HashMap<i32, i128>,
    pub td: HashMap<i32, i128>,
    pub tc: HashMap<i32, i128>,
}

pub struct Built {
    pub body: Value,
    pub slug: String,
    pub grid: Vec<Vec<String>>,
}

pub fn money(v: i128) -> String {
    dec::money(v)
}

pub fn day_ok(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

pub fn in_range(day: &str, from: &str, to: &str, exclusive_end: bool) -> bool {
    if day.len() < 10 || day < from {
        return false;
    }
    if exclusive_end {
        day < to
    } else {
        day <= to
    }
}

pub fn starts(number: &str, prefix: &str) -> bool {
    prefix.is_empty() || number.starts_with(prefix)
}

/// Шаблон `411` и `50*` хващат сметка, която започва с този префикс.
pub fn acc_match(account: &str, pattern: &str) -> bool {
    let prefix = pattern.strip_suffix('*').unwrap_or(pattern);
    !prefix.is_empty() && account.starts_with(prefix)
}

fn add(map: &mut HashMap<i32, i128>, key: i32, amount: i128) {
    if amount == 0 {
        return;
    }
    let slot = map.entry(key).or_insert(0i128);
    *slot = slot.saturating_add(amount);
}

pub fn get(map: &HashMap<i32, i128>, key: i32) -> i128 {
    map.get(&key).copied().unwrap_or(0)
}

pub fn turns(b: &Books) -> Turn {
    let mut t = Turn {
        od: HashMap::new(),
        oc: HashMap::new(),
        td: HashMap::new(),
        tc: HashMap::new(),
    };
    for row in &b.openings {
        if b.cpid > 0 && row.cp_id != b.cpid {
            continue;
        }
        add(&mut t.od, row.account_id, row.debit);
        add(&mut t.oc, row.account_id, row.credit);
    }
    let days: HashMap<i32, &str> = b.entries.iter().map(|e| (e.id, e.day.as_str())).collect();
    let cps: HashMap<i32, i32> = b.entries.iter().map(|e| (e.id, e.cp_id)).collect();
    let from_is_fiscal = b.from == b.fiscal;
    for line in &b.lines {
        if b.cpid > 0 && cps.get(&line.entry_id).copied().unwrap_or(0) != b.cpid {
            continue;
        }
        let day = days.get(&line.entry_id).copied().unwrap_or("");
        if !from_is_fiscal && in_range(day, &b.fiscal, &b.from, true) {
            if line.debit {
                add(&mut t.od, line.account_id, line.amount);
            } else {
                add(&mut t.oc, line.account_id, line.amount);
            }
        }
        if in_range(day, &b.from, &b.to, false) {
            if line.debit {
                add(&mut t.td, line.account_id, line.amount);
            } else {
                add(&mut t.tc, line.account_id, line.amount);
            }
        }
    }
    t
}

pub fn numbers(b: &Books) -> HashMap<i32, String> {
    b.accounts.iter().map(|a| (a.id, a.number.clone())).collect()
}

pub fn names(b: &Books) -> HashMap<String, String> {
    b.accounts.iter().map(|a| (a.number.clone(), a.name.clone())).collect()
}

/// Салда и обороти по номер. Повтарящ се номер се събира.
pub fn by_number(b: &Books, t: &Turn) -> [HashMap<String, i128>; 4] {
    let nums = numbers(b);
    let mut out = [HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new()];
    let src = [&t.od, &t.oc, &t.td, &t.tc];
    for (i, map) in src.iter().enumerate() {
        for (id, amount) in *map {
            let Some(num) = nums.get(id) else { continue };
            let slot = out[i].entry(num.clone()).or_insert(0i128);
            *slot = slot.saturating_add(*amount);
        }
    }
    out
}

pub struct Side {
    pub num: String,
    pub rem: i128,
}

/// FIFO двойки дебит/кредит, както в хронологичния журнал.
pub fn pair(debits: &mut [Side], credits: &mut [Side]) -> Vec<(String, String, i128)> {
    let mut out = Vec::new();
    let mut di = 0;
    let mut ci = 0;
    while di < debits.len() && ci < credits.len() {
        if debits[di].rem == 0 {
            di += 1;
            continue;
        }
        if credits[ci].rem == 0 {
            ci += 1;
            continue;
        }
        let amount = debits[di].rem.min(credits[ci].rem);
        out.push((debits[di].num.clone(), credits[ci].num.clone(), amount));
        debits[di].rem -= amount;
        credits[ci].rem -= amount;
    }
    out
}

pub fn correspondences(b: &Books) -> HashMap<(String, String), i128> {
    let nums = numbers(b);
    let mut by_entry: HashMap<i32, Vec<&Line>> = HashMap::new();
    for line in &b.lines {
        by_entry.entry(line.entry_id).or_default().push(line);
    }
    let mut pairs: HashMap<(String, String), i128> = HashMap::new();
    for entry in &b.entries {
        if !in_range(&entry.day, &b.from, &b.to, false) {
            continue;
        }
        let Some(lines) = by_entry.get(&entry.id) else { continue };
        let mut debits = Vec::new();
        let mut credits = Vec::new();
        for line in lines {
            let num = nums.get(&line.account_id).cloned().unwrap_or_default();
            let side = Side { num, rem: line.amount };
            if line.debit {
                debits.push(side);
            } else {
                credits.push(side);
            }
        }
        for (d, c, amount) in pair(&mut debits, &mut credits) {
            if amount == 0 {
                continue;
            }
            let slot = pairs.entry((d, c)).or_insert(0i128);
            *slot = slot.saturating_add(amount);
        }
    }
    pairs
}

pub fn title(kind: &str) -> &'static str {
    match kind {
        "trial_balance" => "Оборотна ведомост",
        "general_ledger" => "Главна книга",
        "chronological" => "Хронологичен журнал",
        "counterpart_trial" => "Оборотна ведомост по контрагент",
        "counterpart_chrono" => "Хронологичен журнал по контрагент",
        "correspondence_ledger" => "Главна книга по кореспонденции",
        "balance_sheet" => "Счетоводен баланс",
        "income_statement" => "Отчет за приходите и разходите",
        "cash_flow" => "Отчет за паричните потоци",
        "equity" => "Отчет за собствения капитал",
        _ => "Справка",
    }
}

pub fn slug(kind: &str) -> &'static str {
    match kind {
        "trial_balance" => "oborotna",
        "general_ledger" => "glavna-kniga",
        "chronological" => "hronologichen",
        "counterpart_trial" => "oborotna-kontragent",
        "counterpart_chrono" => "hronologichen-kontragent",
        "correspondence_ledger" => "glavna-kniga-korespondencii",
        "balance_sheet" => "balans",
        "income_statement" => "opr",
        "cash_flow" => "parichen-potok",
        "equity" => "sobstven-kapital",
        "stock" => "sklad",
        _ => "spravka",
    }
}

pub fn kind_ok(kind: &str) -> bool {
    matches!(
        kind,
        "trial_balance"
            | "general_ledger"
            | "chronological"
            | "counterpart_trial"
            | "counterpart_chrono"
            | "correspondence_ledger"
            | "balance_sheet"
            | "income_statement"
            | "cash_flow"
            | "equity"
            | "stock"
    )
}

pub fn fs_statement(kind: &str) -> Option<&'static str> {
    match kind {
        "balance_sheet" => Some("balance"),
        "income_statement" => Some("pl"),
        "cash_flow" => Some("cashflow"),
        "equity" => Some("equity"),
        _ => None,
    }
}

pub fn head(b: &Books, kind: &str) -> Value {
    json!({
        "kind": kind,
        "from": b.from,
        "to": b.to,
        "title": title(kind),
        "company_name": b.firm_name,
        "company_eik": b.firm_eik,
    })
}

pub fn firm_line(name: &str, eik: &str) -> String {
    match (name.is_empty(), eik.is_empty()) {
        (true, true) => String::new(),
        (false, true) => name.to_string(),
        (true, false) => format!("ЕИК {eik}"),
        (false, false) => format!("{name} — ЕИК {eik}"),
    }
}

pub async fn load(db: &Db, company_id: i32, from: &str, to: &str, account: &str, cpid: i32) -> Result<Books, ApiError> {
    let year: i32 = from.get(..4).unwrap_or("0").parse().unwrap_or(0);
    let company = crate::invoice_read::qry(db, &format!("SELECT name, eik FROM companies WHERE id = {company_id}")).await?;
    let (firm_name, firm_eik) = company
        .first()
        .map(|r| (cell_str(r, 0), cell_str(r, 1)))
        .unwrap_or_default();
    let acc_rows = crate::invoice_read::qry(
        db,
        &format!("SELECT id, number, name FROM accounts WHERE company_id = {company_id} ORDER BY number, id"),
    )
    .await?;
    let accounts: Vec<Acc> = acc_rows
        .iter()
        .map(|r| Acc {
            id: cell_i32_at(r, 0),
            number: cell_str(r, 1),
            name: cell_str(r, 2),
        })
        .collect();
    let ids: std::collections::HashSet<i32> = accounts.iter().map(|a| a.id).collect();
    let entry_rows = crate::invoice_read::qry(
        db,
        &format!(
            "SELECT id, entry_date, document_number, description, counterpart_id, counterpart_name \
             FROM journal_entries WHERE company_id = {company_id} ORDER BY entry_date, id"
        ),
    )
    .await?;
    let entries: Vec<Entry> = entry_rows
        .iter()
        .map(|r| Entry {
            id: cell_i32_at(r, 0),
            day: cell_str(r, 1).chars().take(10).collect(),
            doc: cell_str(r, 2),
            desc: cell_str(r, 3),
            cp_id: cell_i32_at(r, 4),
            cp_name: cell_str(r, 5),
        })
        .collect();
    let entry_ids: std::collections::HashSet<i32> = entries.iter().map(|e| e.id).collect();
    let line_rows = crate::invoice_read::qry(
        db,
        "SELECT entry_id, account_id, direction, amount FROM journal_lines ORDER BY id",
    )
    .await?;
    let lines = line_rows
        .iter()
        .filter_map(|r| {
            let entry_id = cell_i32_at(r, 0);
            if !entry_ids.contains(&entry_id) {
                return None;
            }
            Some(Line {
                entry_id,
                account_id: cell_i32_at(r, 1),
                debit: cell_str(r, 2) == "debit",
                amount: dec::or0(&cell_str(r, 3)),
            })
        })
        .collect();
    let open_rows = crate::invoice_read::qry(
        db,
        &format!(
            "SELECT account_id, debit, credit, counterpart_id FROM opening_balances WHERE fiscal_year = {year} ORDER BY id"
        ),
    )
    .await?;
    let openings = open_rows
        .iter()
        .filter_map(|r| {
            let account_id = cell_i32_at(r, 0);
            if !ids.contains(&account_id) {
                return None;
            }
            Some(Open {
                account_id,
                debit: dec::or0(&cell_str(r, 1)),
                credit: dec::or0(&cell_str(r, 2)),
                cp_id: cell_i32_at(r, 3),
            })
        })
        .collect();
    Ok(Books {
        company_id,
        accounts,
        entries,
        lines,
        openings,
        from: from.to_string(),
        to: to.to_string(),
        fiscal: format!("{year:04}-01-01"),
        account: account.to_string(),
        cpid,
        firm_name,
        firm_eik,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_and_prefix() {
        assert!(day_ok("2026-10-01"));
        assert!(!day_ok("2026-10-1"));
        assert!(in_range("2026-01-15", "2026-01-01", "2026-10-01", true));
        assert!(!in_range("2026-10-01", "2026-01-01", "2026-10-01", true));
        assert!(in_range("2026-10-01", "2026-10-01", "2026-10-01", false));
        assert!(acc_match("4111", "411"));
        assert!(acc_match("501", "50*"));
        assert!(!acc_match("601", "50*"));
    }

    #[test]
    fn pairs_fifo() {
        let mut d = vec![Side { num: "411".into(), rem: dec::or0("1200") }];
        let mut c = vec![
            Side { num: "702".into(), rem: dec::or0("1000") },
            Side { num: "4532".into(), rem: dec::or0("200") },
        ];
        let got = pair(&mut d, &mut c);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].2, dec::or0("1000"));
        assert_eq!(got[1].2, dec::or0("200"));
    }
}
