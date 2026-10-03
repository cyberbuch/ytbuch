//! Връзка към Octo и схемата на първата фаза.
//! Octo няма CREATE INDEX и не записва вътре в транзакция.
//! Разширеният протокол (Parse/Bind) забива при INSERT, затова заявките са simple query.

use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls, SimpleQueryMessage};

pub type DbResult<T> = Result<T, tokio_postgres::Error>;

/// Една връзка към Octo. Заявките се пускат една по една:
/// паралелен simple query забива протокола.
pub struct Db {
    inner: Mutex<Client>,
}

impl Db {
    pub fn new(client: Client) -> Self {
        Self {
            inner: Mutex::new(client),
        }
    }

    pub async fn query(&self, sql: &str) -> DbResult<Vec<Row>> {
        let client = self.inner.lock().await;
        query(&client, sql).await
    }

    pub async fn exec(&self, sql: &str) -> DbResult<u64> {
        let client = self.inner.lock().await;
        exec(&client, sql).await
    }

    pub async fn next_id(&self, table: &str) -> DbResult<i32> {
        let client = self.inner.lock().await;
        next_id(&client, table).await
    }
}

pub struct Row {
    cols: Vec<String>,
}

pub async fn connect(url: &str) -> DbResult<Client> {
    let (client, connection) = tokio_postgres::connect(url, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("octo connection closed: {e}");
        }
    });
    Ok(client)
}

