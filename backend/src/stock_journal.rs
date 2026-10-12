//! Осчетоводяване след преизчисление: маха стария себестойностен ред от
//! фактурата и презаписва записите на производство и ръчно изписване.

use std::collections::{HashMap, HashSet};

use crate::db::{cell_i32_at, cell_str, lit, Db};
use crate::dec::{self, money};
use crate::invoice_read;
use crate::posting::{self, Side};
use crate::stock_replay::{Books, CostRow, Meta};
use crate::stock_rules::{self, Rule};
use crate::ApiError;

pub(crate) async fn strip_legacy(db: &Db, company: i32) -> Result<(), ApiError> {
    let entries = invoice_read::qry(
        db,
        &format!(
            "SELECT e.id FROM journal_entries e, invoices i WHERE e.document_id = i.id AND e.company_id = {company} \
             AND i.company_id = {company} AND i.direction = 'out' AND i.status = 'posted' \
             AND (i.document_type = '01' OR i.document_type = '11') \
             AND (e.document_type = '01' OR e.document_type = '11')"
        ),
    )
    .await?;
    for e in &entries {
        let id = cell_i32_at(e, 0);
        let lines = invoice_read::qry(
            db,
            &format!("SELECT id, account_id, direction, amount, vat_amount FROM journal_lines WHERE entry_id = {id}"),
        )
        .await?;
        let mut debit_acc = HashSet::new();
        let mut credit_acc = HashSet::new();
        for line in &lines {
            if cell_str(line, 2) == "credit" {
                credit_acc.insert(cell_i32_at(line, 1));
            } else {
                debit_acc.insert(cell_i32_at(line, 1));
            }
        }
        let dual: HashSet<i32> = debit_acc.intersection(&credit_acc).copied().collect();
        let mut drop_amt = Vec::new();
        let mut drop_id = Vec::new();
        for line in &lines {
            if cell_str(line, 2) == "debit" && dual.contains(&cell_i32_at(line, 1)) {
                drop_amt.push(dec::or0(&cell_str(line, 3)));
                drop_id.push(cell_i32_at(line, 0));
            }
        }
        for line in &lines {
            if cell_str(line, 2) != "credit" || dual.contains(&cell_i32_at(line, 1)) || dec::or0(&cell_str(line, 4)) != 0 {
                continue;
            }
            let amt = dec::or0(&cell_str(line, 3));
            if let Some(pos) = drop_amt.iter().position(|a| *a == amt) {
                drop_amt.remove(pos);
                drop_id.push(cell_i32_at(line, 0));
            }
        }
        for lid in drop_id {
            invoice_read::exe(db, &format!("DELETE FROM journal_lines WHERE id = {lid}")).await?;
        }
    }
    Ok(())
}

pub(crate) async fn rewrite(db: &Db, company: i32, products: &HashMap<i32, Meta>, books: &Books, warnings: &mut Vec<String>) -> Result<(), ApiError> {
    let rules = stock_rules::load(db, company).await?;
    rewrite_production(db, company, products, &rules, books, warnings).await?;
    rewrite_material(db, company, &rules, books, warnings).await?;
    Ok(())
}

async fn rewrite_production(db: &Db, company: i32, products: &HashMap<i32, Meta>, rules: &[Rule], books: &Books, warnings: &mut Vec<String>) -> Result<(), ApiError> {
    for order in &books.prods {
        let Some(out) = products.get(&order.output_id) else { continue };
        let Some(rule) = stock_rules::pair(rules, "production", out.inventory, out.revenue) else {
            warnings.push(format!("няма правило за продукция за {}", out.name));
            continue;
        };
        if rule.clearing_account_id <= 0 {
            warnings.push(format!("правилото за продукция няма междинна сметка ({})", out.name));
            continue;
        }
        let mut ok = true;
        let mut sides = Vec::new();
        let mut total = 0i128;
        let costs: Vec<&CostRow> = books.made.iter().filter(|c| c.doc == order.id).collect();
        for cost in &costs {
            let Some(mat) = products.get(&cost.product) else { continue };
            if stock_rules::material_inventory(rules, mat.inventory).is_none() {
                warnings.push(format!("няма правило за материал за {}", mat.name));
                ok = false;
                break;
            }
            if cost.cost == 0 {
                continue;
            }
            total = dec::add(total, cost.cost).unwrap_or(total);
            let clearing = posting::by_id(db, rule.clearing_account_id).await?;
            let inv = posting::by_id(db, mat.inventory).await?;
            sides.push(Side { account: clearing, direction: "debit", amount: money(cost.cost) });
            sides.push(Side { account: inv, direction: "credit", amount: money(cost.cost) });
        }
        if !ok {
            continue;
        }
        if total != 0 {
            let clearing = posting::by_id(db, rule.clearing_account_id).await?;
            let inv = posting::by_id(db, out.inventory).await?;
            sides.push(Side { account: inv, direction: "debit", amount: money(total) });
            sides.push(Side { account: clearing, direction: "credit", amount: money(total) });
        }
        let entry = write_fresh(db, company, "production", order.id, &order.number, &order.date, &format!("Производство {}", order.number), &sides).await?;
        store_prod_lines(db, order.id, &costs).await?;
        invoice_read::exe(
            db,
            &format!(
                "UPDATE production_orders SET material_cost = {}, journal_entry_id = {entry} WHERE id = {}",
                lit(&money(total)),
                order.id
            ),
        )
        .await?;
    }
    Ok(())
}

