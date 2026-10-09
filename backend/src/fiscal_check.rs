//! Интеграционни тестове на фискалното ядро срещу жив Octo: фактура →
//! пост в дневника, склад WAC и ДДС дневник. Тестват се handler-ите и
//! `post_one`/`unpost_one` директно, без HTTP.
//! Без Octo на `YTBUCH_DATABASE_URL` (CI без база) всеки тест се
//! прескача — в тестовете има `let Some(...) else { return }`.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap};
use axum::Json;
use serde_json::json;

use crate::auth;
use crate::db::{self, Db};
use crate::dec;
use crate::form;
use crate::invoice_money;
use crate::invoice_post;
use crate::invoice_read;
use crate::invoices;
use crate::vat;
use crate::AppState;

/// Месец далеч от демо данните: периодът няма ред и е отворен,
/// а в ДДС дневника сме само ние.
const MONTH: &str = "2031-01";
const CP_NAME: &str = "Тест Фискален К";
const PROD_CODE: &str = "TST-FISCAL";
const BUY_NO: &str = "T-FISCAL-BUY";
const SELL_NO: &str = "T-FISCAL-SELL";

/// `next_id` е MAX(id)+1 — паралелните тестове биха разменили id-тата.
/// Всички тестове минават една по една през тази ключалка.
static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn octo() -> Option<Db> {
    let url = std::env::var("YTBUCH_DATABASE_URL").unwrap_or_else(|_| {
        "host=127.0.0.1 port=1337 user=ydb password=ydbrocks dbname=ydb sslmode=disable".into()
    });
    match tokio::time::timeout(std::time::Duration::from_secs(5), Db::connect(&url, 2)).await {
        Ok(Ok(db)) => Some(db),
        _ => {
            eprintln!("SKIP: няма Octo на YTBUCH_DATABASE_URL");
            None
        }
    }
}

fn auth_headers(st: &AppState) -> HeaderMap {
    let token = auth::issue(&st.jwt_secret, "superadmin").expect("jwt");
    let mut headers = HeaderMap::new();
    let value = format!("Bearer {token}").parse().expect("bearer");
    headers.insert(header::AUTHORIZATION, value);
    headers
}

struct Fixture {
    st: AppState,
    headers: HeaderMap,
    company: i32,
    counterpart: i32,
    product: i32,
}

/// Изчиства остатъци от прекъснат тест и вдига нови фикстури.
async fn fixture() -> Option<Fixture> {
    let st = AppState {
        db: Arc::new(octo().await?),
        jwt_secret: "fiscal-test-secret".into(),
    };
    purge(&st).await;
    let company = form::active_or_one(&st, "superadmin")
        .await
        .expect("активна фирма");
    let counterpart = invoice_read::nid(&st.db, "counterparts")
        .await
        .expect("id на контрагент");
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO counterparts (id, company_id, name, counterpart_type, eik, vat_number, \
             address, city, post_code, country, contact_person) \
             VALUES ({counterpart}, {company}, {}, 'company', '999999999', '', '', '', '', 'BG', '')",
            db::lit(CP_NAME)
        ),
    )
    .await
    .expect("контрагент");
    let product = invoice_read::nid(&st.db, "products").await.expect("id на стока");
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO products (id, company_id, name, code, description, unit, price, cost_price, \
             vat_rate, is_service, is_inventory, inventory_account_id, revenue_account_id, \
             stock_quantity, stock_value, is_active) \
             VALUES ({product}, {company}, 'Тест Артикул', {}, '', 'C62', '5.00', '5.00', '20', 0, 1, 0, 0, '0', '0', 1)",
            db::lit(PROD_CODE)
        ),
    )
    .await
    .expect("стока");
    let headers = auth_headers(&st);
    Some(Fixture {
        st,
        headers,
        company,
        counterpart,
        product,
    })
}

