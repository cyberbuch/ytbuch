//! Сметкоплан и дневник за месечния SAF-T.
//! AccountID е код на НАП. TaxpayerAccountID е номерът във фирмата.
//! Контрагентът е на записа, защото редът няма counterpart_id.

use std::collections::HashMap;

use crate::dec::money;
use crate::saft_load::{Entry, Line, Snap};
use crate::saft_xml::{self, amount, bal, bucket, clip, same_co, text, window, Window};

pub fn accounts_xml(s: &Snap, year: i32, month: i32) -> String {
    let w = window(year, month, s.company.fys);
    let map = balances(s, &w);
    let mut xml = saft_xml::open("GeneralLedgerAccounts");
    for a in &s.accounts {
        let od = get(&map, &format!("a:{}:od", a.id));
        let oc = get(&map, &format!("a:{}:oc", a.id));
        let pd = get(&map, &format!("a:{}:pd", a.id));
        let pc = get(&map, &format!("a:{}:pc", a.id));
        let desc = {
            let d = clip(&a.name, 256);
            if d.is_empty() { a.nra.clone() } else { d }
        };
        xml.push_str(&saft_xml::open("Account"));
        xml.push_str(&text("AccountID", &a.nra));
        xml.push_str(&text("AccountDescription", &desc));
        xml.push_str(&text("TaxpayerAccountID", &a.firm));
        xml.push_str(&text("AccountType", "Bifunctional"));
        xml.push_str(&bal(
            "OpeningDebitBalance",
            "OpeningCreditBalance",
            od,
            oc,
        ));
        xml.push_str(&bal(
            "ClosingDebitBalance",
            "ClosingCreditBalance",
            od + pd,
            oc + pc,
        ));
        xml.push_str(&saft_xml::close("Account"));
    }
    xml.push_str(&saft_xml::close("GeneralLedgerAccounts"));
    xml
}

pub fn entries_xml(s: &Snap, year: i32, month: i32) -> String {
    let w = window(year, month, s.company.fys);
    let cur = currency(s);
    let mut body = String::new();
    let mut n = 0i64;
    let mut debit = 0i128;
    let mut credit = 0i128;
    for e in &s.entries {
        if !same_co(e.company_id, s.company_id) || !e.date.starts_with(&w.prefix) {
            continue;
        }
        let day = day(&e.date);
        if day.len() < 10 {
            continue;
        }
        let Some(built) = tx(s, e, &day, year, month, &cur, &mut debit, &mut credit) else {
            continue;
        };
        body.push_str(&built);
        n += 1;
    }
    let mut xml = saft_xml::open("GeneralLedgerEntries");
    xml.push_str(&text("NumberOfEntries", &n.to_string()));
    xml.push_str(&text("TotalDebit", &money(debit)));
    xml.push_str(&text("TotalCredit", &money(credit)));
    xml.push_str(&saft_xml::open("Journal"));
    xml.push_str(&text("JournalID", "GL"));
    xml.push_str(&text("Description", "Счетоводни записи"));
    xml.push_str(&text("Type", "GLEntry"));
    xml.push_str(&body);
    xml.push_str(&saft_xml::close("Journal"));
    xml.push_str(&saft_xml::close("GeneralLedgerEntries"));
    xml
}

