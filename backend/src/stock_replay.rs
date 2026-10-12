//! Наличността се сглобява от документите по дата, не по реда на запис.
//! За един ден: начало, доставка, трансфер, производство, ръчно изписване, продажба.

use std::collections::{HashMap, HashSet};

use crate::db::{cell_i32_at, cell_str, lit, Db};
use crate::dec::{self, money, qty};
use crate::invoice_read;
use crate::ApiError;

#[derive(Clone)]
pub struct SaleHit {
    pub product_id: i32,
    pub product_name: String,
    pub date: String,
    pub cost: i128,
    pub inventory_account_id: i32,
    pub revenue_account_id: i32,
}

pub struct Flow {
    pub date: String,
    pub doc: String,
    pub kind: String,
    pub reason: String,
    pub product_id: i32,
    pub product_name: String,
    pub location_id: i32,
    pub quantity: String,
    pub value: String,
}

pub struct Balance {
    pub product_id: i32,
    pub product_name: String,
    pub location_id: i32,
    pub quantity: String,
    pub value: String,
}

pub struct Out {
    pub warnings: Vec<String>,
    pub sales: Vec<SaleHit>,
    pub flows: Vec<Flow>,
    pub balances: Vec<Balance>,
}

pub(crate) struct Meta {
    pub(crate) name: String,
    pub(crate) is_inventory: i32,
    pub(crate) inventory: i32,
    pub(crate) revenue: i32,
}

struct Lot {
    loc: i32,
    rem: i128,
    unit: i128,
    date: String,
    inv: i32,
    line: i32,
}

struct State {
    qty: i128,
    val: i128,
    lots: Vec<Lot>,
}

pub(crate) struct ProdRow {
    pub(crate) id: i32,
    pub(crate) number: String,
    pub(crate) output_id: i32,
    pub(crate) date: String,
}

pub(crate) struct UseRow {
    pub(crate) id: i32,
    pub(crate) number: String,
    pub(crate) date: String,
}

#[derive(Clone)]
pub(crate) struct CostRow {
    pub(crate) doc: i32,
    pub(crate) product: i32,
    pub(crate) qty: i128,
    pub(crate) cost: i128,
}

enum Mv {
    Open { product: i32, qty: i128, value: i128 },
    In { product: i32, qty: i128, unit: i128, loc: i32, inv: i32, line: i32, date: String, doc: String },
    Out { product: i32, qty: i128, loc: i32, date: String, doc: String, reason: String, sale: bool, bucket: i32 },
    Move { product: i32, qty: i128, from: i32, to: i32, date: String, doc: String },
    Make { product: i32, qty: i128, loc: i32, date: String, doc: String, order: i32 },
}

struct Item {
    date: String,
    rank: u8,
    seq: i64,
    mv: Mv,
}

pub(crate) struct Books {
    pub(crate) prods: Vec<ProdRow>,
    pub(crate) uses: Vec<UseRow>,
    pub(crate) made: Vec<CostRow>,
    pub(crate) spent: Vec<CostRow>,
}

pub async fn rebuild(db: &Db, company: i32, until: &str) -> Result<Out, ApiError> {
    run(db, company, until, true).await
}

pub async fn explain(db: &Db, company: i32, until: &str) -> Result<Out, ApiError> {
    run(db, company, until, false).await
}

async fn run(db: &Db, company: i32, until: &str, write: bool) -> Result<Out, ApiError> {
    let products = load_products(db, company).await?;
    let (start_month, method) = company_setup(db, company).await?;
    let mut items = Vec::new();
    let mut dates = Vec::new();
    load_invoices(db, company, &products, &mut items, &mut dates).await?;
    load_production(db, company, &products, &mut items, &mut dates).await?;
    load_transfers(db, company, &products, &mut items, &mut dates).await?;
    load_issues(db, company, &products, &mut items, &mut dates).await?;
    let horizon = horizon_of(until, &dates);
    let (year, fy) = fy_start(&horizon, start_month);
    load_opening(db, year, &fy, &products, &mut items).await?;
    items.retain(|it| it.date.as_str() >= fy.as_str() && it.date.as_str() <= horizon.as_str());
    items.sort_by(|a, b| (&a.date, a.rank, a.seq).cmp(&(&b.date, b.rank, b.seq)));
    let (mut warnings, sales, flows, states, books) = apply(&items, &products, &method);
    let balances = balances_of(&states, &products);
    if write {
        persist(db, &products, &states).await?;
        crate::stock_journal::strip_legacy(db, company).await?;
        crate::stock_journal::rewrite(db, company, &products, &books, &mut warnings).await?;
    }
    Ok(Out { warnings, sales, flows, balances })
}

