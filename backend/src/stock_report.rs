//! Справка за склада: приход, разход и наличност към дата.
//! Чете същите движения като преизчислението и не пише в склада.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{cell_str, Db};
use crate::form::{active_or_one, auth_sub};
use crate::invoice_read;
use crate::reports_data::{self, Built};
use crate::stock_replay::{self, Balance, Flow};
use crate::{AppState, ApiError};

#[derive(Deserialize, Default)]
pub struct Q {
    from: Option<String>,
    to: Option<String>,
    location_id: Option<String>,
}

pub async fn report(State(st): State<AppState>, headers: HeaderMap, Query(q): Query<Q>) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let to = q.to.unwrap_or_default();
    let to = if iso(&to) { to } else { "9999-12-31".into() };
    let from = q.from.unwrap_or_default();
    let from = if iso(&from) { from } else { "0001-01-01".into() };
    let loc = q.location_id.unwrap_or_default().trim().parse::<i32>().unwrap_or(0);
    let out = stock_replay::explain(&st.db, company, &to).await?;
    let in_range = |f: &Flow| f.date.as_str() >= from.as_str() && f.date.as_str() <= to.as_str() && (loc <= 0 || f.location_id == loc);
    let receipts: Vec<Value> = out.flows.iter().filter(|f| f.kind == "in" && in_range(f)).map(flow_json).collect();
    let issues: Vec<Value> = out.flows.iter().filter(|f| f.kind == "out" && in_range(f)).map(flow_json).collect();
    let balances: Vec<Value> = out
        .balances
        .iter()
        .filter(|b| loc <= 0 || b.location_id == loc)
        .map(bal_json)
        .collect();
    Ok(Json(json!({
        "from": from,
        "to": if to == "9999-12-31" { "" } else { &to },
        "receipts": receipts,
        "issues": issues,
        "balances": balances,
        "warnings": out.warnings,
    })))
}

fn iso(s: &str) -> bool {
    s.len() == 10 && s.as_bytes().get(4) == Some(&b'-') && s.as_bytes().get(7) == Some(&b'-')
}

fn flow_json(f: &Flow) -> Value {
    json!({
        "date": f.date,
        "document": f.doc,
        "kind": f.kind,
        "reason": f.reason,
        "product_id": f.product_id,
        "product_name": f.product_name,
        "location_id": f.location_id,
        "quantity": f.quantity,
        "value": f.value,
    })
}

fn bal_json(b: &Balance) -> Value {
    json!({
        "product_id": b.product_id,
        "product_name": b.product_name,
        "location_id": b.location_id,
        "quantity": b.quantity,
        "value": b.value,
    })
}

pub async fn sheet(db: &Db, company: i32, from: &str, to: &str, loc: i32) -> Result<Built, ApiError> {
    let out = stock_replay::explain(db, company, to).await?;
    let in_range = |f: &Flow| f.date.as_str() >= from && f.date.as_str() <= to && (loc <= 0 || f.location_id == loc);
    let receipts: Vec<&Flow> = out.flows.iter().filter(|f| f.kind == "in" && in_range(f)).collect();
    let issues: Vec<&Flow> = out.flows.iter().filter(|f| f.kind == "out" && in_range(f)).collect();
    let balances: Vec<&Balance> = out.balances.iter().filter(|b| loc <= 0 || b.location_id == loc).collect();
    let firm = invoice_read::qry(db, &format!("SELECT name, eik FROM companies WHERE id = {company}")).await?;
    let (name, eik) = firm.first().map(|r| (cell_str(r, 0), cell_str(r, 1))).unwrap_or_default();
    let mut grid = Vec::new();
    let line = reports_data::firm_line(&name, &eik);
    if !line.is_empty() {
        grid.push(vec![line]);
    }
    grid.push(vec!["Склад".into(), from.into(), to.into()]);
    push_flows(&mut grid, "Приход", &receipts);
    push_flows(&mut grid, "Разход", &issues);
    grid.push(vec![]);
    grid.push(vec!["Наличности".into()]);
    grid.push(vec!["Артикул".into(), "Обект".into(), "Количество".into(), "Стойност".into()]);
    for b in &balances {
        grid.push(vec![b.product_name.clone(), b.location_id.to_string(), b.quantity.clone(), b.value.clone()]);
    }
    Ok(Built {
        body: json!({
            "from": from,
            "to": to,
            "receipts": receipts.iter().map(|f| flow_json(f)).collect::<Vec<_>>(),
            "issues": issues.iter().map(|f| flow_json(f)).collect::<Vec<_>>(),
            "balances": balances.iter().map(|b| bal_json(b)).collect::<Vec<_>>(),
            "warnings": out.warnings,
        }),
        slug: "sklad".into(),
        grid,
    })
}

fn push_flows(grid: &mut Vec<Vec<String>>, title: &str, rows: &[&Flow]) {
    grid.push(vec![]);
    grid.push(vec![title.into()]);
    grid.push(vec![
        "Дата".into(),
        "Документ".into(),
        "Артикул".into(),
        "Обект".into(),
        "Количество".into(),
        "Стойност".into(),
        "Основание".into(),
    ]);
    for f in rows {
        grid.push(vec![
            f.date.clone(),
            f.doc.clone(),
            f.product_name.clone(),
            f.location_id.to_string(),
            f.quantity.clone(),
            f.value.clone(),
            f.reason.clone(),
        ]);
    }
}
