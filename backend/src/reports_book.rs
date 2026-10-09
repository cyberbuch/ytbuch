//! Хронологичен журнал (FIFO двойки) и главна книга.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::reports_data::{self, get, in_range, money, pair, starts, turns, Books, Built, Side};

fn grouped(b: &Books) -> HashMap<i32, Vec<&reports_data::Line>> {
    let mut map = HashMap::new();
    for line in &b.lines {
        map.entry(line.entry_id).or_insert_with(Vec::new).push(line);
    }
    map
}

pub fn chrono(kind: &str, b: &Books) -> Built {
    let nums = reports_data::numbers(b);
    let groups = grouped(b);
    let mut rows = Vec::new();
    let mut grid = vec![
        vec![reports_data::title(kind).to_string()],
        vec![format!("{} — {}", b.from, b.to)],
        vec![
            "№".into(),
            "Дата".into(),
            "Документ".into(),
            "Описание".into(),
            "Контрагент".into(),
            "Дт сметка".into(),
            "Кт сметка".into(),
            "Дебит".into(),
            "Кредит".into(),
        ],
    ];
    let mut n = 1i64;
    let mut td = 0i128;
    let mut tc = 0i128;
    for entry in &b.entries {
        if !in_range(&entry.day, &b.from, &b.to, false) {
            continue;
        }
        if b.cpid > 0 && entry.cp_id != b.cpid {
            continue;
        }
        let Some(lines) = groups.get(&entry.id) else { continue };
        let mut debits = Vec::new();
        let mut credits = Vec::new();
        let mut acc_ok = b.account.is_empty();
        for line in lines {
            let num = nums.get(&line.account_id).cloned().unwrap_or_default();
            if starts(&num, &b.account) {
                acc_ok = true;
            }
            let side = Side { num, rem: line.amount };
            if line.debit {
                debits.push(side);
            } else {
                credits.push(side);
            }
        }
        if !acc_ok {
            continue;
        }
        for (da, ca, amount) in pair(&mut debits, &mut credits) {
            if amount == 0 {
                continue;
            }
            td = td.saturating_add(amount);
            tc = tc.saturating_add(amount);
            rows.push(json!({
                "row_number": n,
                "date": entry.day,
                "document_number": entry.doc,
                "description": entry.desc,
                "counterpart_name": entry.cp_name,
                "debit_account": da,
                "credit_account": ca,
                "debit_amount": money(amount),
                "credit_amount": money(amount),
            }));
            grid.push(vec![
                n.to_string(),
                entry.day.clone(),
                entry.doc.clone(),
                entry.desc.clone(),
                entry.cp_name.clone(),
                da,
                ca,
                money(amount),
                money(amount),
            ]);
            n += 1;
        }
    }
    grid.push(vec![
        String::new(),
        String::new(),
        String::new(),
        "Общо".into(),
        String::new(),
        String::new(),
        String::new(),
        money(td),
        money(tc),
    ]);
    let mut body = reports_data::head(b, kind);
    body["rows"] = Value::Array(rows);
    body["totals"] = json!({ "debit_amount": money(td), "credit_amount": money(tc) });
    Built {
        body,
        slug: reports_data::slug(kind).into(),
        grid,
    }
}

pub fn ledger(kind: &str, b: &Books) -> Built {
    let t = turns(b);
    let groups = grouped(b);
    let mut accounts = Vec::new();
    let mut grid = vec![
        vec![reports_data::title(kind).to_string()],
        vec![format!("{} — {}", b.from, b.to)],
        vec![
            "Сметка".into(),
            "Дата".into(),
            "Документ".into(),
            "Описание".into(),
            "Дебит".into(),
            "Кредит".into(),
        ],
    ];
    for acc in &b.accounts {
        if !starts(&acc.number, &b.account) {
            continue;
        }
        let od = get(&t.od, acc.id);
        let oc = get(&t.oc, acc.id);
        let td = get(&t.td, acc.id);
        let tc = get(&t.tc, acc.id);
        if od == 0 && oc == 0 && td == 0 && tc == 0 {
            continue;
        }
        let cd = od.saturating_add(td);
        let cc = oc.saturating_add(tc);
        let label = format!("{} {}", acc.number, acc.name);
        grid.push(vec![label, String::new(), String::new(), "Начално салдо".into(), money(od), money(oc)]);
        let mut txs = Vec::new();
        let mut net = od.saturating_sub(oc);
        for entry in &b.entries {
            if !in_range(&entry.day, &b.from, &b.to, false) {
                continue;
            }
            let Some(lines) = groups.get(&entry.id) else { continue };
            for line in lines {
                if line.account_id != acc.id {
                    continue;
                }
                if b.cpid > 0 && entry.cp_id != b.cpid {
                    continue;
                }
                let (d, c) = if line.debit {
                    net = net.saturating_add(line.amount);
                    (line.amount, 0)
                } else {
                    net = net.saturating_sub(line.amount);
                    (0, line.amount)
                };
                let (rd, rc) = if net > 0 {
                    (net, 0)
                } else if net < 0 {
                    (0, -net)
                } else {
                    (0, 0)
                };
                txs.push(json!({
                    "date": entry.day,
                    "document_number": entry.doc,
                    "description": entry.desc,
                    "debit": money(d),
                    "credit": money(c),
                    "running_debit": money(rd),
                    "running_credit": money(rc),
                }));
                grid.push(vec![
                    String::new(),
                    entry.day.clone(),
                    entry.doc.clone(),
                    entry.desc.clone(),
                    money(d),
                    money(c),
                ]);
            }
        }
        grid.push(vec![String::new(), String::new(), String::new(), "Обороти за периода".into(), money(td), money(tc)]);
        grid.push(vec![String::new(), String::new(), String::new(), "Крайно салдо".into(), money(cd), money(cc)]);
        accounts.push(json!({
            "account_number": acc.number,
            "account_name": acc.name,
            "opening_debit": money(od),
            "opening_credit": money(oc),
            "turnover_debit": money(td),
            "turnover_credit": money(tc),
            "closing_debit": money(cd),
            "closing_credit": money(cc),
            "transactions": txs,
        }));
    }
    let mut body = reports_data::head(b, kind);
    body["accounts"] = Value::Array(accounts);
    Built {
        body,
        slug: reports_data::slug(kind).into(),
        grid,
    }
}
