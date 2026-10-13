//! Рецепти за производство. Редовете се подменят цели при запис.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Map, Value};

use crate::db::{cell_i32_at, cell_str, lit};
use crate::dec::{self, qty};
use crate::form::{self, active_or_one, auth_sub};
use crate::invoice_read;
use crate::pagination::{paginate, PageQuery};
use crate::{api_err, AppState, ApiError};

const SEL: &str = "SELECT id, code, name, output_product_id, output_quantity, output_unit, notes, is_active, company_id FROM recipes";

pub async fn index(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let rows = invoice_read::qry(&st.db, &format!("{SEL} WHERE company_id = {company} ORDER BY id")).await?;
    let items: Vec<Value> = rows.iter().map(head).collect();
    Ok(paginate(items, &q))
}

pub async fn show(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    Ok(Json(load(&st, id, company).await?))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let company = company(&st, &headers).await?;
    let obj = form::object(&body)?;
    let fields = fields_of(obj, None)?;
    let lines = lines_of(obj, true)?;
    form::ensure_ref(&st.db, "products", fields.output_id, company, "артикулът").await?;
    for line in &lines {
        form::ensure_ref(&st.db, "products", line.product_id, company, "артикулът").await?;
    }
    let id = invoice_read::nid(&st.db, "recipes").await?;
    invoice_read::exe(&st.db, &insert_sql(id, company, &fields)).await?;
    if let Err(e) = write_lines(&st, id, &lines).await {
        let _ = wipe(&st, id).await;
        return Err(e);
    }
    Ok((StatusCode::CREATED, Json(load(&st, id, company).await?)))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let company = company(&st, &headers).await?;
    let cur = load(&st, id, company).await?;
    let obj = form::object(&body)?;
    let fields = fields_of(obj, Some(&cur))?;
    let new_lines = if obj.contains_key("lines") {
        Some(lines_of(obj, true)?)
    } else {
        None
    };
    form::ensure_ref(&st.db, "products", fields.output_id, company, "артикулът").await?;
    if let Some(lines) = &new_lines {
        for line in lines {
            form::ensure_ref(&st.db, "products", line.product_id, company, "артикулът").await?;
        }
    }
    invoice_read::exe(
        &st.db,
        &format!(
            "UPDATE recipes SET code = {}, name = {}, output_product_id = {}, output_quantity = {}, output_unit = {}, notes = {}, is_active = {} WHERE id = {id}",
            lit(&fields.code), lit(&fields.name), fields.output_id, lit(&fields.output_qty), lit(&fields.output_unit), lit(&fields.notes), fields.active
        ),
    )
    .await?;
    if let Some(lines) = new_lines {
        invoice_read::exe(&st.db, &format!("DELETE FROM recipe_lines WHERE recipe_id = {id}")).await?;
        write_lines(&st, id, &lines).await?;
    }
    Ok(Json(load(&st, id, company).await?))
}

pub async fn delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let company = company(&st, &headers).await?;
    let _ = load(&st, id, company).await?;
    wipe(&st, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) struct Recipe {
    pub output_id: i32,
    pub output_qty: String,
    pub lines: Vec<RecipeLine>,
}

pub(crate) struct RecipeLine {
    pub product_id: i32,
    pub quantity: String,
    pub unit: String,
    pub wastage: String,
}

pub(crate) async fn load_recipe(st: &AppState, id: i32, company: i32) -> Result<Recipe, ApiError> {
    let row = row_of(st, id, company).await?;
    let lines = line_rows(st, id).await?;
    Ok(Recipe {
        output_id: cell_i32_at(&row, 3),
        output_qty: cell_str(&row, 4),
        lines: lines
            .iter()
            .map(|r| RecipeLine {
                product_id: cell_i32_at(r, 2),
                quantity: cell_str(r, 3),
                unit: cell_str(r, 4),
                wastage: cell_str(r, 5),
            })
            .collect(),
    })
}

struct Fields {
    code: String,
    name: String,
    output_id: i32,
    output_qty: String,
    output_unit: String,
    notes: String,
    active: i32,
}

fn fields_of(obj: &Map<String, Value>, cur: Option<&Value>) -> Result<Fields, ApiError> {
    let name = pick_str(obj, cur, "name")?;
    if name.trim().is_empty() {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "name е задължително"));
    }
    let output_id = pick_i32(obj, cur, "output_product_id")?;
    if output_id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "изходният артикул е задължителен"));
    }
    let qty_raw = pick_str(obj, cur, "output_quantity")?;
    let output_qty = match dec::parse(if qty_raw.trim().is_empty() { "1" } else { &qty_raw }) {
        Some(v) => qty(v),
        None => return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност")),
    };
    let active = if let Some(v) = form::flag_field(obj, "is_active")? {
        v
    } else {
        cur.and_then(|c| c.get("is_active")).and_then(|v| v.as_i64()).unwrap_or(1) as i32
    };
    Ok(Fields {
        code: pick_str(obj, cur, "code")?,
        name,
        output_id,
        output_qty,
        output_unit: pick_str(obj, cur, "output_unit")?,
        notes: pick_str(obj, cur, "notes")?,
        active,
    })
}

struct InLine {
    product_id: i32,
    quantity: String,
    unit: String,
    wastage: String,
}