pub async fn migrate(db: &Client) -> DbResult<()> {
    db.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            id INTEGER PRIMARY KEY,
            name VARCHAR
        );
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            email VARCHAR,
            name VARCHAR,
            password_hash VARCHAR,
            role VARCHAR,
            is_active INTEGER,
            is_super_admin INTEGER,
            role_id INTEGER,
            totp_enabled INTEGER,
            created_at VARCHAR
        );
        CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY,
            setting_key VARCHAR,
            value VARCHAR
        );
        CREATE TABLE IF NOT EXISTS companies (
            id INTEGER PRIMARY KEY,
            name VARCHAR,
            eik VARCHAR,
            vat_number VARCHAR,
            is_vat_registered INTEGER,
            vat_period VARCHAR,
            currency VARCHAR,
            address VARCHAR,
            city VARCHAR,
            post_code VARCHAR,
            phone VARCHAR,
            email VARCHAR,
            website VARCHAR,
            mol VARCHAR,
            manager_eik VARCHAR,
            accountant_name VARCHAR,
            accountant_egn VARCHAR,
            tax_authority VARCHAR,
            nap_office VARCHAR,
            iban VARCHAR,
            bic VARCHAR,
            inventory_valuation_method VARCHAR,
            fiscal_year_start_month INTEGER,
            country VARCHAR,
            manager_name VARCHAR,
            manager_egn VARCHAR,
            authorized_person_name VARCHAR,
            authorized_person_egn VARCHAR,
            vat_branch_number VARCHAR,
            tax_accounting_basis VARCHAR,
            tax_entity VARCHAR,
            software_company_name VARCHAR,
            software_id VARCHAR,
            software_version VARCHAR,
            is_part_of_group VARCHAR,
            street_name VARCHAR,
            building_number VARCHAR,
            address_building VARCHAR,
            additional_address_detail VARCHAR,
            address_type VARCHAR,
            region VARCHAR,
            settings VARCHAR,
            created_at VARCHAR
        );
        CREATE TABLE IF NOT EXISTS company_locations (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            name VARCHAR,
            location_type VARCHAR,
            street_name VARCHAR,
            building_number VARCHAR,
            city VARCHAR,
            post_code VARCHAR,
            region VARCHAR,
            country VARCHAR,
            is_main INTEGER
        );
        CREATE TABLE IF NOT EXISTS beneficial_owners (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            first_name_bg VARCHAR,
            last_name_bg VARCHAR,
            egn VARCHAR,
            first_name_latin VARCHAR,
            last_name_latin VARCHAR,
            country VARCHAR,
            ownership_percentage VARCHAR
        );
        CREATE TABLE IF NOT EXISTS ultimate_parents (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            name_bg VARCHAR,
            uic VARCHAR,
            name_latin VARCHAR,
            country VARCHAR
        );
        CREATE TABLE IF NOT EXISTS document_series (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            name VARCHAR,
            document_types VARCHAR,
            pad_width INTEGER,
            start_number INTEGER,
            next_number VARCHAR
        );
        CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            number VARCHAR,
            name VARCHAR,
            saft_account_type VARCHAR,
            analytic_type VARCHAR
        );
        CREATE TABLE IF NOT EXISTS counterparts (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            name VARCHAR,
            counterpart_type VARCHAR,
            eik VARCHAR,
            vat_number VARCHAR,
            address VARCHAR,
            city VARCHAR,
            post_code VARCHAR,
            country VARCHAR,
            contact_person VARCHAR
        );
        CREATE TABLE IF NOT EXISTS counterpart_extra (
            id INTEGER PRIMARY KEY,
            vies_address VARCHAR,
            street_name VARCHAR,
            building_number VARCHAR,
            address_building VARCHAR,
            additional_address_detail VARCHAR,
            address_type VARCHAR,
            region VARCHAR,
            email VARCHAR,
            phone VARCHAR
        );
        CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            name VARCHAR,
            code VARCHAR,
            description VARCHAR,
            unit VARCHAR,
            price VARCHAR,
            cost_price VARCHAR,
            vat_rate VARCHAR,
            is_service INTEGER,
            is_inventory INTEGER,
            inventory_account_id INTEGER,
            revenue_account_id INTEGER,
            stock_quantity VARCHAR,
            stock_value VARCHAR,
            is_active INTEGER
        );
        CREATE TABLE IF NOT EXISTS invoices (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            direction VARCHAR,
            document_type VARCHAR,
            number VARCHAR,
            issue_date VARCHAR,
            due_date VARCHAR,
            counterpart_id INTEGER,
            counterpart_name VARCHAR,
            currency VARCHAR,
            net_amount VARCHAR,
            vat_amount VARCHAR,
            total_amount VARCHAR,
            status VARCHAR,
            payment_method VARCHAR,
            notes VARCHAR
        );
        CREATE TABLE IF NOT EXISTS invoice_lines (
            id INTEGER PRIMARY KEY,
            invoice_id INTEGER,
            code VARCHAR,
            description VARCHAR,
            quantity VARCHAR,
            unit VARCHAR,
            unit_price VARCHAR,
            vat_rate VARCHAR,
            net_amount VARCHAR,
            vat_amount VARCHAR,
            total_amount VARCHAR
        );
        CREATE TABLE IF NOT EXISTS invoice_extra (
            id INTEGER PRIMARY KEY,
            tax_event_date VARCHAR,
            accounting_month VARCHAR,
            currency_rate VARCHAR,
            discount_percent VARCHAR,
            discount_amount VARCHAR,
            vat_exemption_reason VARCHAR,
            original_invoice_id INTEGER,
            location_id INTEGER,
            journal_entry_id INTEGER
        );
        CREATE TABLE IF NOT EXISTS invoice_line_extra (
            id INTEGER PRIMARY KEY,
            product_id INTEGER
        );
        CREATE TABLE IF NOT EXISTS journal_entries (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            entry_date VARCHAR,
            document_type VARCHAR,
            document_id INTEGER,
            document_number VARCHAR,
            description VARCHAR,
            vat_type VARCHAR,
            counterpart_id INTEGER,
            counterpart_name VARCHAR,
            document_date VARCHAR,
            vat_period VARCHAR
        );
        CREATE TABLE IF NOT EXISTS journal_lines (
            id INTEGER PRIMARY KEY,
            entry_id INTEGER,
            account_id INTEGER,
            account_number VARCHAR,
            account_name VARCHAR,
            direction VARCHAR,
            amount VARCHAR,
            vat_amount VARCHAR
        );
        CREATE TABLE IF NOT EXISTS user_login (
            id INTEGER PRIMARY KEY,
            username VARCHAR,
            totp_secret_b32 VARCHAR,
            totp_pending_b32 VARCHAR
        );
        CREATE TABLE IF NOT EXISTS user_recovery_codes (
            id INTEGER PRIMARY KEY,
            user_id INTEGER,
            code_hash VARCHAR,
            used INTEGER
        );
        CREATE TABLE IF NOT EXISTS accounting_periods (
            id INTEGER PRIMARY KEY,
            year INTEGER,
            month INTEGER,
            status VARCHAR,
            closed_at VARCHAR,
            notes VARCHAR
        );
        CREATE TABLE IF NOT EXISTS opening_balances (
            id INTEGER PRIMARY KEY,
            fiscal_year INTEGER,
            account_id INTEGER,
            debit VARCHAR,
            credit VARCHAR,
            counterpart_id INTEGER,
            product_id INTEGER,
            quantity VARCHAR,
            description VARCHAR
        );
        ",
    )
    .await?;
    exec(db, crate::schema5::SQL).await?;
    let n = query(db, "SELECT id FROM schema_migrations WHERE id = 1").await?;
    if n.is_empty() {
        exec(
            db,
            "INSERT INTO schema_migrations (id, name) VALUES (1, 'phase1_auth_companies')",
        )
        .await?;
    }
    seed_demo(db).await?;
    Ok(())
}