fn horizon_of(until: &str, dates: &[String]) -> String {
    let far = until.trim().is_empty() || until.starts_with("9999");
    if !far {
        return until.trim().to_string();
    }
    let mut h = chrono::Utc::now().format("%Y-%m-%d").to_string();
    for d in dates {
        if d.as_str() > h.as_str() {
            h = d.clone();
        }
    }
    h
}

fn fy_start(date: &str, start_month: i32) -> (i32, String) {
    let y: i32 = date.get(0..4).and_then(|s| s.parse().ok()).unwrap_or(2026);
    let m: i32 = date.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
    let sm = if (1..=12).contains(&start_month) { start_month } else { 1 };
    let year = if m >= sm { y } else { y - 1 };
    (year, format!("{year}-{sm:02}-01"))
}

fn apply(
    items: &[Item],
    products: &HashMap<i32, Meta>,
    method: &str,
) -> (Vec<String>, Vec<SaleHit>, Vec<Flow>, HashMap<i32, State>, Books) {
    let mut warnings = Vec::new();
    let mut sales = Vec::new();
    let mut flows = Vec::new();
    let mut states: HashMap<i32, State> = HashMap::new();
    let mut order_cost: HashMap<i32, i128> = HashMap::new();
    let mut made = Vec::new();
    let mut spent = Vec::new();
    for it in items {
        match &it.mv {
            Mv::Open { product, qty, value } => {
                let st = states.entry(*product).or_default();
                st.qty = dec::add(st.qty, *qty).unwrap_or(st.qty);
                st.val = dec::add(st.val, *value).unwrap_or(st.val);
                st.lots.push(Lot { loc: 0, rem: *qty, unit: unit_of(*value, *qty), date: it.date.clone(), inv: 0, line: 0 });
                push_flow(&mut flows, products, &it.date, "начало", "in", "opening", *product, 0, *qty, *value);
            }
            Mv::In { product, qty, unit, loc, inv, line, date, doc } => {
                let value = dec::mul(*qty, *unit).unwrap_or(0);
                receive(states.entry(*product).or_default(), *qty, *unit, *loc, date, *inv, *line);
                push_flow(&mut flows, products, date, doc, "in", "purchase", *product, *loc, *qty, value);
            }
            Mv::Out { product, qty, loc, date, doc, reason, sale, bucket } => {
                let name = products.get(product).map(|m| m.name.clone()).unwrap_or_default();
                let (cost, miss) = issue(states.entry(*product).or_default(), *qty, *loc, method);
                if miss > 0 {
                    warnings.push(format!("{name}: липсват {}", dec::qty(miss)));
                }
                let covered = qty.saturating_sub(miss);
                push_flow(&mut flows, products, date, doc, "out", reason, *product, *loc, covered, cost);
                if *reason == "production" {
                    *order_cost.entry(*bucket).or_insert(0) += cost;
                    made.push(CostRow { doc: *bucket, product: *product, qty: covered, cost });
                }
                if *reason == "material" {
                    spent.push(CostRow { doc: *bucket, product: *product, qty: covered, cost });
                }
                if *sale {
                    let meta = products.get(product);
                    sales.push(SaleHit {
                        product_id: *product,
                        product_name: name,
                        date: date.clone(),
                        cost,
                        inventory_account_id: meta.map(|m| m.inventory).unwrap_or(0),
                        revenue_account_id: meta.map(|m| m.revenue).unwrap_or(0),
                    });
                }
            }
            Mv::Move { product, qty, from, to, date, doc } => {
                let name = products.get(product).map(|m| m.name.clone()).unwrap_or_default();
                let (moved, value, miss) = transfer(states.entry(*product).or_default(), *qty, *from, *to, date);
                if miss > 0 {
                    warnings.push(format!("{name}: липсват {} за трансфер", dec::qty(miss)));
                }
                push_flow(&mut flows, products, date, doc, "out", "transfer", *product, *from, moved, value);
                push_flow(&mut flows, products, date, doc, "in", "transfer", *product, *to, moved, value);
            }
            Mv::Make { product, qty, loc, date, doc, order } => {
                let total = order_cost.get(order).copied().unwrap_or(0);
                let unit = if *qty > 0 { dec::div(total, *qty).unwrap_or(0) } else { 0 };
                receive(states.entry(*product).or_default(), *qty, unit, *loc, date, 0, 0);
                push_flow(&mut flows, products, date, doc, "in", "production", *product, *loc, *qty, total);
            }
        }
    }
    let books = books_with(Books { prods: Vec::new(), uses: Vec::new(), made, spent }, items);
    (warnings, sales, flows, states, books)
}

