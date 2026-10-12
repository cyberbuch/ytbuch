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
use crate::stock_replay;
use crate::stock_writeoff;
use crate::vat;
use crate::AppState;

/// Месец далеч от демо данните: периодът няма ред и е отворен,
/// а в ДДС дневника сме само ние.
const MONTH: &str = "2031-01";
const CP_NAME: &str = "Тест Фискален К";
const PROD_CODE: &str = "TST-FISCAL";
const BUY_NO: &str = "T-FISCAL-BUY";
const SELL_NO: &str = "T-FISCAL-SELL";
const MARK_NOS: &[&str] = &[BUY_NO, SELL_NO, "T-GA-BUY", "T-GA-SELL", "T-GB-BUY", "T-GB-SELL"];
const MARK_CODES: &[&str] = &[PROD_CODE, "TST-GOODS-A", "TST-GOODS-B"];

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
    let db = octo().await?;
    db.migrate().await.expect("migrate");
    let st = AppState {
        db: Arc::new(db),
        jwt_secret: "fiscal-test-secret".into(),
    };
    purge(&st).await;
    let company = form::active_or_one(&st, "superadmin")
        .await
        .expect("активна фирма");
    let inventory = account_id(&st, company, "304").await;
    let revenue = account_id(&st, company, "702").await;
    ensure_goods(&st, company, inventory, revenue).await;
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
             VALUES ({product}, {company}, 'Тест Артикул', {}, '', 'C62', '5.00', '5.00', '20', 0, 1, {inventory}, {revenue}, '0', '0', 1)",
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
async fn account_id(st: &AppState, company: i32, number: &str) -> i32 {
    let rows = invoice_read::qry(
        &st.db,
        &format!("SELECT id FROM accounts WHERE company_id = {company} AND number = {}", db::lit(number)),
    )
    .await
    .expect("сметкоплан");
    let row = rows.first().unwrap_or_else(|| panic!("няма сметка {number}"));
    db::cell_i32(row)
}

async fn ensure_goods(st: &AppState, company: i32, inventory: i32, offset: i32) -> Option<i32> {
    let rows = invoice_read::qry(
        &st.db,
        &format!(
            "SELECT id FROM stock_account_rules WHERE company_id = {company} AND kind = 'goods' \
             AND inventory_account_id = {inventory} AND offset_account_id = {offset}"
        ),
    )
    .await
    .expect("правила");
    if !rows.is_empty() {
        return None;
    }
    let id = invoice_read::nid(&st.db, "stock_account_rules").await.expect("id на правило");
    invoice_read::exe(
        &st.db,
        &format!(
            "INSERT INTO stock_account_rules (id, company_id, kind, inventory_account_id, offset_account_id, clearing_account_id) \
             VALUES ({id}, {company}, 'goods', {inventory}, {offset}, 0)"
        ),
    )
    .await
    .expect("правило за стоки");
    Some(id)
}

