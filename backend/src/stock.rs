//! Заприхождаване и изписване: WAC, FIFO, LIFO.
//! Изходяща фактура 01/11: Дт приход / Кт склад по себестойност.
//! Връщането при отпост е като в bagabuch: входящата партида се маха само
//! ако е непокътната; изходящото количество се връща по средна цена без нова партида.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str, lit, Db};
use crate::dec::{self, money, qty};
use crate::form::{active_or_one, auth_sub};
use crate::invoice_read::{self, Inv};
use crate::posting::{self, Side};
use crate::{api_err, AppState, ApiError};

pub struct Snap {
    product_id: i32,
    qty: String,
    val: String,
    cost: String,
    lots: Vec<(i32, String)>,
    created: Vec<i32>,
}

impl Snap {
    fn empty() -> Self {
        Self { product_id: 0, qty: String::new(), val: String::new(), cost: String::new(), lots: Vec::new(), created: Vec::new() }
    }
}

struct Prod {
    name: String,
    is_inventory: i32,
    qty: i128,
    val: i128,
    cost_price: String,
    inventory_account_id: i32,
    revenue_account_id: i32,
}

struct Lot {
    id: i32,
    location_id: i32,
    remaining: i128,
    unit_cost: i128,
}

#[derive(Deserialize, Default)]
pub(crate) struct LocQ {
    location_id: Option<String>,
}

pub async fn at_location(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LocQ>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company = active_or_one(&st, &sub).await?;
    let location_id = q.location_id.unwrap_or_default().trim().parse::<i32>().unwrap_or(0);
    let products = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, name, code, unit, cost_price FROM products WHERE company_id = {company} AND is_inventory = 1 ORDER BY id"
        ),
    )
    .await?;
    let lots = invoice_read::qry(
        &st.db,
        "SELECT product_id, location_id, quantity_remaining FROM inventory_lots ORDER BY id",
    )
    .await?;
    let mut items = Vec::new();
    for p in &products {
        let pid = cell_i32_at(p, 0);
        let mut sum = 0i128;
        for lot in &lots {
            if cell_i32_at(lot, 0) == pid && cell_i32_at(lot, 1) == location_id {
                sum = dec::add(sum, dec::or0(&cell_str(lot, 2))).unwrap_or(sum);
            }
        }
        if sum > 0 {
            items.push(json!({
                "product_id": pid,
                "name": cell_str(p, 1),
                "code": cell_str(p, 2),
                "unit": cell_str(p, 3),
                "quantity": qty(sum),
                "unit_cost": cell_str(p, 4),
            }));
        }
    }
    let count = items.len();
    Ok(Json(json!({ "items": items, "count": count, "location_id": location_id })))
}

pub async fn valuation(db: &Db, company: i32) -> Result<String, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT inventory_valuation_method FROM companies WHERE id = {company}"),
    )
    .await?;
    let m = rows.first().map(|r| cell_str(r, 0)).unwrap_or_default();
    Ok(if m == "FIFO" || m == "LIFO" { m } else { "WAC".into() })
}

pub async fn apply_invoice(db: &Db, inv: &Inv) -> Result<(Vec<Side>, Vec<Snap>), ApiError> {
    if inv.document_type != "01" && inv.document_type != "11" {
        return Ok((Vec::new(), Vec::new()));
    }
    let method = valuation(db, inv.company_id).await?;
    let lines = invoice_read::lines_of(db, inv.id).await?;
    let mut sides = Vec::new();
    let mut snaps = Vec::new();
    for line in &lines {
        if line.product_id <= 0 {
            continue;
        }
        let prod = match load_prod(db, line.product_id).await? {
            Some(p) if p.is_inventory == 1 => p,
            _ => continue,
        };
        let step = if inv.direction == "out" {
            issue(db, line.product_id, &line.quantity, &method, &prod.name, inv.location_id).await
        } else {
            receive(db, line.product_id, inv.id, line.id, &inv.issue_date, &line.quantity, &line.unit_price, inv.location_id).await
                .map(|(_, snap)| ("0.00".into(), snap))
        };
        match step {
            Ok((cost, snap)) => {
                if inv.direction == "out" && !zero_money(&cost) {
                    match cogs(db, inv, &prod, &cost).await {
                        Ok(pair) => sides.extend(pair),
                        Err(e) => {
                            snaps.push(snap);
                            let _ = undo(db, &snaps).await;
                            return Err(e);
                        }
                    }
                }
                snaps.push(snap);
            }
            Err(e) => {
                let _ = undo(db, &snaps).await;
                return Err(e);
            }
        }
    }
    Ok((sides, snaps))
}