fn books_with(mut books: Books, items: &[Item]) -> Books {
    let mut seen_p = HashSet::new();
    let mut seen_u = HashSet::new();
    for it in items {
        match &it.mv {
            Mv::Make { order, product, date, doc, .. } if seen_p.insert(*order) => {
                books.prods.push(ProdRow {
                    id: *order,
                    number: doc.clone(),
                    output_id: *product,
                    date: date.clone(),
                });
            }
            Mv::Out { bucket, date, doc, reason, .. } if reason == "material" && seen_u.insert(*bucket) => {
                books.uses.push(UseRow { id: *bucket, number: doc.clone(), date: date.clone() });
            }
            _ => {}
        }
    }
    books
}

fn receive(st: &mut State, q: i128, unit: i128, loc: i32, date: &str, inv: i32, line: i32) {
    if q <= 0 {
        return;
    }
    let add = dec::mul(q, unit).unwrap_or(0);
    st.qty = dec::add(st.qty, q).unwrap_or(st.qty);
    st.val = dec::add(st.val, add).unwrap_or(st.val);
    st.lots.push(Lot { loc, rem: q, unit, date: date.to_string(), inv, line });
}

fn issue(st: &mut State, q: i128, loc: i32, method: &str) -> (i128, i128) {
    if q <= 0 {
        return (0, 0);
    }
    if method == "FIFO" || method == "LIFO" {
        return issue_lots(st, q, loc, method == "LIFO");
    }
    let covered = q.min(st.qty.max(0));
    let miss = q - covered;
    if covered <= 0 {
        return (0, miss);
    }
    let unit = if st.qty > 0 { dec::div(st.val, st.qty).unwrap_or(0) } else { 0 };
    let mut cost = dec::mul(covered, unit).unwrap_or(0);
    if st.qty - covered <= 0 {
        cost = st.val;
        st.qty = 0;
        st.val = 0;
    } else {
        st.qty -= covered;
        st.val = dec::sub(st.val, cost).unwrap_or(0);
    }
    consume(st, loc, covered);
    (cost, miss)
}

fn issue_lots(st: &mut State, q: i128, loc: i32, lifo: bool) -> (i128, i128) {
    let mut idx: Vec<usize> = st.lots.iter().enumerate().filter(|(_, l)| l.loc == loc && l.rem > 0).map(|(i, _)| i).collect();
    if lifo {
        idx.reverse();
    }
    let mut need = q;
    let mut cost = 0i128;
    for i in idx {
        if need <= 0 {
            break;
        }
        let take = st.lots[i].rem.min(need);
        cost = dec::add(cost, dec::mul(take, st.lots[i].unit).unwrap_or(0)).unwrap_or(cost);
        st.lots[i].rem -= take;
        need -= take;
    }
    let covered = q - need;
    if covered > 0 {
        if st.qty - covered <= 0 {
            cost = st.val;
            st.qty = 0;
            st.val = 0;
        } else {
            st.qty -= covered;
            st.val = dec::sub(st.val, cost).unwrap_or(0);
        }
    }
    (cost, need)
}

fn consume(st: &mut State, loc: i32, mut need: i128) {
    for pass in 0..2 {
        for lot in &mut st.lots {
            if need <= 0 {
                break;
            }
            let here = if pass == 0 { lot.loc == loc } else { lot.loc != loc };
            if !here || lot.rem <= 0 {
                continue;
            }
            let take = lot.rem.min(need);
            lot.rem -= take;
            need -= take;
        }
    }
}

fn transfer(st: &mut State, q: i128, from: i32, to: i32, date: &str) -> (i128, i128, i128) {
    let mut need = q;
    let mut value = 0i128;
    let mut born = Vec::new();
    for lot in &mut st.lots {
        if need <= 0 {
            break;
        }
        if lot.loc != from || lot.rem <= 0 {
            continue;
        }
        let take = lot.rem.min(need);
        value = dec::add(value, dec::mul(take, lot.unit).unwrap_or(0)).unwrap_or(value);
        lot.rem -= take;
        need -= take;
        born.push(Lot { loc: to, rem: take, unit: lot.unit, date: date.to_string(), inv: lot.inv, line: lot.line });
    }
    st.lots.extend(born);
    (q - need, value, need)
}

