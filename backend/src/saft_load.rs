//! Четене за SAF-T. Не пише в базата и не пипа демо дневника.

use std::collections::HashMap;

use crate::db::{self, cell_i32_at, cell_str, Db};
use crate::form::db_err;
use crate::saft_xml::{self, money_of, pack, same_co};
use crate::ApiError;

pub struct Company {
    pub name: String,
    pub eik: String,
    pub vat: String,
    pub currency: String,
    pub city: String,
    pub post: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    pub mol: String,
    pub iban: String,
    pub fys: i32,
    pub country: String,
    pub basis: String,
    pub entity: String,
    pub soft_name: String,
    pub soft_id: String,
    pub soft_ver: String,
    pub group: String,
    pub street: String,
    pub number: String,
    pub building: String,
    pub extra: String,
    pub addr_type: String,
    pub region: String,
}

pub struct Owner {
    pub company_id: i32,
    pub first_bg: String,
    pub last_bg: String,
    pub egn: String,
    pub first_lat: String,
    pub last_lat: String,
    pub country: String,
}

pub struct Parent {
    pub company_id: i32,
    pub name_bg: String,
    pub uic: String,
    pub name_lat: String,
    pub country: String,
}

pub struct Account {
    pub id: i32,
    pub name: String,
    pub nra: String,
    pub firm: String,
}

pub struct Opening {
    pub year: i32,
    pub account_id: i32,
    pub debit: i128,
    pub credit: i128,
    pub counterpart_id: i32,
}

pub struct Entry {
    pub id: i32,
    pub company_id: i32,
    pub date: String,
    pub description: String,
    pub counterpart_id: i32,
}

pub struct Line {
    pub id: i32,
    pub entry_id: i32,
    pub account_id: i32,
    pub direction: String,
    pub amount: i128,
    pub vat: i128,
}

pub struct Party {
    pub id: i32,
    pub company_id: i32,
    pub name: String,
    pub kind: String,
    pub eik: String,
    pub city: String,
    pub post: String,
    pub country: String,
    pub street: String,
    pub number: String,
    pub building: String,
    pub extra: String,
    pub addr_type: String,
    pub region: String,
}

pub struct Product {
    pub id: i32,
    pub company_id: i32,
    pub name: String,
    pub code: String,
    pub unit: String,
    pub cost: i128,
    pub service: bool,
    pub inventory: bool,
    pub qty: String,
    pub value: i128,
}

pub struct Invoice {
    pub id: i32,
    pub company_id: i32,
    pub direction: String,
    pub document_type: String,
    pub number: String,
    pub issue_date: String,
    pub counterpart_id: i32,
    pub currency: String,
    pub net: i128,
    pub vat: i128,
    pub total: i128,
    pub accounting_month: String,
    pub tax_date: String,
    pub reason: String,
    pub journal_entry_id: i32,
}

pub struct InvLine {
    pub code: String,
    pub description: String,
    pub quantity: String,
    pub unit: String,
    pub price: i128,
    pub rate: i128,
    pub net: i128,
    pub vat: i128,
    pub product_id: i32,
}

pub struct BankTx {
    pub id: i32,
    pub company_id: i32,
    pub date: String,
    pub amount: i128,
    pub currency: String,
    pub name: String,
    pub description: String,
    pub reference: String,
    pub kind: String,
    pub booked: bool,
    pub journal_id: i32,
    pub gl: String,
}

pub struct Asset {
    pub id: i32,
    pub company_id: i32,
    pub name: String,
    pub inventory: String,
    pub category_id: i32,
    pub acquired: String,
    pub startup: String,
    pub cost: i128,
    pub life: String,
    pub method: String,
    pub rate: String,
    pub acc: i128,
    pub tax_acc: i128,
    pub tax_rate: String,
}