async fn store_prod_lines(db: &Db, order: i32, costs: &[&CostRow]) -> Result<(), ApiError> {
    let lines = invoice_read::qry(
        db,
        &format!("SELECT id, product_id FROM production_order_lines WHERE order_id = {order} ORDER BY id"),
    )
    .await?;
    let mut left: Vec<CostRow> = costs.iter().map(|c| CostRow { doc: c.doc, product: c.product, qty: c.qty, cost: c.cost }).collect();
    for line in &lines {
        let id = cell_i32_at(line, 0);
        let pid = cell_i32_at(line, 1);
        let pos = left.iter().position(|c| c.product == pid);
        let (unit, amount) = if let Some(pos) = pos {
            let c = left.remove(pos);
            let unit = if c.qty > 0 { dec::text(dec::div(c.cost, c.qty).unwrap_or(0), 6) } else { "0.000000".into() };
            (unit, money(c.cost))
        } else {
            ("0.000000".into(), "0.00".into())
        };
        invoice_read::exe(
            db,
            &format!("UPDATE production_order_lines SET unit_cost = {}, amount = {} WHERE id = {id}", lit(&unit), lit(&amount)),
        )
        .await?;
    }
    Ok(())
}

async fn rewrite_material(db: &Db, company: i32, rules: &[Rule], books: &Books, warnings: &mut Vec<String>) -> Result<(), ApiError> {
    for doc in &books.uses {
        let lines = invoice_read::qry(
            db,
            &format!(
                "SELECT id, product_id, account_id, inventory_account_id, quantity FROM material_issue_lines WHERE doc_id = {} ORDER BY line_no, id",
                doc.id
            ),
        )
        .await?;
        let mut left: Vec<CostRow> = books.spent.iter().filter(|c| c.doc == doc.id).cloned().collect();
        let mut sides = Vec::new();
        let mut updates = Vec::new();
        let mut skip = false;
        for line in &lines {
            let id = cell_i32_at(line, 0);
            let pid = cell_i32_at(line, 1);
            let debit = cell_i32_at(line, 2);
            let credit = cell_i32_at(line, 3);
            if stock_rules::pair(rules, "material", credit, debit).is_none() {
                warnings.push(format!("няма правило за материал по документ {}", doc.number));
                skip = true;
                break;
            }
            let pos = left.iter().position(|c| c.product == pid);
            let cost = if let Some(pos) = pos { left.remove(pos).cost } else { 0 };
            let q = dec::or0(&cell_str(line, 4));
            let unit = if q > 0 { dec::text(dec::div(cost, q).unwrap_or(0), 6) } else { "0.000000".into() };
            updates.push((id, unit, money(cost)));
            if cost != 0 {
                sides.push(Side { account: posting::by_id(db, debit).await?, direction: "debit", amount: money(cost) });
                sides.push(Side { account: posting::by_id(db, credit).await?, direction: "credit", amount: money(cost) });
            }
        }
        if skip {
            continue;
        }
        let entry = write_fresh(db, company, "material_issue", doc.id, &doc.number, &doc.date, &format!("Изписване {}", doc.number), &sides).await?;
        for (id, unit, amount) in updates {
            invoice_read::exe(
                db,
                &format!("UPDATE material_issue_lines SET unit_cost = {}, amount = {} WHERE id = {id}", lit(&unit), lit(&amount)),
            )
            .await?;
        }
        invoice_read::exe(
            db,
            &format!("UPDATE material_issues SET journal_entry_id = {entry} WHERE id = {}", doc.id),
        )
        .await?;
    }
    Ok(())
}

async fn write_fresh(db: &Db, company: i32, doc_type: &str, doc_id: i32, number: &str, date: &str, desc: &str, sides: &[Side]) -> Result<i32, ApiError> {
    let old = invoice_read::qry(
        db,
        &format!(
            "SELECT id FROM journal_entries WHERE company_id = {company} AND document_type = {} AND document_id = {doc_id}",
            lit(doc_type)
        ),
    )
    .await?;
    for row in &old {
        invoice_read::wipe_journal(db, cell_i32_at(row, 0)).await?;
    }
    if sides.is_empty() {
        return Ok(0);
    }
    posting::write_entry(db, company, date, doc_type, doc_id, number, desc, 0, "", &posting::month_of(date), sides).await
}