async fn purge(st: &AppState) {
    for number in MARK_NOS {
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
        &format!(
            "SELECT id FROM products WHERE code = {} OR code = {} OR code = {}",
            db::lit(MARK_CODES[0]),
            db::lit(MARK_CODES[1]),
            db::lit(MARK_CODES[2])
        ),
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
    if let Ok(company) = form::active_or_one(st, "superadmin").await {
        let rows = invoice_read::qry(
            &st.db,
            &format!(
                "SELECT id FROM journal_entries WHERE company_id = {company} AND document_type = 'stock_writeoff' \
                 AND document_number = {}",
                db::lit("2032-02")
            ),
        )
        .await
        .unwrap_or_default();
        for row in rows {
            let _ = invoice_read::wipe_journal(&st.db, db::cell_i32(&row)).await;
        }
        stock_replay::rebuild(&st.db, company, "9999-12-31").await.expect("преизчисление след теста");
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
    date: &str,
) -> i32 {
    let body = json!({
        "direction": direction,
        "document_type": doc_type,
        "number": number,
        "issue_date": date,
        "due_date": date,
        "accounting_month": &date[..7],
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
/// 40 + ДДС 8 = 48). Себестойността не е на фактурата. След покупката
/// остават 10 бр. за 50. След продажбата 6 бр. за 30. Отпостът на
/// покупката минава: картичката се сглобява наново и става нула.
#[tokio::test]
async fn fiscal_post_and_unpost_wac() {
    let _lock = LOCK.lock().await;
    let Some(f) = fixture().await else { return };

    let buy = create_invoice(&f, BUY_NO, "in", "11", "10", "5.00", "50.00", "10.00", "60.00", "2031-01-10").await;
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

    let sell = create_invoice(&f, SELL_NO, "out", "01", "4", "10.00", "40.00", "8.00", "48.00", "2031-01-10").await;
    invoice_post::post_one(&f.st, f.company, sell).await.expect("постът на продажбата");
    assert_journal(
        &f,
        sell,
        &[
            ("411", "debit", 4800),
            ("702", "credit", 4000),
            ("4532", "credit", 800),
        ],
    )
    .await;
    assert_stock(&f, "6", "30").await;

    invoice_post::unpost_one(&f.st, f.company, sell).await.expect("връщането на продажбата");
    assert_journal(&f, sell, &[]).await;
    assert_stock(&f, "10", "50").await;
    invoice_post::unpost_one(&f.st, f.company, buy).await.expect("връщането на покупката");
    assert_journal(&f, buy, &[]).await;
    assert_stock(&f, "0", "0").await;

    purge(&f.st).await;
}

/// ДДС дневникът и файловете за НАП събират постнатите фактури на месеца.
#[tokio::test]
async fn fiscal_vat_register_and_files() {
    let _lock = LOCK.lock().await;
    let Some(f) = fixture().await else { return };

    let buy = create_invoice(&f, BUY_NO, "in", "11", "10", "5.00", "50.00", "10.00", "60.00", "2031-01-10").await;
    let sell = create_invoice(&f, SELL_NO, "out", "01", "4", "10.00", "40.00", "8.00", "48.00", "2031-01-10").await;
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

fn with_product(f: &Fixture, product: i32) -> Fixture {
    Fixture {
        st: f.st.clone(),
        headers: f.headers.clone(),
        company: f.company,
        counterpart: f.counterpart,
        product,
    }
}

async fn insert_goods(f: &Fixture, code: &str, name: &str, inventory: i32, revenue: i32) -> i32 {
    let product = invoice_read::nid(&f.st.db, "products").await.expect("id на стока");
    invoice_read::exe(
        &f.st.db,
        &format!(
            "INSERT INTO products (id, company_id, name, code, description, unit, price, cost_price, \
             vat_rate, is_service, is_inventory, inventory_account_id, revenue_account_id, \
             stock_quantity, stock_value, is_active) \
             VALUES ({product}, {}, {}, {}, '', 'C62', '10.00', '0', '20', 0, 1, {inventory}, {revenue}, '0', '0', 1)",
            f.company,
            db::lit(name),
            db::lit(code)
        ),
    )
    .await
    .expect("стока");
    product
}

async fn stock_pair(f: &Fixture, product: i32) -> (i128, i128) {
    let rows = invoice_read::qry(
        &f.st.db,
        &format!("SELECT stock_quantity, stock_value FROM products WHERE id = {product}"),
    )
    .await
    .expect("склад");
    let row = rows.first().expect("стоката липсва");
    (dec::or0(&db::cell_str(row, 0)), dec::or0(&db::cell_str(row, 1)))
}

async fn writeoff_cents(f: &Fixture, month: &str) -> Vec<(String, String, i64)> {
    let rows = invoice_read::qry(
        &f.st.db,
        &format!(
            "SELECT l.account_number, l.direction, l.amount FROM journal_lines l, journal_entries e \
             WHERE l.entry_id = e.id AND e.company_id = {} AND e.document_type = 'stock_writeoff' \
             AND e.document_number = {}",
            f.company,
            db::lit(month)
        ),
    )
    .await
    .expect("изписване");
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

/// Две правила за стоки. Продажбата влиза преди доставка с по-ранна дата.
/// След преизчислението количеството е доставка минус продажба, а изписването
/// е по доставната цена и по правилото на артикула. Вторият пуск не удвоява.
#[tokio::test]
async fn fiscal_writeoff_two_rules_out_of_order() {
    let _lock = LOCK.lock().await;
    let Some(f) = fixture().await else { return };

    let empty = stock_writeoff::run(&f.st.db, 999999, Some("2032-02"), None)
        .await
        .expect_err("фирма без правила");
    assert_eq!(empty.0, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        empty.1["detail"].as_str(),
        Some("няма правила за склада — добавете ги в настройки, таб Склад")
    );

    let a_inv = account_id(&f.st, f.company, "304").await;
    let a_off = account_id(&f.st, f.company, "702").await;
    let b_inv = account_id(&f.st, f.company, "303").await;
    let b_off = account_id(&f.st, f.company, "701").await;
    ensure_goods(&f.st, f.company, a_inv, a_off).await;
    let extra_b = ensure_goods(&f.st, f.company, b_inv, b_off).await;
    let a = insert_goods(&f, "TST-GOODS-A", "Тест Стока А", a_inv, a_off).await;
    let b = insert_goods(&f, "TST-GOODS-B", "Тест Стока Б", b_inv, b_off).await;
    let fa = with_product(&f, a);
    let fb = with_product(&f, b);

    let a_sell = create_invoice(&fa, "T-GA-SELL", "out", "01", "2", "10.00", "20.00", "4.00", "24.00", "2032-02-20").await;
    invoice_post::post_one(&f.st, f.company, a_sell).await.expect("продажба А преди доставката");
    assert_eq!(stock_pair(&f, a).await, (dec::or0("0"), dec::or0("0")));

    let b_sell = create_invoice(&fb, "T-GB-SELL", "out", "01", "1", "10.00", "10.00", "2.00", "12.00", "2032-02-18").await;
    invoice_post::post_one(&f.st, f.company, b_sell).await.expect("продажба Б преди доставката");

    let a_buy = create_invoice(&fa, "T-GA-BUY", "in", "11", "5", "4.00", "20.00", "4.00", "24.00", "2032-02-05").await;
    invoice_post::post_one(&f.st, f.company, a_buy).await.expect("доставка А с по-ранна дата");
    assert_eq!(stock_pair(&f, a).await, (dec::or0("3"), dec::or0("12")));

    let b_buy = create_invoice(&fb, "T-GB-BUY", "in", "11", "3", "6.00", "18.00", "3.60", "21.60", "2032-02-01").await;
    invoice_post::post_one(&f.st, f.company, b_buy).await.expect("доставка Б с по-ранна дата");
    assert_eq!(stock_pair(&f, b).await, (dec::or0("2"), dec::or0("12")));

    let posted = stock_writeoff::run(&f.st.db, f.company, Some("2032-02"), None).await.expect("изписване");
    assert_eq!(posted["amount"].as_str(), Some("14.00"));
    let mut lines = writeoff_cents(&f, "2032-02").await;
    lines.sort();
    let mut want = vec![
        ("702".into(), "debit".into(), 800),
        ("304".into(), "credit".into(), 800),
        ("701".into(), "debit".into(), 600),
        ("303".into(), "credit".into(), 600),
    ];
    want.sort();
    assert_eq!(lines, want, "изписването не е по правилата на артикулите");

    let again = stock_writeoff::run(&f.st.db, f.company, Some("2032-02"), None).await.expect("повторно изписване");
    assert_eq!(again["amount"].as_str(), Some("14.00"));
    let entries = invoice_read::qry(
        &f.st.db,
        &format!(
            "SELECT id FROM journal_entries WHERE company_id = {} AND document_type = 'stock_writeoff' AND document_number = {}",
            f.company,
            db::lit("2032-02")
        ),
    )
    .await
    .expect("запис");
    assert_eq!(entries.len(), 1, "повторният пуск удвоява записа");
    let mut again_lines = writeoff_cents(&f, "2032-02").await;
    again_lines.sort();
    assert_eq!(again_lines, want);

    if let Some(id) = extra_b {
        let _ = invoice_read::exe(&f.st.db, &format!("DELETE FROM stock_account_rules WHERE id = {id}")).await;
    }
    purge(&f.st).await;
}