fn lines_of(obj: &Map<String, Value>, required_key: bool) -> Result<Vec<InLine>, ApiError> {
    let Some(raw) = obj.get("lines") else {
        if required_key {
            return Ok(Vec::new());
        }
        return Ok(Vec::new());
    };
    let Some(arr) = raw.as_array() else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "lines трябва да е масив"));
    };
    let mut lines = Vec::new();
    for el in arr {
        let Some(line) = el.as_object() else {
            return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "редът трябва да е обект"));
        };
        let product_id = form::i32_field(line, "product_id")?.unwrap_or(0);
        if product_id <= 0 {
            continue;
        }
        let quantity = num_text(&form::str_field(line, "quantity")?.unwrap_or_else(|| "0".into()), true)?;
        let wastage = num_text(&form::str_field(line, "wastage_percent")?.unwrap_or_else(|| "0".into()), false)?;
        lines.push(InLine {
            product_id,
            quantity,
            unit: form::str_field(line, "unit")?.unwrap_or_default(),
            wastage,
        });
    }
    Ok(lines)
}

fn num_text(raw: &str, as_qty: bool) -> Result<String, ApiError> {
    let Some(v) = dec::parse(raw) else {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидна стойност"));
    };
    Ok(if as_qty { qty(v) } else { dec::text(v, 2) })
}

fn insert_sql(id: i32, company: i32, f: &Fields) -> String {
    format!(
        "INSERT INTO recipes (id, company_id, code, name, output_product_id, output_quantity, output_unit, notes, is_active) \
         VALUES ({id}, {company}, {}, {}, {}, {}, {}, {}, {})",
        lit(&f.code), lit(&f.name), f.output_id, lit(&f.output_qty), lit(&f.output_unit), lit(&f.notes), f.active
    )
}

async fn write_lines(st: &AppState, recipe_id: i32, lines: &[InLine]) -> Result<(), ApiError> {
    for (i, line) in lines.iter().enumerate() {
        let id = invoice_read::nid(&st.db, "recipe_lines").await?;
        let no = (i + 1) as i32;
        invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO recipe_lines (id, recipe_id, product_id, quantity, unit, wastage_percent, line_no) \
                 VALUES ({id}, {recipe_id}, {}, {}, {}, {}, {no})",
                line.product_id, lit(&line.quantity), lit(&line.unit), lit(&line.wastage)
            ),
        )
        .await?;
    }
    Ok(())
}

async fn load(st: &AppState, id: i32, company: i32) -> Result<Value, ApiError> {
    let row = row_of(st, id, company).await?;
    let mut item = head(&row);
    item["lines"] = json!(line_json(&line_rows(st, id).await?));
    Ok(item)
}

async fn row_of(st: &AppState, id: i32, company: i32) -> Result<crate::db::Row, ApiError> {
    if id <= 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid id"));
    }
    let rows = invoice_read::qry(&st.db, &format!("{SEL} WHERE id = {id}")).await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(api_err(StatusCode::NOT_FOUND, "Рецептата не е намерена"));
    };
    if cell_i32_at(&row, 8) != company {
        return Err(api_err(StatusCode::NOT_FOUND, "Рецептата не е намерена"));
    }
    Ok(row)
}

fn head(row: &crate::db::Row) -> Value {
    json!({
        "id": cell_i32_at(row, 0),
        "code": cell_str(row, 1),
        "name": cell_str(row, 2),
        "output_product_id": cell_i32_at(row, 3),
        "output_quantity": cell_str(row, 4),
        "output_unit": cell_str(row, 5),
        "notes": cell_str(row, 6),
        "is_active": cell_i32_at(row, 7),
    })
}

async fn line_rows(st: &AppState, recipe_id: i32) -> Result<Vec<crate::db::Row>, ApiError> {
    invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, recipe_id, product_id, quantity, unit, wastage_percent, line_no \
             FROM recipe_lines WHERE recipe_id = {recipe_id} ORDER BY line_no, id"
        ),
    )
    .await
}

fn line_json(rows: &[crate::db::Row]) -> Vec<Value> {
    rows.iter()
        .map(|r| {
            json!({
                "id": cell_i32_at(r, 0),
                "recipe_id": cell_i32_at(r, 1),
                "product_id": cell_i32_at(r, 2),
                "quantity": cell_str(r, 3),
                "unit": cell_str(r, 4),
                "wastage_percent": cell_str(r, 5),
                "line_no": cell_i32_at(r, 6),
            })
        })
        .collect()
}

async fn wipe(st: &AppState, id: i32) -> Result<(), ApiError> {
    invoice_read::exe(&st.db, &format!("DELETE FROM recipe_lines WHERE recipe_id = {id}")).await?;
    invoice_read::exe(&st.db, &format!("DELETE FROM recipes WHERE id = {id}")).await
}

fn pick_str(obj: &Map<String, Value>, cur: Option<&Value>, key: &str) -> Result<String, ApiError> {
    if let Some(v) = form::str_field(obj, key)? {
        return Ok(v);
    }
    Ok(cur.and_then(|c| c.get(key)).and_then(|v| v.as_str()).unwrap_or("").to_string())
}

fn pick_i32(obj: &Map<String, Value>, cur: Option<&Value>, key: &str) -> Result<i32, ApiError> {
    if let Some(v) = form::i32_field(obj, key)? {
        return Ok(v);
    }
    Ok(cur.and_then(|c| c.get(key)).and_then(|v| v.as_i64()).unwrap_or(0) as i32)
}

async fn company(st: &AppState, headers: &HeaderMap) -> Result<i32, ApiError> {
    let sub = auth_sub(st, headers)?;
    let id = active_or_one(st, &sub).await?;
    if id <= 0 {
        return Err(api_err(StatusCode::UNPROCESSABLE_ENTITY, "няма избрана активна фирма"));
    }
    Ok(id)
}