async fn seed_demo(db: &Client) -> DbResult<()> {
    let n = query(db, "SELECT id FROM schema_migrations WHERE id = 2").await?;
    if !n.is_empty() {
        return Ok(());
    }
    let companies = query(db, "SELECT id FROM companies WHERE id = 1").await?;
    if companies.is_empty() {
        exec(
            db,
            "INSERT INTO companies (id, name, eik, is_vat_registered, currency, country, vat_period, fiscal_year_start_month, inventory_valuation_method, created_at) \
             VALUES (1, 'Бага ООД', '207123456', 1, 'EUR', 'BG', 'monthly', 1, 'WAC', '2026-10-01T00:00:00Z')",
        )
        .await?;
    }
    exec(
        db,
        "UPDATE companies SET name = 'Бага ООД', eik = '207123456', vat_number = 'BG207123456', \
         is_vat_registered = 1, vat_period = 'monthly', currency = 'EUR', country = 'BG', \
         city = 'София', post_code = '1000', address = 'бул. Витоша 1', phone = '02 000 0000', \
         email = 'office@baga.example', mol = 'Иван Петров', manager_name = 'Иван Петров', \
         street_name = 'бул. Витоша', building_number = '1', region = 'София', \
         tax_accounting_basis = 'A', nap_office = 'ТД на НАП София', \
         iban = 'BG80BNBG96611020345678', bic = 'BNBGBGSD', \
         inventory_valuation_method = 'WAC', fiscal_year_start_month = 1 \
         WHERE id = 1",
    )
    .await?;
    let key = lit("активна_фирма:superadmin");
    let setting = query(
        db,
        &format!("SELECT id FROM settings WHERE setting_key = {key}"),
    )
    .await?;
    if let Some(row) = setting.first() {
        let id = cell_i32(row);
        exec(db, &format!("UPDATE settings SET value = '1' WHERE id = {id}")).await?;
    } else {
        let id = next_id(db, "settings").await?;
        exec(
            db,
            &format!(
                "INSERT INTO settings (id, setting_key, value) VALUES ({id}, {key}, '1')"
            ),
        )
        .await?;
    }
    exec(
        db,
        "INSERT INTO company_locations (id, company_id, name, location_type, street_name, building_number, city, post_code, region, country, is_main) \
         VALUES (1, 1, 'Офис Витоша', 'OFFICE', 'бул. Витоша', '1', 'София', '1000', 'София', 'BG', 1)",
    )
    .await?;
    exec(
        db,
        "INSERT INTO company_locations (id, company_id, name, location_type, street_name, building_number, city, post_code, region, country, is_main) \
         VALUES (2, 1, 'Склад Младост', 'WAREHOUSE', 'ул. Пример', '12', 'София', '1784', 'София', 'BG', 0)",
    )
    .await?;
    exec(
        db,
        "INSERT INTO beneficial_owners (id, company_id, first_name_bg, last_name_bg, egn, first_name_latin, last_name_latin, country, ownership_percentage) \
         VALUES (1, 1, 'Иван', 'Петров', '8001010000', 'Ivan', 'Petrov', 'BG', '100.00')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO ultimate_parents (id, company_id, name_bg, uic, name_latin, country) \
         VALUES (1, 1, 'Бага Холдинг ЕАД', '175000001', 'Baga Holding', 'BG')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO document_series (id, company_id, name, document_types, pad_width, start_number, next_number) \
         VALUES (1, 1, 'Фактури', '01,02,03', 10, 1, '0000000003')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO accounts (id, company_id, number, name, saft_account_type, analytic_type) VALUES \
         (1, 1, '302', 'Материали', 'Asset', 'none'), \
         (2, 1, '401', 'Доставчици', 'Liability', 'counterpart'), \
         (3, 1, '411', 'Клиенти', 'Asset', 'counterpart'), \
         (4, 1, '4531', 'Начислен ДДС', 'Liability', 'none'), \
         (5, 1, '501', 'Каса в лева', 'Asset', 'none'), \
         (6, 1, '601', 'Разходи за материали', 'Expense', 'none'), \
         (7, 1, '702', 'Приходи от продажби', 'Revenue', 'none')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO counterparts (id, company_id, name, counterpart_type, eik, vat_number, address, city, post_code, country, contact_person) VALUES \
         (1, 1, 'Клиент Демо ЕООД', 'customer', '111111111', 'BG111111111', 'ул. Клиент 2', 'Пловдив', '4000', 'BG', 'Мария Георгиева'), \
         (2, 1, 'Доставчик Демо ООД', 'supplier', '222222222', 'BG222222222', 'ул. Доставчик 3', 'Варна', '9000', 'BG', 'Георги Димитров')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO products (id, company_id, name, code, description, unit, price, cost_price, vat_rate, is_service, is_inventory, inventory_account_id, revenue_account_id, stock_quantity, stock_value, is_active) \
         VALUES (1, 1, 'Консултация', 'SRV-1', 'Счетоводна консултация', 'HUR', '100.00', '0.00', '20', 1, 0, 0, 7, '0', '0.00', 1)",
    )
    .await?;
    exec(
        db,
        "INSERT INTO invoices (id, company_id, direction, document_type, number, issue_date, due_date, counterpart_id, counterpart_name, currency, net_amount, vat_amount, total_amount, status, payment_method, notes) VALUES \
         (1, 1, 'out', '01', '0000000001', '2026-10-01', '2026-10-15', 1, 'Клиент Демо ЕООД', 'EUR', '1000.00', '200.00', '1200.00', 'posted', 'bank', 'Демо продажба'), \
         (2, 1, 'in', '01', '0000000042', '2026-10-02', '2026-10-16', 2, 'Доставчик Демо ООД', 'EUR', '250.00', '50.00', '300.00', 'draft', 'bank', 'Демо покупка')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO invoice_lines (id, invoice_id, code, description, quantity, unit, unit_price, vat_rate, net_amount, vat_amount, total_amount) VALUES \
         (1, 1, 'SRV-1', 'Счетоводна консултация', '10', 'HUR', '100.00', '20', '1000.00', '200.00', '1200.00'), \
         (2, 2, 'MAT-1', 'Канцеларски материали', '1', 'C62', '250.00', '20', '250.00', '50.00', '300.00')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO journal_entries (id, company_id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period) VALUES \
         (1, 1, '2026-10-01', '01', 1, '0000000001', 'Продажба към Клиент Демо ЕООД', 'sales', 1, 'Клиент Демо ЕООД', '2026-10-01', '2026-10'), \
         (2, 1, '2026-10-02', '01', 2, '0000000042', 'Покупка от Доставчик Демо ООД', 'purchase', 2, 'Доставчик Демо ООД', '2026-10-02', '2026-10')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO journal_lines (id, entry_id, account_id, account_number, account_name, direction, amount, vat_amount) VALUES \
         (1, 1, 3, '411', 'Клиенти', 'debit', '1200.00', '0.00'), \
         (2, 1, 7, '702', 'Приходи от продажби', 'credit', '1000.00', '0.00'), \
         (3, 1, 4, '4531', 'Начислен ДДС', 'credit', '200.00', '200.00'), \
         (4, 2, 6, '601', 'Разходи за материали', 'debit', '250.00', '0.00'), \
         (5, 2, 4, '4531', 'Начислен ДДС', 'debit', '50.00', '50.00'), \
         (6, 2, 2, '401', 'Доставчици', 'credit', '300.00', '0.00')",
    )
    .await?;
    exec(
        db,
        "INSERT INTO schema_migrations (id, name) VALUES (2, 'demo_company')",
    )
    .await?;
    Ok(())
}