fn unit_of(value: i128, q: i128) -> i128 {
    if q > 0 { dec::div(value, q).unwrap_or(0) } else { 0 }
}

fn push_flow(flows: &mut Vec<Flow>, products: &HashMap<i32, Meta>, date: &str, doc: &str, kind: &str, reason: &str, product: i32, loc: i32, q: i128, value: i128) {
    if q <= 0 && value <= 0 {
        return;
    }
    flows.push(Flow {
        date: date.to_string(),
        doc: doc.to_string(),
        kind: kind.to_string(),
        reason: reason.to_string(),
        product_id: product,
        product_name: products.get(&product).map(|m| m.name.clone()).unwrap_or_default(),
        location_id: loc,
        quantity: qty(q),
        value: money(value),
    });
}

fn balances_of(states: &HashMap<i32, State>, products: &HashMap<i32, Meta>) -> Vec<Balance> {
    let mut out = Vec::new();
    for (pid, st) in states {
        if st.qty <= 0 && st.lots.iter().all(|l| l.rem <= 0) {
            continue;
        }
        let name = products.get(pid).map(|m| m.name.clone()).unwrap_or_default();
        let mut by: HashMap<i32, i128> = HashMap::new();
        for lot in &st.lots {
            if lot.rem > 0 {
                *by.entry(lot.loc).or_insert(0) += lot.rem;
            }
        }
        if by.is_empty() && st.qty > 0 {
            by.insert(0, st.qty);
        }
        let sum: i128 = by.values().copied().sum();
        let mut left = st.val;
        let mut n = 0;
        let total = by.len();
        for (loc, q) in &by {
            n += 1;
            let val = if n == total {
                left
            } else if sum > 0 {
                let v = dec::div(dec::mul(st.val, *q).unwrap_or(0), sum).unwrap_or(0);
                left = dec::sub(left, v).unwrap_or(0);
                v
            } else {
                0
            };
            out.push(Balance {
                product_id: *pid,
                product_name: name.clone(),
                location_id: *loc,
                quantity: qty(*q),
                value: money(val),
            });
        }
    }
    out.sort_by_key(|b| (b.product_name.clone(), b.location_id));
    out
}

async fn persist(db: &Db, products: &HashMap<i32, Meta>, states: &HashMap<i32, State>) -> Result<(), ApiError> {
    for (id, meta) in products {
        if meta.is_inventory != 1 {
            continue;
        }
        invoice_read::exe(db, &format!("DELETE FROM inventory_lots WHERE product_id = {id}")).await?;
        let st = states.get(id);
        let (q, val) = st.map(|s| (s.qty, s.val)).unwrap_or((0, 0));
        let cost = if q > 0 { dec::text(dec::div(val, q).unwrap_or(0), 6) } else { "0.000000".into() };
        invoice_read::exe(
            db,
            &format!(
                "UPDATE products SET stock_quantity = {}, stock_value = {}, cost_price = {} WHERE id = {id}",
                lit(&qty(q)),
                lit(&money(val)),
                lit(&cost)
            ),
        )
        .await?;
        if let Some(st) = st {
            for lot in &st.lots {
                if lot.rem <= 0 {
                    continue;
                }
                let lid = invoice_read::nid(db, "inventory_lots").await?;
                invoice_read::exe(
                    db,
                    &format!(
                        "INSERT INTO inventory_lots (id, product_id, invoice_id, invoice_line_id, received_date, quantity, quantity_remaining, unit_cost, location_id) \
                         VALUES ({lid}, {id}, {}, {}, {}, {}, {}, {}, {})",
                        lot.inv,
                        lot.line,
                        lit(&lot.date),
                        lit(&qty(lot.rem)),
                        lit(&qty(lot.rem)),
                        lit(&dec::text(lot.unit, 6)),
                        lot.loc
                    ),
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn load_products(db: &Db, company: i32) -> Result<HashMap<i32, Meta>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT id, name, is_inventory, inventory_account_id, revenue_account_id FROM products WHERE company_id = {company}"
        ),
    )
    .await?;
    let mut map = HashMap::new();
    for r in &rows {
        map.insert(
            cell_i32_at(r, 0),
            Meta {
                name: cell_str(r, 1),
                is_inventory: cell_i32_at(r, 2),
                inventory: cell_i32_at(r, 3),
                revenue: cell_i32_at(r, 4),
            },
        );
    }
    Ok(map)
}

async fn company_setup(db: &Db, company: i32) -> Result<(i32, String), ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT fiscal_year_start_month, inventory_valuation_method FROM companies WHERE id = {company}"),
    )
    .await?;
    let Some(row) = rows.first() else { return Ok((1, "WAC".into())) };
    let m = cell_str(row, 1);
    let method = if m == "FIFO" || m == "LIFO" { m } else { "WAC".into() };
    Ok((cell_i32_at(row, 0), method))
}