async fn cogs(db: &Db, inv: &Inv, prod: &Prod, cost: &str) -> Result<Vec<Side>, ApiError> {
    let revenue = posting::prefer(db, inv.company_id, prod.revenue_account_id, &["702"], "липсва приходна сметка (702)").await?;
    let inventory = posting::prefer(db, inv.company_id, prod.inventory_account_id, &["304"], "липсва складова сметка (304)").await?;
    Ok(vec![
        Side { account: revenue, direction: "debit", amount: cost.to_string() },
        Side { account: inventory, direction: "credit", amount: cost.to_string() },
    ])
}

pub async fn restore_posted(db: &Db, inv: &Inv) -> Result<(), ApiError> {
    if inv.document_type != "01" && inv.document_type != "11" {
        return Ok(());
    }
    let lines = invoice_read::lines_of(db, inv.id).await?;
    if inv.direction == "in" {
        for line in &lines {
            if line.product_id <= 0 {
                continue;
            }
            let Some(prod) = load_prod(db, line.product_id).await? else { continue };
            if prod.is_inventory != 1 {
                continue;
            }
            intact_lot(db, line.id).await?;
        }
    }
    for line in &lines {
        if line.product_id <= 0 {
            continue;
        }
        let Some(prod) = load_prod(db, line.product_id).await? else { continue };
        if prod.is_inventory != 1 {
            continue;
        }
        if inv.direction == "in" {
            restore_in(db, line.id, line.product_id).await?;
        } else {
            restore_out(db, line.product_id, &line.quantity).await?;
        }
    }
    Ok(())
}

async fn intact_lot(db: &Db, line_id: i32) -> Result<(), ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT quantity, quantity_remaining FROM inventory_lots WHERE invoice_line_id = {line_id} ORDER BY id"),
    )
    .await?;
    let Some(row) = rows.first() else { return Ok(()) };
    if dec::or0(&cell_str(row, 0)) != dec::or0(&cell_str(row, 1)) {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("партида за ред {line_id} вече е изписана — наличността не се връща автоматично"),
        ));
    }
    Ok(())
}

async fn restore_in(db: &Db, line_id: i32, product_id: i32) -> Result<(), ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!("SELECT id, quantity, quantity_remaining, unit_cost FROM inventory_lots WHERE invoice_line_id = {line_id} ORDER BY id"),
    )
    .await?;
    let Some(row) = rows.first() else { return Ok(()) };
    let quantity = dec::or0(&cell_str(row, 1));
    let remaining = dec::or0(&cell_str(row, 2));
    if quantity != remaining {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("партида за ред {line_id} вече е изписана — наличността не се връща автоматично"),
        ));
    }
    let Some(prod) = load_prod(db, product_id).await? else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"));
    };
    let lot_val = dec::mul(quantity, dec::or0(&cell_str(row, 3))).unwrap_or(0);
    let nq = dec::sub(prod.qty, quantity).unwrap_or(0);
    let nv = dec::sub(prod.val, lot_val).unwrap_or(0);
    let avg = if nq > 0 { dec::div(nv, nq).unwrap_or(0) } else { 0 };
    let lot_id = cell_i32_at(row, 0);
    invoice_read::exe(db, &format!("DELETE FROM inventory_lots WHERE id = {lot_id}")).await?;
    set_stock(db, product_id, nq, nv, &dec::text(avg, 6)).await
}

