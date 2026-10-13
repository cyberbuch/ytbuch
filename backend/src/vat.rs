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
    street: String,
    city: String,
    post: String,
    decl_egn: String,
    decl_name: String,
    quality: String,
}

pub(crate) struct Pack {
    pub(crate) body: Value,
    pub(crate) pok: Vec<u8>,
    pub(crate) pro: Vec<u8>,
    pub(crate) dek: Vec<u8>,
    pub(crate) vies: Vec<u8>,
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
            "SELECT name, vat_number, accountant_name, accountant_egn, authorized_person_name, authorized_person_egn, manager_name, manager_egn, vat_branch_number, \
             city, post_code, street_name, building_number, address_building, additional_address_detail \
             FROM companies WHERE id = {id}"
        ),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Err(api_err(StatusCode::INTERNAL_SERVER_ERROR, "няма фирма за ДДС"));
    };
    let vat = vat_fmt::with_bg(&cell_str(row, 1));
    let (decl_egn, decl_name, quality) = vat_files::declarant(
        &cell_str(row, 3),
        &cell_str(row, 2),
        &cell_str(row, 5),
        &cell_str(row, 4),
        &cell_str(row, 7),
        &cell_str(row, 6),
    );
    let street = street_of(&cell_str(row, 11), &cell_str(row, 12), &cell_str(row, 14));
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
        street,
        city: cell_str(row, 9),
        post: cell_str(row, 10),
        decl_egn,
        decl_name,
        quality,
        vat,
    })
}

fn street_of(street: &str, number: &str, extra: &str) -> String {
    let mut out = street.trim().to_string();
    let n = number.trim();
    if !n.is_empty() {
        if !out.is_empty() {
            out.push_str(" № ");
        }
        out.push_str(n);
    }
    let e = extra.trim();
    if !e.is_empty() {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(e);
    }
    out
}

fn vies_vin(raw: &str) -> Option<String> {
    let v = raw.trim().to_ascii_uppercase();
    let b = v.as_bytes();
    if b.len() < 3 || b.len() > 15 {
        return None;
    }
    if !b[..2].iter().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    if v.starts_with("BG") {
        return None;
    }
    Some(v)
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

async fn overlay(st: &AppState, company: i32, ln: &mut Line, inv_id: i32) -> Result<(), ApiError> {
    let head = invoice_read::qry(
        &st.db,
        &format!("SELECT issue_date FROM invoices WHERE id = {inv_id} AND company_id = {company}"),
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

async fn one_line(st: &AppState, company: i32, row: &Row, kind: &str) -> Result<Line, ApiError> {
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
            &format!("SELECT vat_number FROM counterparts WHERE id = {cp} AND company_id = {company}"),
        )
        .await?;
        if let Some(c) = cps.first() {
            ln.cp_vat = vat_fmt::cp_vat(&cell_str(c, 0));
        }
    }
    let doc_id = cell_i32_at(row, 4);
    if doc_id > 0 {
        overlay(st, company, &mut ln, doc_id).await?;
    }
    ln.apply_credit();
    Ok(ln)
}

async fn load_manual(st: &AppState, company_id: i32, period: &str, cells: &mut vat_line::Cells) -> Result<(), ApiError> {
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT cell_33, cell_70, cell_71, cell_80, cell_81, cell_82 FROM vat_manual WHERE company_id = {company_id} AND period = {}",
            lit(period)
        ),
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(());
    };
    let c33 = cell_str(row, 0);
    cells.m33 = if c33.trim().is_empty() { "0.00".into() } else { c33 };
    cells.m70 = cents(&cell_str(row, 1)).unwrap_or(0);
    cells.m71 = cents(&cell_str(row, 2)).unwrap_or(0);
    cells.m80 = cents(&cell_str(row, 3)).unwrap_or(0);
    cells.m81 = cents(&cell_str(row, 4)).unwrap_or(0);
    cells.m82 = cents(&cell_str(row, 5)).unwrap_or(0);
    Ok(())
}

fn money_field(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<String, ApiError> {
    let raw = obj
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .replace(',', ".");
    if raw.is_empty() {
        return Ok("0.00".into());
    }
    match cents(&raw) {
        Some(c) => Ok(crate::invoice_money::money(c)),
        None => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} трябва да е сума"),
        )),
    }
}

fn coef_field(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<String, ApiError> {
    let raw = obj
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .replace(',', ".");
    if raw.is_empty() {
        return Ok("0.00".into());
    }
    match raw.parse::<f64>() {
        Ok(v) if (0.0..=1.0).contains(&v) => Ok(format!("{v:.2}")),
        _ => Err(api_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{key} трябва да е коефициент между 0 и 1"),
        )),
    }
}