pub struct Snap {
    pub company_id: i32,
    pub company: Company,
    pub owners: Vec<Owner>,
    pub parents: Vec<Parent>,
    pub accounts: Vec<Account>,
    pub nra: HashMap<i32, String>,
    pub openings: Vec<Opening>,
    pub entries: Vec<Entry>,
    pub lines: HashMap<i32, Vec<Line>>,
    pub parties: Vec<Party>,
    pub products: Vec<Product>,
    pub invoices: Vec<Invoice>,
    pub inv_lines: HashMap<i32, Vec<InvLine>>,
    pub banks: Vec<BankTx>,
    pub assets: Vec<Asset>,
}

pub async fn load(db: &Db, company_id: i32) -> Result<Snap, ApiError> {
    let rows = db
        .query(&format!(
            "SELECT name, eik, vat_number, currency, city, post_code, phone, email, website, mol, iban, \
             fiscal_year_start_month, country, tax_accounting_basis, tax_entity, software_company_name, \
             software_id, software_version, is_part_of_group, street_name, building_number, address_building, \
             additional_address_detail, address_type, region FROM companies WHERE id = {company_id}"
        ))
        .await
        .map_err(db_err)?;
    let Some(c) = rows.first() else {
        return Err(crate::api_err(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "няма фирма за експорт",
        ));
    };
    let company = Company {
        name: cell_str(c, 0),
        eik: cell_str(c, 1),
        vat: cell_str(c, 2),
        currency: cell_str(c, 3),
        city: cell_str(c, 4),
        post: cell_str(c, 5),
        phone: cell_str(c, 6),
        email: cell_str(c, 7),
        website: cell_str(c, 8),
        mol: cell_str(c, 9),
        iban: cell_str(c, 10),
        fys: cell_i32_at(c, 11),
        country: cell_str(c, 12),
        basis: cell_str(c, 13),
        entity: cell_str(c, 14),
        soft_name: cell_str(c, 15),
        soft_id: cell_str(c, 16),
        soft_ver: cell_str(c, 17),
        group: cell_str(c, 18),
        street: cell_str(c, 19),
        number: cell_str(c, 20),
        building: cell_str(c, 21),
        extra: cell_str(c, 22),
        addr_type: cell_str(c, 23),
        region: cell_str(c, 24),
    };
    let owners = map_rows(
        db,
        "SELECT company_id, first_name_bg, last_name_bg, egn, first_name_latin, last_name_latin, country \
         FROM beneficial_owners ORDER BY id",
        |r| Owner {
            company_id: cell_i32_at(r, 0),
            first_bg: cell_str(r, 1),
            last_bg: cell_str(r, 2),
            egn: cell_str(r, 3),
            first_lat: cell_str(r, 4),
            last_lat: cell_str(r, 5),
            country: cell_str(r, 6),
        },
    )
    .await?;
    let parents = map_rows(
        db,
        "SELECT company_id, name_bg, uic, name_latin, country FROM ultimate_parents ORDER BY id",
        |r| Parent {
            company_id: cell_i32_at(r, 0),
            name_bg: cell_str(r, 1),
            uic: cell_str(r, 2),
            name_lat: cell_str(r, 3),
            country: cell_str(r, 4),
        },
    )
    .await?;
    let acc_rows = db
        .query("SELECT id, company_id, number, name, saft_account_type FROM accounts ORDER BY id")
        .await
        .map_err(db_err)?;
    let mut accounts = Vec::new();
    let mut nra = HashMap::new();
    for r in &acc_rows {
        if !same_co(cell_i32_at(r, 1), company_id) {
            continue;
        }
        let Some((code, firm)) = pack(&cell_str(r, 4), &cell_str(r, 2)) else {
            continue;
        };
        let id = cell_i32_at(r, 0);
        nra.insert(id, code.clone());
        accounts.push(Account {
            id,
            name: cell_str(r, 3),
            nra: code,
            firm,
        });
    }
    let openings = map_rows(
        db,
        "SELECT fiscal_year, account_id, debit, credit, counterpart_id FROM opening_balances ORDER BY id",
        |r| Opening {
            year: cell_i32_at(r, 0),
            account_id: cell_i32_at(r, 1),
            debit: money_of(&cell_str(r, 2)),
            credit: money_of(&cell_str(r, 3)),
            counterpart_id: cell_i32_at(r, 4),
        },
    )
    .await?;
    let entries = map_rows(
        db,
        "SELECT id, company_id, entry_date, description, counterpart_id FROM journal_entries ORDER BY id",
        |r| Entry {
            id: cell_i32_at(r, 0),
            company_id: cell_i32_at(r, 1),
            date: cell_str(r, 2),
            description: cell_str(r, 3),
            counterpart_id: cell_i32_at(r, 4),
        },
    )
    .await?;
    let line_rows = db
        .query(
            "SELECT id, entry_id, account_id, direction, amount, vat_amount FROM journal_lines ORDER BY id",
        )
        .await
        .map_err(db_err)?;
    let mut lines: HashMap<i32, Vec<Line>> = HashMap::new();
    for r in &line_rows {
        let line = Line {
            id: cell_i32_at(r, 0),
            entry_id: cell_i32_at(r, 1),
            account_id: cell_i32_at(r, 2),
            direction: cell_str(r, 3),
            amount: money_of(&cell_str(r, 4)),
            vat: money_of(&cell_str(r, 5)),
        };
        lines.entry(line.entry_id).or_default().push(line);
    }
    let parties = map_rows(
        db,
        "SELECT c.id, c.company_id, c.name, c.counterpart_type, c.eik, c.city, c.post_code, c.country, \
         e.street_name, e.building_number, e.address_building, e.additional_address_detail, e.address_type, e.region \
         FROM counterparts c LEFT JOIN counterpart_extra e ON c.id = e.id ORDER BY c.id",
        |r| Party {
            id: cell_i32_at(r, 0),
            company_id: cell_i32_at(r, 1),
            name: cell_str(r, 2),
            kind: cell_str(r, 3),
            eik: cell_str(r, 4),
            city: cell_str(r, 5),
            post: cell_str(r, 6),
            country: cell_str(r, 7),
            street: cell_str(r, 8),
            number: cell_str(r, 9),
            building: cell_str(r, 10),
            extra: cell_str(r, 11),
            addr_type: cell_str(r, 12),
            region: cell_str(r, 13),
        },
    )
    .await?;
    let products = map_rows(
        db,
        "SELECT id, company_id, name, code, unit, cost_price, is_service, is_inventory, stock_quantity, stock_value \
         FROM products ORDER BY id",
        |r| Product {
            id: cell_i32_at(r, 0),
            company_id: cell_i32_at(r, 1),
            name: cell_str(r, 2),
            code: cell_str(r, 3),
            unit: cell_str(r, 4),
            cost: money_of(&cell_str(r, 5)),
            service: cell_i32_at(r, 6) == 1,
            inventory: cell_i32_at(r, 7) == 1,
            qty: cell_str(r, 8),
            value: money_of(&cell_str(r, 9)),
        },
    )
    .await?;
    let invoices = map_rows(
        db,
        "SELECT i.id, i.company_id, i.direction, i.document_type, i.number, i.issue_date, i.counterpart_id, \
         i.currency, i.net_amount, i.vat_amount, i.total_amount, e.accounting_month, e.tax_event_date, \
         e.vat_exemption_reason, e.journal_entry_id \
         FROM invoices i LEFT JOIN invoice_extra e ON i.id = e.id ORDER BY i.id",
        |r| Invoice {
            id: cell_i32_at(r, 0),
            company_id: cell_i32_at(r, 1),
            direction: cell_str(r, 2),
            document_type: cell_str(r, 3),
            number: cell_str(r, 4),
            issue_date: cell_str(r, 5),
            counterpart_id: cell_i32_at(r, 6),
            currency: cell_str(r, 7),
            net: money_of(&cell_str(r, 8)),
            vat: money_of(&cell_str(r, 9)),
            total: money_of(&cell_str(r, 10)),
            accounting_month: cell_str(r, 11),
            tax_date: cell_str(r, 12),
            reason: cell_str(r, 13),
            journal_entry_id: cell_i32_at(r, 14),
        },
    )
    .await?;
    let ilines = db
        .query(
            "SELECT l.invoice_id, l.code, l.description, l.quantity, l.unit, l.unit_price, l.vat_rate, \
             l.net_amount, l.vat_amount, e.product_id \
             FROM invoice_lines l LEFT JOIN invoice_line_extra e ON l.id = e.id ORDER BY l.id",
        )
        .await
        .map_err(db_err)?;
    let mut inv_lines: HashMap<i32, Vec<InvLine>> = HashMap::new();
    for r in &ilines {
        let line = InvLine {
            code: cell_str(r, 1),
            description: cell_str(r, 2),
            quantity: cell_str(r, 3),
            unit: cell_str(r, 4),
            price: money_of(&cell_str(r, 5)),
            rate: money_of(&cell_str(r, 6)),
            net: money_of(&cell_str(r, 7)),
            vat: money_of(&cell_str(r, 8)),
            product_id: cell_i32_at(r, 9),
        };
        inv_lines.entry(cell_i32_at(r, 0)).or_default().push(line);
    }
    let bank_acc = db
        .query("SELECT b.id, a.number FROM bank_accounts b LEFT JOIN accounts a ON b.gl_account_id = a.id")
        .await
        .map_err(db_err)?;
    let mut gl_of: HashMap<i32, String> = HashMap::new();
    for r in &bank_acc {
        let num = cell_str(r, 1);
        if saft_xml::nra_ok(&num) {
            gl_of.insert(cell_i32_at(r, 0), num.trim().to_string());
        }
    }
    let tx_rows = db
        .query(
            "SELECT id, company_id, bank_account_id, txn_date, amount, currency, counterpart_name, description, \
             reference, transaction_type, is_booked, journal_entry_id FROM bank_transactions ORDER BY id",
        )
        .await
        .map_err(db_err)?;
    let mut banks = Vec::new();
    for r in &tx_rows {
        banks.push(BankTx {
            id: cell_i32_at(r, 0),
            company_id: cell_i32_at(r, 1),
            date: cell_str(r, 3),
            amount: money_of(&cell_str(r, 4)),
            currency: cell_str(r, 5),
            name: cell_str(r, 6),
            description: cell_str(r, 7),
            reference: cell_str(r, 8),
            kind: cell_str(r, 9),
            booked: cell_i32_at(r, 10) == 1,
            journal_id: cell_i32_at(r, 11),
            gl: gl_of.get(&cell_i32_at(r, 2)).cloned().unwrap_or_default(),
        });
    }
    let assets = map_rows(
        db,
        "SELECT id, company_id, name, inventory_number, category_id, acquisition_date, put_into_service_date, cost, \
         useful_life_months, depreciation_method, accounting_depreciation_rate, accumulated_depreciation, \
         tax_accumulated_depreciation, tax_depreciation_rate FROM fixed_assets ORDER BY id",
        |r| Asset {
            id: cell_i32_at(r, 0),
            company_id: cell_i32_at(r, 1),
            name: cell_str(r, 2),
            inventory: cell_str(r, 3),
            category_id: cell_i32_at(r, 4),
            acquired: cell_str(r, 5),
            startup: cell_str(r, 6),
            cost: money_of(&cell_str(r, 7)),
            life: cell_str(r, 8),
            method: cell_str(r, 9),
            rate: cell_str(r, 10),
            acc: money_of(&cell_str(r, 11)),
            tax_acc: money_of(&cell_str(r, 12)),
            tax_rate: cell_str(r, 13),
        },
    )
    .await?;
    Ok(Snap {
        company_id,
        company,
        owners,
        parents,
        accounts,
        nra,
        openings,
        entries,
        lines,
        parties,
        products,
        invoices,
        inv_lines,
        banks,
        assets,
    })
}

async fn map_rows<T>(
    db: &Db,
    sql: &str,
    f: impl Fn(&db::Row) -> T,
) -> Result<Vec<T>, ApiError> {
    let rows = db.query(sql).await.map_err(db_err)?;
    Ok(rows.iter().map(f).collect())
}
