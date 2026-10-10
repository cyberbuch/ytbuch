//! Пагинация за списъчни endpoints. Octo няма LIMIT/OFFSET в SQL,
//! затова пагинацията се прилага в паметта след fetch на редовете.

use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

pub const DEFAULT_LIMIT: u32 = 50;
pub const MAX_LIMIT: u32 = 500;

#[derive(Debug, Default, Deserialize)]
pub struct PageQuery {
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

impl PageQuery {
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }

    pub fn offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

/// Пагинира вектор от JSON обекти и връща готов отговор.
pub fn paginate(items: Vec<Value>, q: &PageQuery) -> Json<Value> {
    let total = items.len() as u32;
    let limit = q.limit();
    let offset = q.offset().min(total);
    let end = (offset + limit).min(total) as usize;
    let page: Vec<Value> = items[offset as usize..end].to_vec();
    let count = page.len() as u32;
    Json(json!({
        "items": page,
        "count": count,
        "total": total,
        "limit": limit,
        "offset": offset,
        "has_more": end < total as usize,
    }))
}