pub async fn manual_save(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PeriodQ>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let sub = auth_sub(&st, &headers)?;
    let company_id = active_or_one(&st, &sub).await?;
    let period = match vat_fmt::parse_period(q.period.as_deref().unwrap_or("")) {
        Ok(p) => p,
        Err(_) => return Err(period_err(q.period.as_deref().unwrap_or(""))),
    };
    let obj = body.as_object().cloned().ok_or_else(|| {
        api_err(StatusCode::UNPROCESSABLE_ENTITY, "невалидно тяло")
    })?;
    let c33 = coef_field(&obj, "cell_33")?;
    let c70 = money_field(&obj, "cell_70")?;
    let c71 = money_field(&obj, "cell_71")?;
    let c80 = money_field(&obj, "cell_80")?;
    let c81 = money_field(&obj, "cell_81")?;
    let c82 = money_field(&obj, "cell_82")?;
    let existing = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id FROM vat_manual WHERE company_id = {company_id} AND period = {}",
            lit(&period.iso)
        ),
    )
    .await?;
    if let Some(row) = existing.first() {
        let id = cell_i32_at(row, 0);
        invoice_read::exe(
            &st.db,
            &format!(
                "UPDATE vat_manual SET cell_33 = {}, cell_70 = {}, cell_71 = {}, cell_80 = {}, cell_81 = {}, cell_82 = {} WHERE id = {id}",
                lit(&c33), lit(&c70), lit(&c71), lit(&c80), lit(&c81), lit(&c82)
            ),
        )
        .await?;
    } else {
        let id = st.db.next_id("vat_manual").await.map_err(|e| {
            api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        invoice_read::exe(
            &st.db,
            &format!(
                "INSERT INTO vat_manual (id, company_id, period, cell_33, cell_70, cell_71, cell_80, cell_81, cell_82) VALUES ({id}, {company_id}, {}, {}, {}, {}, {}, {}, {})",
                lit(&period.iso), lit(&c33), lit(&c70), lit(&c71), lit(&c80), lit(&c81), lit(&c82)
            ),
        )
        .await?;
    }
    let mut cells = vat_line::Cells::default();
    load_manual(&st, company_id, &period.iso, &mut cells).await?;
    let packed = collect(&st, company_id, &period).await?;
    Ok(Json(packed.body))
}

async fn collect(st: &AppState, company_id: i32, period: &vat_fmt::Period) -> Result<Pack, ApiError> {
    let co = company(st, company_id).await?;
    let mut rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id, company_id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period FROM journal_entries WHERE vat_period = {} AND company_id = {}",
            lit(&period.iso),
            company_id
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
    let mut vies_map: std::collections::HashMap<String, (i64, i64, i64)> = std::collections::HashMap::new();
    for row in &rows {
        let vt = cell_str(row, 7);
        if vt != "purchase" && vt != "sales" {
            continue;
        }
        let owner = cell_i32_at(row, 1);
        if owner != 0 && owner != company_id {
            continue;
        }
        let ln = one_line(st, company_id, row, &vt).await?;
        let id = cell_i32_at(row, 0);
        if vt == "purchase" {
            let n = cells.purch_n + 1;
            cells.add(&ln);
            purchases.push(ln.to_json(id, n as i32));
            pok.extend(pok_line(&co.vat, &period.nap, &co.branch, n, &ln));
        } else {
            let n = cells.sales_n + 1;
            if let Some(vin) = vies_vin(&ln.cp_vat) {
                let e = vies_map.entry(vin).or_insert((0, 0, 0));
                e.0 += ln.s_vod;
                e.1 += ln.s_tri;
                e.2 += ln.s_svc;
            }
            cells.add(&ln);
            sales.push(ln.to_json(id, n as i32));
            pro.extend(pro_line(&co.vat, &period.nap, &co.branch, n, &ln));
        }
    }
    cells.finish();
    load_manual(st, company_id, &period.iso, &mut cells).await?;
    let mut vies_parties: Vec<vat_files::ViesParty> = vies_map
        .into_iter()
        .filter(|(_, (k3, k4, k5))| *k3 != 0 || *k4 != 0 || *k5 != 0)
        .map(|(vin, (k3, k4, k5))| vat_files::ViesParty { vin, k3, k4, k5 })
        .collect();
    vies_parties.sort_by(|a, b| a.vin.cmp(&b.vin));
    let mm = &period.nap[4..6];
    let yyyy = &period.nap[..4];
    let vies = vat_files::vies_txt(
        &vat_files::ViesHead {
            vat: co.vat.clone(),
            name: co.name.clone(),
            street: co.street.clone(),
            decl_egn: co.decl_egn.clone(),
            decl_name: co.decl_name.clone(),
            decl_city: co.city.clone(),
            decl_post: co.post.clone(),
            decl_street: co.street.clone(),
            quality: co.quality.clone(),
        },
        &format!("{mm}/{yyyy}"),
        &vies_parties,
    );
    let vies_rows: Vec<Value> = vies_parties
        .iter()
        .map(|p| {
            serde_json::json!({
                "vat_number": p.vin,
                "country": &p.vin[..2],
                "base_vod": crate::invoice_money::money(p.k3),
                "base_tri": crate::invoice_money::money(p.k4),
                "base_services": crate::invoice_money::money(p.k5),
                "total": crate::invoice_money::money(p.k3 + p.k4 + p.k5),
            })
        })
        .collect();
    let dek = deklar(&co.vat, &co.name, &period.nap, &co.submitter, &cells);
    let mut body = vat_line::pack_json(
        &period.iso,
        &period.nap,
        &co.name,
        &co.vat,
        &co.submitter,
        purchases,
        sales,
        &cells,
    );
    if let Some(map) = body.as_object_mut() {
        map.insert("vies".into(), serde_json::json!(vies_rows));
    }
    Ok(Pack {
        body,
        pok,
        pro,
        dek,
        vies,
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
        "vies" => file_resp(
            packed.vies,
            "text/plain; charset=windows-1251",
            format!("VIES_{}.TXT", packed.period_nap),
        ),
        "zip" => file_resp(
            vat_files::zip3(&packed.dek, &packed.pok, &packed.pro),
            "application/zip",
            format!("PPDDS_{}.zip", packed.period_nap),
        ),
        _ => file_resp(packed.dek, "text/plain; charset=windows-1251", "DEKLAR.TXT".into()),
    };
    Ok(resp)
}
