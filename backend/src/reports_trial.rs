//! Шестколонна оборотна ведомост, и същата по контрагент.

use serde_json::{json, Value};

use crate::reports_data::{self, get, money, starts, turns, Books, Built};

pub fn build(kind: &str, b: &Books) -> Built {
    let t = turns(b);
    let mut rows = Vec::new();
    let mut grid = vec![
        vec![reports_data::title(kind).to_string()],
        vec![format!("{} — {}", b.from, b.to)],
        vec![
            "Сметка".into(),
            "Наименование".into(),
            "НД".into(),
            "НК".into(),
            "ОД".into(),
            "ОК".into(),
            "КД".into(),
            "КК".into(),
        ],
    ];
    let mut tod = 0i128;
    let mut toc = 0i128;
    let mut ttd = 0i128;
    let mut ttc = 0i128;
    let mut tcd = 0i128;
    let mut tcc = 0i128;
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
        tod = tod.saturating_add(od);
        toc = toc.saturating_add(oc);
        ttd = ttd.saturating_add(td);
        ttc = ttc.saturating_add(tc);
        tcd = tcd.saturating_add(cd);
        tcc = tcc.saturating_add(cc);
        rows.push(json!({
            "account_number": acc.number,
            "account_name": acc.name,
            "opening_debit": money(od),
            "opening_credit": money(oc),
            "turnover_debit": money(td),
            "turnover_credit": money(tc),
            "closing_debit": money(cd),
            "closing_credit": money(cc),
        }));
        grid.push(vec![
            acc.number.clone(),
            acc.name.clone(),
            money(od),
            money(oc),
            money(td),
            money(tc),
            money(cd),
            money(cc),
        ]);
    }
    grid.push(vec![
        String::new(),
        "Общо".into(),
        money(tod),
        money(toc),
        money(ttd),
        money(ttc),
        money(tcd),
        money(tcc),
    ]);
    let mut body = reports_data::head(b, kind);
    body["rows"] = Value::Array(rows);
    body["totals"] = json!({
        "opening_debit": money(tod),
        "opening_credit": money(toc),
        "turnover_debit": money(ttd),
        "turnover_credit": money(ttc),
        "closing_debit": money(tcd),
        "closing_credit": money(tcc),
    });
    Built {
        body,
        slug: reports_data::slug(kind).into(),
        grid,
    }
}