fn keep(products: &HashMap<i32, Meta>, id: i32) -> bool {
    products.get(&id).map(|m| m.is_inventory == 1).unwrap_or(false)
}

fn push_item(items: &mut Vec<Item>, dates: &mut Vec<String>, date: &str, rank: u8, seq: i64, mv: Mv) {
    if date.len() < 10 {
        return;
    }
    dates.push(date.to_string());
    items.push(Item { date: date.to_string(), rank, seq, mv });
}

async fn load_opening(db: &Db, year: i32, fy: &str, products: &HashMap<i32, Meta>, items: &mut Vec<Item>) -> Result<(), ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT product_id, quantity, debit FROM opening_balances WHERE fiscal_year = {year} AND product_id > 0"),
    )
    .await?;
    for (i, r) in rows.iter().enumerate() {
        let pid = cell_i32_at(r, 0);
        if !keep(products, pid) {
            continue;
        }
        let q = dec::or0(&cell_str(r, 1));
        if q <= 0 || fy.len() < 10 {
            continue;
        }
        let value = dec::or0(&cell_str(r, 2));
        items.push(Item {
            date: fy.to_string(),
            rank: 0,
            seq: i as i64,
            mv: Mv::Open { product: pid, qty: q, value },
        });
    }
    Ok(())
}

async fn load_invoices(db: &Db, company: i32, products: &HashMap<i32, Meta>, items: &mut Vec<Item>, dates: &mut Vec<String>) -> Result<(), ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT i.id, i.direction, i.issue_date, i.number, e.location_id, l.id, x.product_id, l.quantity, l.net_amount \
             FROM invoices i LEFT JOIN invoice_extra e ON i.id = e.id \
             INNER JOIN invoice_lines l ON l.invoice_id = i.id \
             LEFT JOIN invoice_line_extra x ON x.id = l.id \
             WHERE i.company_id = {company} AND i.status = 'posted' AND (i.document_type = '01' OR i.document_type = '11') \
             ORDER BY i.id, l.id"
        ),
    )
    .await?;
    for (i, r) in rows.iter().enumerate() {
        let pid = cell_i32_at(r, 6);
        if !keep(products, pid) {
            continue;
        }
        let date = cell_str(r, 2);
        let q = dec::or0(&cell_str(r, 7));
        if q <= 0 {
            continue;
        }
        let inv = cell_i32_at(r, 0);
        let doc = cell_str(r, 3);
        let loc = cell_i32_at(r, 4);
        let line = cell_i32_at(r, 5);
        let dir = cell_str(r, 1);
        let seq = inv as i64 * 1000 + i as i64;
        if dir == "in" {
            let unit = unit_of(dec::or0(&cell_str(r, 8)), q);
            push_item(items, dates, &date, 1, seq, Mv::In { product: pid, qty: q, unit, loc, inv, line, date: date.clone(), doc });
        } else if dir == "out" {
            push_item(
                items,
                dates,
                &date,
                6,
                seq,
                Mv::Out { product: pid, qty: q, loc, date: date.clone(), doc, reason: "sale".into(), sale: true, bucket: inv },
            );
        }
    }
    Ok(())
}

