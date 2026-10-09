//! ДДС дневници и файл за НАП. Източникът е дневникът на активната фирма.
//! Ставките идват от редовете на фактурата. Ръчен запис без фактура е 20% или без ДДС.

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::db::{cell_i32_at, cell_str, lit, Row};
use crate::form::{active_or_one, auth_sub};
use crate::invoice_money::cents;
use crate::invoice_read;
use crate::vat_files::{self, deklar, pok_line, pro_line};
use crate::vat_fmt::{self, PeriodBad};
use crate::vat_line::{self, Cells, Line};
use crate::{api_err, AppState, ApiError};

#[derive(Deserialize, Default)]
pub(crate) struct PeriodQ {
    period: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

struct Co {
    name: String,
    vat: String,
    submitter: String,
    branch: String,
}

pub(crate) struct Pack {
    pub(crate) body: Value,
    pub(crate) pok: Vec<u8>,
    pub(crate) pro: Vec<u8>,
    pub(crate) dek: Vec<u8>,
    pub(crate) period_nap: String,
}

fn period_err(raw: &str) -> ApiError {
    if raw.trim().is_empty() {
        api_err(StatusCode::BAD_REQUEST, "period е задължителен (YYYY-MM)")
    } else {
        api_err(StatusCode::BAD_REQUEST, "period трябва да е YYYY-MM или YYYYMM")
    }
}

async fn company(st: &AppState, id: i32) -> Result<Co, ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT name, vat_number, accountant_name, accountant_egn, authorized_person_name, authorized_person_egn, manager_name, manager_egn, vat_branch_number FROM companies WHERE id = {id}"
        ),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::INTERNAL_SERVER_ERROR, "няма фирма за ДДС"));
    };
    let vat = vat_fmt::with_bg(&cell_str(row, 1));
    Ok(Co {
        name: cell_str(row, 0),
        submitter: vat_files::submitter(
            &cell_str(row, 3),
            &cell_str(row, 2),
            &cell_str(row, 5),
            &cell_str(row, 4),
            &cell_str(row, 7),
            &cell_str(row, 6),
        ),
        branch: cell_str(row, 8),
        vat,
    })
}

fn sums(rows: &[Row]) -> (i64, i64) {
    let mut total = 0i64;
    let mut vat = 0i64;
    for row in rows {
        if cell_str(row, 0) == "debit" {
            total += cents(&cell_str(row, 1)).unwrap_or(0);
        }
        vat += cents(&cell_str(row, 2)).unwrap_or(0);
    }
    (total, vat)
}

async fn overlay(st: &AppState, ln: &mut Line, inv_id: i32) -> Result<(), ApiError> {
    let head = invoice_read::qry(
        &st.db,
        &format!("SELECT issue_date FROM invoices WHERE id = {inv_id}"),
    )
    .await?;
    let Some(head) = head.first() else {
        return Ok(());
    };
    let issue = cell_str(head, 0);
    if !issue.is_empty() {
        ln.doc_date = issue;
    }
    let extra = invoice_read::qry(
        &st.db,
        &format!("SELECT vat_exemption_reason FROM invoice_extra WHERE id = {inv_id}"),
    )
    .await?;
    let ex = extra.first().map(|r| cell_str(r, 0)).unwrap_or_default();
    let lines = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT description, net_amount, vat_amount, vat_rate FROM invoice_lines WHERE invoice_id = {inv_id} ORDER BY id"
        ),
    )
    .await?;
    if lines.is_empty() {
        return Ok(());
    }
    ln.reset_amounts();
    for (i, row) in lines.iter().enumerate() {
        let goods = cell_str(row, 0);
        if i == 0 && !goods.is_empty() {
            ln.goods = goods;
        }
        let base = cents(&cell_str(row, 1)).unwrap_or(0);
        let vat = cents(&cell_str(row, 2)).unwrap_or(0);
        let rate = vat_fmt::rate_kind(&cell_str(row, 3));
        if ln.kind == "sales" {
            ln.add_sale(base, vat, rate, &ex);
        } else {
            ln.add_purch(base, vat);
        }
    }
    Ok(())
}