async fn restore_out(db: &Db, product_id: i32, qty_s: &str) -> Result<(), ApiError> {
    let q = dec::or0(qty_s);
    if q <= 0 {
        return Ok(());
    }
    let Some(prod) = load_prod(db, product_id).await? else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"));
    };
    let uc = dec::or0(&prod.cost_price);
    let add = dec::mul(q, uc).unwrap_or(0);
    let nq = dec::add(prod.qty, q).unwrap_or(prod.qty);
    let nv = dec::add(prod.val, add).unwrap_or(prod.val);
    let avg = if nq > 0 { dec::div(nv, nq).unwrap_or(uc) } else { uc };
    set_stock(db, product_id, nq, nv, &dec::text(avg, 6)).await
}

pub async fn undo(db: &Db, snaps: &[Snap]) -> Result<(), ApiError> {
    for snap in snaps.iter().rev() {
        if snap.product_id <= 0 {
            continue;
        }
        for id in &snap.created {
            invoice_read::exe(db, &format!("DELETE FROM inventory_lots WHERE id = {id}")).await?;
        }
        for (id, rem) in &snap.lots {
            invoice_read::exe(
                db,
                &format!("UPDATE inventory_lots SET quantity_remaining = {} WHERE id = {id}", lit(rem)),
            )
            .await?;
        }
        invoice_read::exe(
            db,
            &format!(
                "UPDATE products SET stock_quantity = {}, stock_value = {}, cost_price = {} WHERE id = {}",
                lit(&snap.qty),
                lit(&snap.val),
                lit(&snap.cost),
                snap.product_id
            ),
        )
        .await?;
    }
    Ok(())
}

pub async fn receive(
    db: &Db,
    product_id: i32,
    invoice_id: i32,
    invoice_line_id: i32,
    received_date: &str,
    qty_s: &str,
    unit_cost_s: &str,
    location_id: i32,
) -> Result<(String, Snap), ApiError> {
    let Some(prod) = load_prod(db, product_id).await? else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"));
    };
    let q = dec::or0(qty_s);
    if prod.is_inventory == 0 || q <= 0 {
        return Ok(("0.00".into(), Snap::empty()));
    }
    let snap = capture(db, product_id).await?;
    let unit = dec::or0(unit_cost_s);
    let line = dec::mul(q, unit).ok_or_else(|| api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност"))?;
    let nq = dec::add(prod.qty, q).unwrap_or(prod.qty);
    let nv = dec::add(prod.val, line).unwrap_or(prod.val);
    let avg = if nq > 0 { dec::div(nv, nq).unwrap_or(unit) } else { unit };
    let mut snap = snap;
    let lot_id = match insert_lot(db, product_id, invoice_id, invoice_line_id, received_date, q, unit, location_id).await {
        Ok(id) => id,
        Err(e) => {
            let _ = undo(db, &[snap]).await;
            return Err(e);
        }
    };
    snap.created.push(lot_id);
    if let Err(e) = set_stock(db, product_id, nq, nv, &dec::text(avg, 6)).await {
        let _ = undo(db, &[snap]).await;
        return Err(e);
    }
    Ok((money(line), snap))
}

pub async fn issue(
    db: &Db,
    product_id: i32,
    qty_s: &str,
    method: &str,
    product_name: &str,
    location_id: i32,
) -> Result<(String, Snap), ApiError> {
    let Some(prod) = load_prod(db, product_id).await? else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"));
    };
    let q = dec::or0(qty_s);
    if prod.is_inventory == 0 || q <= 0 {
        return Ok(("0.00".into(), Snap::empty()));
    }
    if prod.qty < q {
        return Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("недостатъчна наличност за {product_name} (налично {}, нужно {qty_s})", qty(prod.qty)),
        ));
    }
    let snap = capture(db, product_id).await?;
    if method == "FIFO" || method == "LIFO" {
        return issue_lots(db, product_id, q, prod.qty, prod.val, method, location_id, snap).await;
    }
    let unit = if prod.qty > 0 { dec::div(prod.val, prod.qty).unwrap_or(0) } else { 0 };
    let mut cost = dec::mul(q, unit).unwrap_or(0);
    let nq = dec::sub(prod.qty, q).unwrap_or(0);
    let nv = if nq <= 0 {
        cost = prod.val;
        0
    } else {
        dec::sub(prod.val, cost).unwrap_or(0)
    };
    if let Err(e) = take_lots(db, product_id, location_id, q, 0, false, "").await {
        if location_id > 0 {
            let _ = undo(db, &[snap]).await;
            return Err(e);
        }
    }
    if let Err(e) = set_stock(db, product_id, nq, nv, &dec::text(unit, 6)).await {
        let _ = undo(db, &[snap]).await;
        return Err(e);
    }
    Ok((money(cost), snap))
}