async fn load_production(db: &Db, company: i32, products: &HashMap<i32, Meta>, items: &mut Vec<Item>, dates: &mut Vec<String>) -> Result<Vec<ProdRow>, ApiError> {
    let orders = invoice_read::qry(
        db,
        &format!(
            "SELECT id, number, output_product_id, quantity, location_id, order_date FROM production_orders \
             WHERE company_id = {company} AND status = 'completed' ORDER BY id"
        ),
    )
    .await?;
    let mut meta = Vec::new();
    for r in &orders {
        let id = cell_i32_at(r, 0);
        let number = cell_str(r, 1);
        let output = cell_i32_at(r, 2);
        let q = dec::or0(&cell_str(r, 3));
        let loc = cell_i32_at(r, 4);
        let date = cell_str(r, 5);
        let lines = invoice_read::qry(
            db,
            &format!("SELECT product_id, quantity FROM production_order_lines WHERE order_id = {id} ORDER BY id"),
        )
        .await?;
        for (i, line) in lines.iter().enumerate() {
            let pid = cell_i32_at(line, 0);
            let lq = dec::or0(&cell_str(line, 1));
            if !keep(products, pid) || lq <= 0 {
                continue;
            }
            push_item(
                items,
                dates,
                &date,
                3,
                id as i64 * 1000 + i as i64,
                Mv::Out { product: pid, qty: lq, loc, date: date.clone(), doc: number.clone(), reason: "production".into(), sale: false, bucket: id },
            );
        }
        if keep(products, output) && q > 0 {
            push_item(
                items,
                dates,
                &date,
                4,
                id as i64 * 1000 + 999,
                Mv::Make { product: output, qty: q, loc, date: date.clone(), doc: number.clone(), order: id },
            );
            meta.push(ProdRow { id, number, output_id: output, date });
        }
    }
    Ok(meta)
}

async fn load_transfers(db: &Db, company: i32, products: &HashMap<i32, Meta>, items: &mut Vec<Item>, dates: &mut Vec<String>) -> Result<(), ApiError> {
    let docs = invoice_read::qry(
        db,
        &format!(
            "SELECT id, number, doc_date, from_location_id, to_location_id FROM internal_docs \
             WHERE company_id = {company} AND status = 'confirmed' ORDER BY id"
        ),
    )
    .await?;
    for r in &docs {
        let id = cell_i32_at(r, 0);
        let number = cell_str(r, 1);
        let date = cell_str(r, 2);
        let from = cell_i32_at(r, 3);
        let to = cell_i32_at(r, 4);
        let lines = invoice_read::qry(
            db,
            &format!("SELECT product_id, quantity, line_kind FROM internal_doc_lines WHERE doc_id = {id} ORDER BY id"),
        )
        .await?;
        for (i, line) in lines.iter().enumerate() {
            if cell_str(line, 2) != "product" {
                continue;
            }
            let pid = cell_i32_at(line, 0);
            let q = dec::or0(&cell_str(line, 1));
            if !keep(products, pid) || q <= 0 {
                continue;
            }
            push_item(
                items,
                dates,
                &date,
                2,
                id as i64 * 1000 + i as i64,
                Mv::Move { product: pid, qty: q, from, to, date: date.clone(), doc: number.clone() },
            );
        }
    }
    Ok(())
}

async fn load_issues(db: &Db, company: i32, products: &HashMap<i32, Meta>, items: &mut Vec<Item>, dates: &mut Vec<String>) -> Result<Vec<UseRow>, ApiError> {
    let docs = invoice_read::qry(
        db,
        &format!(
            "SELECT id, number, doc_date, location_id FROM material_issues \
             WHERE company_id = {company} AND status = 'confirmed' ORDER BY id"
        ),
    )
    .await?;
    let mut meta = Vec::new();
    for r in &docs {
        let id = cell_i32_at(r, 0);
        let number = cell_str(r, 1);
        let date = cell_str(r, 2);
        let loc = cell_i32_at(r, 3);
        let lines = invoice_read::qry(
            db,
            &format!("SELECT product_id, quantity FROM material_issue_lines WHERE doc_id = {id} ORDER BY line_no, id"),
        )
        .await?;
        let mut any = false;
        for (i, line) in lines.iter().enumerate() {
            let pid = cell_i32_at(line, 0);
            let q = dec::or0(&cell_str(line, 1));
            if !keep(products, pid) || q <= 0 {
                continue;
            }
            any = true;
            push_item(
                items,
                dates,
                &date,
                5,
                id as i64 * 1000 + i as i64,
                Mv::Out { product: pid, qty: q, loc, date: date.clone(), doc: number.clone(), reason: "material".into(), sale: false, bucket: id },
            );
        }
        if any {
            meta.push(UseRow { id, number, date });
        }
    }
    Ok(meta)
}

impl Default for State {
    fn default() -> Self {
        Self { qty: 0, val: 0, lots: Vec::new() }
    }
}
