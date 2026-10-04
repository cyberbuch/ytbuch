//! Главна книга по кореспонденции: водещ дебит и водещ кредит.

use serde_json::{json, Value};

use crate::reports_data::{self, correspondences, get, money, starts, turns, Books, Built};

pub fn build(kind: &str, b: &Books) -> Built {
    let t = turns(b);
    let pairs = correspondences(b);
    let names = reports_data::names(b);
    let mut debit_lead = Vec::new();
    let mut credit_lead = Vec::new();
    let mut grid = vec![
        vec![reports_data::title(kind).to_string()],
        vec![format!("{} — {}", b.from, b.to)],
        vec!["Сметка".into(), "Кореспонденция".into(), "Дебит".into(), "Кредит".into()],
    ];
    for side in 0..2 {
        grid.push(vec![
            if side == 0 { "Водещ дебит" } else { "Водещ кредит" }.into(),
            String::new(),
            String::new(),
            String::new(),
        ]);
        let mut lead = Vec::new();
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
            let mut rows = Vec::new();
            grid.push(vec![
                format!("{} {}", acc.number, acc.name),
                "Начално салдо".into(),
                money(od),
                money(oc),
            ]);
            for other in &b.accounts {
                let key = if side == 0 {
                    (acc.number.clone(), other.number.clone())
                } else {
                    (other.number.clone(), acc.number.clone())
                };
                let Some(amount) = pairs.get(&key).copied() else { continue };
                if amount == 0 {
                    continue;
                }
                let (d, c) = if side == 0 { (amount, 0) } else { (0, amount) };
                let cname = names.get(&other.number).cloned().unwrap_or_default();
                rows.push(json!({
                    "correspondent": other.number,
                    "correspondent_name": cname,
                    "debit": money(d),
                    "credit": money(c),
                }));
                let lab = if side == 0 {
                    format!("Кт {} {}", other.number, cname)
                } else {
                    format!("Дт {} {}", other.number, cname)
                };
                grid.push(vec![String::new(), lab, money(d), money(c)]);
            }
            let cd = od.saturating_add(td);
            let cc = oc.saturating_add(tc);
            grid.push(vec![String::new(), "Обороти за периода".into(), money(td), money(tc)]);
            grid.push(vec![String::new(), "Крайно салдо".into(), money(cd), money(cc)]);
            lead.push(json!({
                "account_number": acc.number,
                "account_name": acc.name,
                "opening_debit": money(od),
                "opening_credit": money(oc),
                "turnover_debit": money(td),
                "turnover_credit": money(tc),
                "closing_debit": money(cd),
                "closing_credit": money(cc),
                "rows": rows,
            }));
        }
        if side == 0 {
            debit_lead = lead;
        } else {
            credit_lead = lead;
        }
    }
    let mut body = reports_data::head(b, kind);
    body["debit_lead"] = Value::Array(debit_lead);
    body["credit_lead"] = Value::Array(credit_lead);
    Built {
        body,
        slug: reports_data::slug(kind).into(),
        grid,
    }
}