async fn issue_lots(
    db: &Db,
    product_id: i32,
    q: i128,
    stock_qty: i128,
    stock_val: i128,
    method: &str,
    location_id: i32,
    snap: Snap,
) -> Result<(String, Snap), ApiError> {
    let mut lots = load_lots(db, product_id).await?;
    if method == "LIFO" {
        lots.reverse();
    }
    let mut need = q;
    let mut cost = 0i128;
    for lot in &lots {
        if need <= 0 {
            break;
        }
        if lot.location_id != location_id || lot.remaining <= 0 {
            continue;
        }
        let take = lot.remaining.min(need);
        cost = dec::add(cost, dec::mul(take, lot.unit_cost).unwrap_or(0)).unwrap_or(cost);
        let left = dec::sub(lot.remaining, take).unwrap_or(0);
        invoice_read::exe(
            db,
            &format!("UPDATE inventory_lots SET quantity_remaining = {} WHERE id = {}", lit(&qty(left)), lot.id),
        )
        .await?;
        need = dec::sub(need, take).unwrap_or(0);
    }
    if need > 0 {
        let _ = undo(db, &[snap]).await;
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "недостатъчни партиди за изписване"));
    }
    let nq = dec::sub(stock_qty, q).unwrap_or(0);
    let (cost, nv, avg) = if nq <= 0 {
        (stock_val, 0, 0)
    } else {
        let nv = dec::sub(stock_val, cost).unwrap_or(0);
        (cost, nv, dec::div(nv, nq).unwrap_or(0))
    };
    if let Err(e) = set_stock(db, product_id, nq, nv, &dec::text(avg, 6)).await {
        let _ = undo(db, &[snap]).await;
        return Err(e);
    }
    Ok((money(cost), snap))
}

pub async fn transfer(
    db: &Db,
    product_id: i32,
    qty_s: &str,
    from_id: i32,
    to_id: i32,
    moved_date: &str,
    product_name: &str,
) -> Result<Snap, ApiError> {
    if from_id == to_id {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "обектът-източник и обектът-приемник съвпадат"));
    }
    let Some(prod) = load_prod(db, product_id).await? else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "артикулът не е намерен"));
    };
    if prod.is_inventory == 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, format!("{product_name} не е складов артикул")));
    }
    let q = dec::or0(qty_s);
    if q <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "количеството трябва да е положително"));
    }
    let snap = capture(db, product_id).await?;
    match take_lots(db, product_id, from_id, q, to_id, true, moved_date).await {
        Ok(created) => {
            let mut snap = snap;
            snap.created.extend(created);
            Ok(snap)
        }
        Err(e) => {
            let _ = undo(db, &[snap]).await;
            Err(e)
        }
    }
}

