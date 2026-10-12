//! Наличност по обект. Количеството и стойността се сглобяват
//! от документите в stock_replay, не се пипат тук.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{cell_i32_at, cell_str};
use crate::dec::{self, qty};
use crate::form::{active_or_one, auth_sub};
use crate::invoice_read;
use crate::{AppState, ApiError};

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