/// Триема всичко с нашите маркери: фактури, дневник, склад, фикстури.
/// Трие се и в началото, и в края — тестът оцелява при рестарт.
async fn purge(st: &AppState) {
    for number in [BUY_NO, SELL_NO] {
        let rows = invoice_read::qry(
            &st.db,
            &format!("SELECT id FROM invoices WHERE number = {}", db::lit(number)),
        )
        .await;
        for row in rows.unwrap_or_default() {
            let id = db::cell_i32(&row);
            let entries = invoice_read::qry(
                &st.db,
                &format!("SELECT id FROM journal_entries WHERE document_id = {id}"),
            )
            .await
            .unwrap_or_default();
            for entry in entries {
                let entry_id = db::cell_i32(&entry);
                let _ = invoice_read::wipe_journal(&st.db, entry_id).await;
            }
            let _ = invoice_read::wipe_invoice(&st.db, id).await;
        }
    }
    let products = invoice_read::qry(
        &st.db,
        &format!("SELECT id FROM products WHERE code = {}", db::lit(PROD_CODE)),
    )
    .await
    .unwrap_or_default();
    for row in products {
        let id = db::cell_i32(&row);
        let _ = invoice_read::exe(&st.db, &format!("DELETE FROM inventory_lots WHERE product_id = {id}")).await;
        let _ = invoice_read::exe(&st.db, &format!("DELETE FROM products WHERE id = {id}")).await;
    }
    let counterparts = invoice_read::qry(
        &st.db,
        &format!("SELECT id FROM counterparts WHERE name = {}", db::lit(CP_NAME)),
    )
    .await
    .unwrap_or_default();
    for row in counterparts {
        let id = db::cell_i32(&row);
        let _ = invoice_read::exe(&st.db, &format!("DELETE FROM counterparts WHERE id = {id}")).await;
    }
}

async fn create_invoice(
    f: &Fixture,
    number: &str,
    direction: &str,
    doc_type: &str,
    qty: &str,
    price: &str,
    net: &str,
    vat: &str,
    total: &str,
) -> i32 {
    let body = json!({
        "direction": direction,
        "document_type": doc_type,
        "number": number,
        "issue_date": "2031-01-10",
        "due_date": "2031-02-10",
        "accounting_month": MONTH,
        "counterpart_id": f.counterpart,
        "lines": [{
            "product_id": f.product,
            "quantity": qty,
            "unit_price": price,
            "vat_rate": "20",
            "net_amount": net,
            "vat_amount": vat,
            "total_amount": total,
        }],
    });
    let (code, Json(value)) = invoices::create(State(f.st.clone()), f.headers.clone(), Json(body))
        .await
        .expect("фактурата не се създава");
    assert_eq!(code, axum::http::StatusCode::CREATED);
    value["id"].as_i64().expect("id на фактура") as i32
}

/// Редовете в дневника на дадена фактура: (сметка, посока, стотини).
async fn journal_cents(f: &Fixture, inv_id: i32) -> Vec<(String, String, i64)> {
    let rows = invoice_read::qry(
        &f.st.db,
        &format!(
            "SELECT l.account_number, l.direction, l.amount FROM journal_lines l, journal_entries e \
             WHERE l.entry_id = e.id AND e.document_id = {inv_id}"
        ),
    )
    .await
    .expect("дневникът не се чете");
    rows.iter()
        .map(|r| {
            (
                db::cell_str(r, 0),
                db::cell_str(r, 1),
                invoice_money::cents(&db::cell_str(r, 2)).unwrap_or(0),
            )
        })
        .collect()
}

async fn assert_journal(f: &Fixture, inv_id: i32, expected: &[(&str, &str, i64)]) {
    let mut got = journal_cents(f, inv_id).await;
    got.sort();
    let mut want: Vec<(String, String, i64)> = expected
        .iter()
        .map(|(n, d, c)| (n.to_string(), d.to_string(), *c))
        .collect();
    want.sort();
    assert_eq!(got, want, "дневникът на фактура {inv_id} не съвпада");
}