async fn one_line(st: &AppState, row: &Row, kind: &str) -> Result<Line, ApiError> {
    let id = cell_i32_at(row, 0);
    let jl = invoice_read::qry(
        &st.db,
        &format!("SELECT direction, amount, vat_amount FROM journal_lines WHERE entry_id = {id}"),
    )
    .await?;
    let (total, vat) = sums(&jl);
    let mut ln = Line::from_totals(kind, total - vat, vat);
    ln.doc_type = vat_fmt::doc_type(&cell_str(row, 3));
    ln.doc_no = cell_str(row, 5);
    let mut dd = cell_str(row, 10);
    if dd.is_empty() {
        dd = cell_str(row, 2);
    }
    ln.doc_date = dd;
    ln.cp_name = cell_str(row, 9);
    ln.goods = cell_str(row, 6);
    let cp = cell_i32_at(row, 8);
    if cp > 0 {
        let cps = invoice_read::qry(
            &st.db,
            &format!("SELECT vat_number FROM counterparts WHERE id = {cp}"),
        )
        .await?;
        if let Some(c) = cps.first() {
            ln.cp_vat = vat_fmt::with_bg(&cell_str(c, 0));
        }
    }
    let doc_id = cell_i32_at(row, 4);
    if doc_id > 0 {
        overlay(st, &mut ln, doc_id).await?;
    }
    ln.apply_credit();
    Ok(ln)
}

async fn collect(st: &AppState, company_id: i32, period: &vat_fmt::Period) -> Result<Pack, ApiError> {
    let co = company(st, company_id).await?;
    let mut rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, company_id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period FROM journal_entries WHERE vat_period = {}",
            lit(&period.iso)
        ),
    )
    .await?;
    rows.sort_by(|a, b| {
        cell_str(a, 2)
            .cmp(&cell_str(b, 2))
            .then(cell_i32_at(a, 0).cmp(&cell_i32_at(b, 0)))
    });
    let mut cells = Cells::default();
    let mut purchases = Vec::new();
    let mut sales = Vec::new();
    let mut pok = Vec::new();
    let mut pro = Vec::new();
    for row in &rows {
        let vt = cell_str(row, 7);
        if vt != "purchase" && vt != "sales" {
            continue;
        }
        let owner = cell_i32_at(row, 1);
        if owner != 0 && owner != company_id {
            continue;
        }
        let ln = one_line(st, row, &vt).await?;
        let id = cell_i32_at(row, 0);
        if vt == "purchase" {
            let n = cells.purch_n + 1;
            cells.add(&ln);
            purchases.push(ln.to_json(id, n as i32));
            pok.extend(pok_line(&co.vat, &period.nap, &co.branch, n, &ln));
        } else {
            let n = cells.sales_n + 1;
            cells.add(&ln);
            sales.push(ln.to_json(id, n as i32));
            pro.extend(pro_line(&co.vat, &period.nap, &co.branch, n, &ln));
        }
    }
    cells.finish();
    let dek = deklar(&co.vat, &co.name, &period.nap, &co.submitter, &cells);
    let body = vat_line::pack_json(
        &period.iso,
        &period.nap,
        &co.name,
        &co.vat,
        &co.submitter,
        purchases,
        sales,
        &cells,
    );
    Ok(Pack {
        body,
        pok,
        pro,
        dek,
        period_nap: period.nap.clone(),
    })
}

pub(crate) async fn pack(st: &AppState, headers: &HeaderMap, raw: &str) -> Result<Pack, ApiError> {
    let sub = auth_sub(st, headers)?;
    let company_id = active_or_one(st, &sub).await?;
    let period = match vat_fmt::parse_period(raw) {
        Ok(p) => p,
        Err(PeriodBad::Empty) => return Err(period_err(raw)),
        Err(PeriodBad::Format) => return Err(period_err(raw)),
    };
    collect(st, company_id, &period).await
}

pub async fn registers(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PeriodQ>,
) -> Result<Json<Value>, ApiError> {
    let packed = pack(&st, &headers, q.period.as_deref().unwrap_or("")).await?;
    Ok(Json(packed.body))
}

fn file_resp(data: Vec<u8>, mime: &'static str, filename: String) -> Response {
    let disp = format!("attachment; filename=\"{filename}\"");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_DISPOSITION, disp)
        .body(Body::from(data))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

pub async fn export(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PeriodQ>,
) -> Result<Response, ApiError> {
    let packed = pack(&st, &headers, q.period.as_deref().unwrap_or("")).await?;
    let kind = q.kind.unwrap_or_default();
    let resp = match kind.as_str() {
        "pokupki" => file_resp(packed.pok, "text/plain; charset=windows-1251", "POKUPKI.TXT".into()),
        "prodagbi" | "prodajbi" => {
            file_resp(packed.pro, "text/plain; charset=windows-1251", "PRODAGBI.TXT".into())
        }
        "zip" => file_resp(
            vat_files::zip3(&packed.dek, &packed.pok, &packed.pro),
            "application/zip",
            format!("PPDDS_{}.zip", packed.period_nap),
        ),
        _ => file_resp(packed.dek, "text/plain; charset=windows-1251", "DEKLAR.TXT".into()),
    };
    Ok(resp)
}