fn tx(
    s: &Snap,
    e: &Entry,
    day: &str,
    year: i32,
    month: i32,
    cur: &str,
    debit: &mut i128,
    credit: &mut i128,
) -> Option<String> {
    let lines = s.lines.get(&e.id)?;
    let desc = {
        let d = clip(&e.description, 256);
        if d.is_empty() { "Запис".into() } else { d }
    };
    let mut body = String::new();
    let mut n = 0;
    let mut tx_c = "0".to_string();
    let mut tx_s = "0".to_string();
    for line in lines {
        let Some(acc) = s.accounts.iter().find(|a| a.id == line.account_id) else {
            continue;
        };
        if line.direction == "debit" {
            *debit += line.amount;
        } else {
            *credit += line.amount;
        }
        let (cust, supp) = if saft_xml::pref(&acc.nra, "411") || saft_xml::pref(&acc.nra, "401") {
            who(s, e.counterpart_id, &line.direction)
        } else {
            ("0".into(), "0".into())
        };
        if cust != "0" {
            tx_c = cust.clone();
        }
        if supp != "0" {
            tx_s = supp.clone();
        }
        body.push_str(&saft_xml::open("TransactionLine"));
        body.push_str(&text("RecordID", &line.id.to_string()));
        body.push_str(&text("AccountID", &acc.nra));
        body.push_str(&text("TaxpayerAccountID", &acc.firm));
        body.push_str(&text("CustomerID", &cust));
        body.push_str(&text("SupplierID", &supp));
        body.push_str(&text("Description", &desc));
        if line.direction == "debit" {
            body.push_str(&amount("DebitAmount", line.amount, cur));
        } else {
            body.push_str(&amount("CreditAmount", line.amount, cur));
        }
        body.push_str(&tax_info(line, cur));
        body.push_str(&saft_xml::close("TransactionLine"));
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let mut xml = saft_xml::open("Transaction");
    xml.push_str(&text("TransactionID", &e.id.to_string()));
    xml.push_str(&text("Period", &month.to_string()));
    xml.push_str(&text("PeriodYear", &year.to_string()));
    xml.push_str(&text("TransactionDate", day));
    xml.push_str(&text("TransactionType", "Normal"));
    xml.push_str(&text("Description", &desc));
    xml.push_str(&text("SystemEntryDate", day));
    xml.push_str(&text("GLPostingDate", day));
    xml.push_str(&text("CustomerID", &tx_c));
    xml.push_str(&text("SupplierID", &tx_s));
    xml.push_str(&body);
    xml.push_str(&saft_xml::close("Transaction"));
    Some(xml)
}

fn tax_info(line: &Line, cur: &str) -> String {
    let mut xml = saft_xml::open("TaxInformation");
    if line.vat != 0 {
        xml.push_str(&text("TaxType", "100"));
        xml.push_str(&text("TaxCode", "100211"));
        xml.push_str(&amount("TaxAmount", line.vat, cur));
    } else {
        xml.push_str(&text("TaxType", "000"));
        xml.push_str(&text("TaxCode", "000000"));
        xml.push_str(&amount("TaxAmount", 0, cur));
    }
    xml.push_str(&saft_xml::close("TaxInformation"));
    xml
}

/// Клиент само на дебит при двойна роля. Иначе по вида на контрагента.
pub fn who(s: &Snap, counterpart_id: i32, direction: &str) -> (String, String) {
    let Some(p) = s.parties.iter().find(|p| p.id == counterpart_id) else {
        return ("0".into(), "0".into());
    };
    let eik = {
        let t = clip(&p.eik, 35);
        if t.is_empty() { "0".into() } else { t }
    };
    let (client, supplier) = roles(&p.kind);
    if client && !supplier {
        return (eik, "0".into());
    }
    if supplier && !client {
        return ("0".into(), eik);
    }
    if client && supplier {
        if direction == "debit" {
            return (eik, "0".into());
        }
        return ("0".into(), eik);
    }
    ("0".into(), "0".into())
}

pub fn roles(kind: &str) -> (bool, bool) {
    match kind.trim() {
        "customer" | "client" => (true, false),
        "supplier" => (false, true),
        "both" => (true, true),
        _ => (false, false),
    }
}

pub fn balances(s: &Snap, w: &Window) -> HashMap<String, i128> {
    let mut map = HashMap::new();
    let fy = w.fy_start[..4].parse().unwrap_or(0);
    for o in &s.openings {
        if o.year != fy || !s.nra.contains_key(&o.account_id) {
            continue;
        }
        add(&mut map, &format!("a:{}:od", o.account_id), o.debit);
        add(&mut map, &format!("a:{}:oc", o.account_id), o.credit);
        if o.counterpart_id > 0 {
            add_party(&mut map, s, o.account_id, o.counterpart_id, "o", "debit", o.debit);
            add_party(&mut map, s, o.account_id, o.counterpart_id, "o", "credit", o.credit);
        }
    }
    for e in &s.entries {
        if !same_co(e.company_id, s.company_id) {
            continue;
        }
        let Some(kind) = bucket(&e.date, w) else {
            continue;
        };
        let Some(lines) = s.lines.get(&e.id) else {
            continue;
        };
        for line in lines {
            if !s.nra.contains_key(&line.account_id) {
                continue;
            }
            let side = if line.direction == "debit" { "d" } else { "c" };
            let slot = if kind == 'o' { "o" } else { "p" };
            add(
                &mut map,
                &format!("a:{}:{slot}{side}", line.account_id),
                line.amount,
            );
            if e.counterpart_id > 0 {
                add_party(
                    &mut map,
                    s,
                    line.account_id,
                    e.counterpart_id,
                    slot,
                    &line.direction,
                    line.amount,
                );
            }
        }
    }
    map
}

fn add_party(
    map: &mut HashMap<String, i128>,
    s: &Snap,
    account_id: i32,
    counterpart_id: i32,
    slot: &str,
    direction: &str,
    amount: i128,
) {
    let Some(code) = s.nra.get(&account_id) else {
        return;
    };
    let side = if direction == "debit" { "d" } else { "c" };
    if saft_xml::pref(code, "411") {
        add(map, &format!("p:{counterpart_id}:c:{slot}{side}"), amount);
    }
    if saft_xml::pref(code, "401") {
        add(map, &format!("p:{counterpart_id}:s:{slot}{side}"), amount);
    }
}

fn add(map: &mut HashMap<String, i128>, key: &str, amount: i128) {
    *map.entry(key.to_string()).or_insert(0) += amount;
}

pub fn get(map: &HashMap<String, i128>, key: &str) -> i128 {
    map.get(key).copied().unwrap_or(0)
}

pub fn currency(s: &Snap) -> String {
    let c = s.company.currency.trim();
    if c.is_empty() { "EUR".into() } else { c.to_string() }
}

fn day(raw: &str) -> String {
    raw.chars().take(10).collect()
}