async fn take_lots(
    db: &Db,
    product_id: i32,
    from_id: i32,
    q: i128,
    dest: i32,
    do_dest: bool,
    date: &str,
) -> Result<Vec<i32>, ApiError> {
    let lots = load_lots(db, product_id).await?;
    let at: i128 = lots.iter().filter(|l| l.location_id == from_id).map(|l| l.remaining).sum();
    if at < q {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "недостатъчна наличност в обекта"));
    }
    let mut need = q;
    let mut created = Vec::new();
    for lot in &lots {
        if need <= 0 {
            break;
        }
        if lot.location_id != from_id || lot.remaining <= 0 {
            continue;
        }
        let take = lot.remaining.min(need);
        let left = dec::sub(lot.remaining, take).unwrap_or(0);
        invoice_read::exe(
            db,
            &format!("UPDATE inventory_lots SET quantity_remaining = {} WHERE id = {}", lit(&qty(left)), lot.id),
        )
        .await?;
        if do_dest {
            let id = insert_lot(db, product_id, 0, 0, date, take, lot.unit_cost, dest).await?;
            created.push(id);
        }
        need = dec::sub(need, take).unwrap_or(0);
    }
    if need > 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "недостатъчна наличност в обекта"));
    }
    Ok(created)
}

async fn capture(db: &Db, product_id: i32) -> Result<Snap, ApiError> {
    let Some(prod) = load_prod(db, product_id).await? else {
        return Ok(Snap::empty());
    };
    let lots = load_lots(db, product_id).await?;
    Ok(Snap {
        product_id,
        qty: qty(prod.qty),
        val: money(prod.val),
        cost: prod.cost_price,
        lots: lots.into_iter().map(|l| (l.id, qty(l.remaining))).collect(),
        created: Vec::new(),
    })
}

async fn load_prod(db: &Db, id: i32) -> Result<Option<Prod>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT name, is_inventory, stock_quantity, stock_value, cost_price, inventory_account_id, revenue_account_id FROM products WHERE id = {id}"
        ),
    )
    .await?;
    let Some(row) = rows.into_iter().next() else { return Ok(None) };
    Ok(Some(Prod {
        name: cell_str(&row, 0),
        is_inventory: cell_i32_at(&row, 1),
        qty: dec::or0(&cell_str(&row, 2)),
        val: dec::or0(&cell_str(&row, 3)),
        cost_price: cell_str(&row, 4),
        inventory_account_id: cell_i32_at(&row, 5),
        revenue_account_id: cell_i32_at(&row, 6),
    }))
}

async fn load_lots(db: &Db, product_id: i32) -> Result<Vec<Lot>, ApiError> {
    let rows = invoice_read::qry(
        db,
        &format!(
            "SELECT id, location_id, quantity_remaining, unit_cost FROM inventory_lots WHERE product_id = {product_id} ORDER BY id"
        ),
    )
    .await?;
    Ok(rows
        .iter()
        .map(|r| Lot {
            id: cell_i32_at(r, 0),
            location_id: cell_i32_at(r, 1),
            remaining: dec::or0(&cell_str(r, 2)),
            unit_cost: dec::or0(&cell_str(r, 3)),
        })
        .collect())
}

async fn insert_lot(
    db: &Db,
    product_id: i32,
    invoice_id: i32,
    invoice_line_id: i32,
    date: &str,
    q: i128,
    unit: i128,
    location_id: i32,
) -> Result<i32, ApiError> {
    let id = invoice_read::nid(db, "inventory_lots").await?;
    let q = qty(q);
    invoice_read::exe(
        db,
        &format!(
            "INSERT INTO inventory_lots (id, product_id, invoice_id, invoice_line_id, received_date, quantity, quantity_remaining, unit_cost, location_id) \
             VALUES ({id}, {product_id}, {invoice_id}, {invoice_line_id}, {}, {}, {}, {}, {location_id})",
            lit(date),
            lit(&q),
            lit(&q),
            lit(&dec::text(unit, 6))
        ),
    )
    .await?;
    Ok(id)
}

async fn set_stock(db: &Db, product_id: i32, q: i128, val: i128, cost: &str) -> Result<(), ApiError> {
    invoice_read::exe(
        db,
        &format!(
            "UPDATE products SET stock_quantity = {}, stock_value = {}, cost_price = {} WHERE id = {product_id}",
            lit(&qty(q)),
            lit(&money(val)),
            lit(cost)
        ),
    )
    .await
}

fn zero_money(s: &str) -> bool {
    matches!(s, "0" | "0.0" | "0.00" | "0.000" | "0.0000")
}