pub async fn query(db: &Client, sql: &str) -> DbResult<Vec<Row>> {
    let msgs = db.simple_query(sql).await?;
    let mut rows = Vec::new();
    for msg in msgs {
        if let SimpleQueryMessage::Row(row) = msg {
            let cols = (0..row.len())
                .map(|i| row.get(i).unwrap_or("").to_string())
                .collect();
            rows.push(Row { cols });
        }
    }
    Ok(rows)
}

pub async fn exec(db: &Client, sql: &str) -> DbResult<u64> {
    let msgs = db.simple_query(sql).await?;
    let mut n = 0;
    for msg in msgs {
        if let SimpleQueryMessage::CommandComplete(c) = msg {
            n = c;
        }
    }
    Ok(n)
}

pub async fn next_id(db: &Client, table: &str) -> DbResult<i32> {
    let sql = format!("SELECT MAX(id) FROM {table}");
    let rows = query(db, &sql).await?;
    let max = rows.first().map(cell_i32).unwrap_or(0);
    Ok(max + 1)
}

/// SQL низ в единични кавички. Единичната кавичка се удвоява.
pub fn lit(value: &str) -> String {
    let mut out = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

pub fn cell_str(row: &Row, idx: usize) -> String {
    row.cols.get(idx).cloned().unwrap_or_default()
}

pub fn cell_i32(row: &Row) -> i32 {
    cell_i32_at(row, 0)
}

pub fn cell_i32_at(row: &Row, idx: usize) -> i32 {
    cell_str(row, idx).parse().unwrap_or(0)
}

pub fn now_stamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