async fn assert_stock(f: &Fixture, qty: &str, value: &str) {
    let rows = invoice_read::qry(
        &f.st.db,
        &format!("SELECT stock_quantity, stock_value FROM products WHERE id = {}", f.product),
    )
    .await
    .expect("складът не се чете");
    let row = rows.first().expect("стоката липсва");
    assert_eq!(
        dec::or0(&db::cell_str(row, 0)),
        dec::or0(qty),
        "количеството не съвпада"
    );
    assert_eq!(dec::or0(&db::cell_str(row, 1)), dec::or0(value), "стойността не съвпада");
}

/// Покупка 11 (10 × 5.00 = 50 + ДДС 10 = 60), продажба 01 (4 × 10.00 =
/// 40 + ДДС 8 = 48). WAC: продажбата носи себестойност 4 × 5 = 20.
#[tokio::test]
async fn fiscal_post_and_unpost_wac() {
    let _lock = LOCK.lock().await;
    let Some(f) = fixture().await else { return };

    let buy = create_invoice(&f, BUY_NO, "in", "11", "10", "5.00", "50.00", "10.00", "60.00").await;
    let posted = invoice_post::post_one(&f.st, f.company, buy).await.expect("постът на покупката");
    assert_eq!(posted["document_number"].as_str(), Some(BUY_NO));
    assert_journal(
        &f,
        buy,
        &[
            ("304", "debit", 5000),
            ("4531", "debit", 1000),
            ("401", "credit", 6000),
        ],
    )
    .await;
    assert_stock(&f, "10", "50").await;

    let sell = create_invoice(&f, SELL_NO, "out", "01", "4", "10.00", "40.00", "8.00", "48.00").await;
    invoice_post::post_one(&f.st, f.company, sell).await.expect("постът на продажбата");
    assert_journal(
        &f,
        sell,
        &[
            ("411", "debit", 4800),
            ("702", "credit", 4000),
            ("4532", "credit", 800),
            ("702", "debit", 2000),
            ("304", "credit", 2000),
        ],
    )
    .await;
    assert_stock(&f, "6", "30").await;

    invoice_post::unpost_one(&f.st, f.company, sell).await.expect("връщането на продажбата");
    assert_journal(&f, sell, &[]).await;
    assert_stock(&f, "10", "50").await;
    // Покупката не се връща: партидата ѝ е била изписвана от продажбата.
    // Правилото пази склада от тихо разместване след реализация.
    let refusal = invoice_post::unpost_one(&f.st, f.company, buy).await.expect_err("връщането на покупката трябва да откаже");
    assert_eq!(refusal.0, axum::http::StatusCode::UNPROCESSABLE_ENTITY);

    purge(&f.st).await;
}

/// ДДС дневникът и файловете за НАП събират постнатите фактури на месеца.
#[tokio::test]
async fn fiscal_vat_register_and_files() {
    let _lock = LOCK.lock().await;
    let Some(f) = fixture().await else { return };

    let buy = create_invoice(&f, BUY_NO, "in", "11", "10", "5.00", "50.00", "10.00", "60.00").await;
    let sell = create_invoice(&f, SELL_NO, "out", "01", "4", "10.00", "40.00", "8.00", "48.00").await;
    invoice_post::post_one(&f.st, f.company, buy).await.expect("постът на покупката");
    invoice_post::post_one(&f.st, f.company, sell).await.expect("постът на продажбата");

    let packed = vat::pack(&f.st, &f.headers, MONTH).await.expect("ДДС дневникът не се събира");
    let body = packed.body.to_string();
    assert!(body.contains(BUY_NO), "регистърът не съдържа покупката {BUY_NO}");
    assert!(body.contains(SELL_NO), "регистърът не съдържа продажбата {SELL_NO}");
    let pok = String::from_utf8_lossy(&packed.pok);
    let pro = String::from_utf8_lossy(&packed.pro);
    assert!(pok.contains(BUY_NO), "POKUPKI.TXT не съдържа {BUY_NO}");
    assert!(pro.contains(SELL_NO), "PRODAGBI.TXT не съдържа {SELL_NO}");

    purge(&f.st).await;
}

