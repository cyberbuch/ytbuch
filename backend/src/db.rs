//! Връзки към Octo и схемата на първата фаза.
//! Octo няма CREATE INDEX и не записва вътре в транзакция.
//! Заявките са simple query за съвместимост с използваната версия на Octo.
//! Мъртва връзка се отваря наново; запис не се повтаря, за да не се дублира.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls, SimpleQueryMessage};

pub type DbResult<T> = Result<T, tokio_postgres::Error>;

pub struct Db {
    url: String,
    clients: Vec<Mutex<Option<Client>>>,
    next_client: AtomicUsize,
}

impl Db {
    pub async fn connect(url: &str, pool_size: usize) -> DbResult<Self> {
        let mut clients = Vec::with_capacity(pool_size);
        clients.push(Mutex::new(Some(dial(url).await?)));
        for _ in 1..pool_size {
            clients.push(Mutex::new(Some(connect(url).await?)));
        }
        Ok(Self {
            url: url.to_string(),
            clients,
            next_client: AtomicUsize::new(0),
        })
    }

    fn slot(&self) -> &Mutex<Option<Client>> {
        let index = self.next_client.fetch_add(1, Ordering::Relaxed) % self.clients.len();
        &self.clients[index]
    }

    async fn ensure(slot: &mut Option<Client>, url: &str) -> DbResult<()> {
        let dead = match slot {
            Some(client) => client.is_closed(),
            None => true,
        };
        if dead {
            *slot = Some(connect(url).await?);
        }
        Ok(())
    }

    pub async fn migrate(&self) -> DbResult<()> {
        let mut slot = self.clients[0].lock().await;
        Self::ensure(&mut slot, &self.url).await?;
        let client = slot.as_ref().expect("ensure sets the client");
        match migrate(client).await {
            Err(e) if client.is_closed() => Err(e),
            other => other,
        }
    }

    pub async fn query(&self, sql: &str) -> DbResult<Vec<Row>> {
        let mut slot = self.slot().lock().await;
        for attempt in 0..2 {
            Self::ensure(&mut slot, &self.url).await?;
            let client = slot.as_ref().expect("ensure sets the client");
            let result = query(client, sql).await;
            let closed = client.is_closed();
            match result {
                Ok(rows) => return Ok(rows),
                Err(_) if closed && attempt == 0 => {
                    tracing::warn!("octo: reconnecting after closed connection");
                    *slot = None;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!("two query attempts")
    }

    pub async fn exec(&self, sql: &str) -> DbResult<u64> {
        let mut slot = self.slot().lock().await;
        Self::ensure(&mut slot, &self.url).await?;
        let client = slot.as_ref().expect("ensure sets the client");
        let result = exec(client, sql).await;
        if result.is_err() && client.is_closed() {
            // Записът не се повтаря, защото може вече да е минал в базата.
            *slot = None;
        }
        result
    }

    pub async fn next_id(&self, table: &str) -> DbResult<i32> {
        let mut slot = self.slot().lock().await;
        for attempt in 0..2 {
            Self::ensure(&mut slot, &self.url).await?;
            let client = slot.as_ref().expect("ensure sets the client");
            let result = next_id(client, table).await;
            let closed = client.is_closed();
            match result {
                Ok(id) => return Ok(id),
                Err(_) if closed && attempt == 0 => {
                    tracing::warn!("octo: reconnecting after closed connection");
                    *slot = None;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!("two next_id attempts")
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

/// Връзка с повторения, докато YottaDB се вдигне. За първата връзка на пула.
async fn dial(url: &str) -> DbResult<Client> {
    let mut last = None;
    for _ in 0..20 {
        match connect(url).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                tracing::warn!("octo: няма връзка, опитвам пак след 3s: {e}");
                last = Some(e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
    Err(last.expect("at least one attempt"))
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
        CREATE TABLE IF NOT EXISTS password_reset_tokens (
            id INTEGER PRIMARY KEY,
            user_id INTEGER,
            token_hash VARCHAR,
            expires_at INTEGER,
            used INTEGER
        );
        CREATE TABLE IF NOT EXISTS accounting_periods (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
            year INTEGER,
            month INTEGER,
            status VARCHAR,
            closed_at VARCHAR,
            notes VARCHAR
        );
        CREATE TABLE IF NOT EXISTS opening_balances (
            id INTEGER PRIMARY KEY,
            company_id INTEGER,
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
    exec(db, crate::schema_late::SQL).await?;
    exec(db, crate::fsform::SQL).await?;
    insert_missing(
        db,
        "schema_migrations",
        "id, name",
        &[(1, "1, 'phase1_auth_companies'")],
    )
    .await?;
    seed_demo(db).await?;
    Ok(())
}

/// Запис само за липсващ `id`. Повторен старт довършва прекъсната стъпка.
/// Octo не записва вътре в транзакция, затова всяка миграция с данни е такава.
async fn insert_missing(db: &Client, table: &str, columns: &str, rows: &[(i32, &str)]) -> DbResult<()> {
    let mut values = Vec::new();
    for (id, vals) in rows {
        let found = query(db, &format!("SELECT id FROM {table} WHERE id = {id}")).await?;
        if found.is_empty() {
            values.push(format!("({vals})"));
        }
    }
    if values.is_empty() {
        return Ok(());
    }
    exec(
        db,
        &format!("INSERT INTO {table} ({columns}) VALUES {}", values.join(", ")),
    )
    .await?;
    Ok(())
}

async fn seed_demo(db: &Client) -> DbResult<()> {
    let n = query(db, "SELECT id FROM schema_migrations WHERE id = 2").await?;
    if !n.is_empty() {
        return Ok(());
    }
    insert_missing(
        db,
        "companies",
        "id, name, eik, is_vat_registered, currency, country, vat_period, fiscal_year_start_month, inventory_valuation_method, created_at",
        &[(1, "1, 'YT ООД', '207123456', 1, 'EUR', 'BG', 'monthly', 1, 'WAC', '2026-10-01T00:00:00Z'")],
    )
    .await?;
    exec(
        db,
        "UPDATE companies SET name = 'YT ООД', eik = '207123456', vat_number = 'BG207123456', \
         is_vat_registered = 1, vat_period = 'monthly', currency = 'EUR', country = 'BG', \
         city = 'София', post_code = '1000', address = 'бул. Витоша 1', phone = '02 000 0000', \
         email = 'office@yt.example', mol = 'Иван Петров', manager_name = 'Иван Петров', \
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
    insert_missing(
        db,
        "company_locations",
        "id, company_id, name, location_type, street_name, building_number, city, post_code, region, country, is_main",
        &[
            (1, "1, 1, 'Офис Витоша', 'OFFICE', 'бул. Витоша', '1', 'София', '1000', 'София', 'BG', 1"),
            (2, "2, 1, 'Склад Младост', 'WAREHOUSE', 'ул. Пример', '12', 'София', '1784', 'София', 'BG', 0"),
        ],
    )
    .await?;
    insert_missing(
        db,
        "beneficial_owners",
        "id, company_id, first_name_bg, last_name_bg, egn, first_name_latin, last_name_latin, country, ownership_percentage",
        &[(1, "1, 1, 'Иван', 'Петров', '8001010000', 'Ivan', 'Petrov', 'BG', '100.00'")],
    )
    .await?;
    insert_missing(
        db,
        "ultimate_parents",
        "id, company_id, name_bg, uic, name_latin, country",
        &[(1, "1, 1, 'YT Холдинг ЕАД', '175000001', 'YT Holding', 'BG'")],
    )
    .await?;
    insert_missing(
        db,
        "document_series",
        "id, company_id, name, document_types, pad_width, start_number, next_number",
        &[(1, "1, 1, 'Фактури', '01,02,03', 10, 1, '0000000003'")],
    )
    .await?;
    insert_missing(
        db,
        "accounts",
        "id, company_id, number, name, saft_account_type, analytic_type",
        &[
            (1, "1, 1, '302', 'Материали', 'Asset', 'none'"),
            (2, "2, 1, '401', 'Доставчици', 'Liability', 'counterpart'"),
            (3, "3, 1, '411', 'Клиенти', 'Asset', 'counterpart'"),
            (4, "4, 1, '4531', 'Начислен ДДС', 'Liability', 'none'"),
            (5, "5, 1, '501', 'Каса в лева', 'Asset', 'none'"),
            (6, "6, 1, '601', 'Разходи за материали', 'Expense', 'none'"),
            (7, "7, 1, '702', 'Приходи от продажби', 'Revenue', 'none'"),
        ],
    )
    .await?;
    insert_missing(
        db,
        "counterparts",
        "id, company_id, name, counterpart_type, eik, vat_number, address, city, post_code, country, contact_person",
        &[
            (1, "1, 1, 'Клиент Демо ЕООД', 'customer', '111111111', 'BG111111111', 'ул. Клиент 2', 'Пловдив', '4000', 'BG', 'Мария Георгиева'"),
            (2, "2, 1, 'Доставчик Демо ООД', 'supplier', '222222222', 'BG222222222', 'ул. Доставчик 3', 'Варна', '9000', 'BG', 'Георги Димитров'"),
        ],
    )
    .await?;
    insert_missing(
        db,
        "products",
        "id, company_id, name, code, description, unit, price, cost_price, vat_rate, is_service, is_inventory, inventory_account_id, revenue_account_id, stock_quantity, stock_value, is_active",
        &[(1, "1, 1, 'Консултация', 'SRV-1', 'Счетоводна консултация', 'HUR', '100.00', '0.00', '20', 1, 0, 0, 7, '0', '0.00', 1")],
    )
    .await?;
    insert_missing(
        db,
        "invoices",
        "id, company_id, direction, document_type, number, issue_date, due_date, counterpart_id, counterpart_name, currency, net_amount, vat_amount, total_amount, status, payment_method, notes",
        &[
            (1, "1, 1, 'out', '01', '0000000001', '2026-10-01', '2026-10-15', 1, 'Клиент Демо ЕООД', 'EUR', '1000.00', '200.00', '1200.00', 'posted', 'bank', 'Демо продажба'"),
            (2, "2, 1, 'in', '01', '0000000042', '2026-10-02', '2026-10-16', 2, 'Доставчик Демо ООД', 'EUR', '250.00', '50.00', '300.00', 'draft', 'bank', 'Демо покупка'"),
        ],
    )
    .await?;
    insert_missing(
        db,
        "invoice_lines",
        "id, invoice_id, code, description, quantity, unit, unit_price, vat_rate, net_amount, vat_amount, total_amount",
        &[
            (1, "1, 1, 'SRV-1', 'Счетоводна консултация', '10', 'HUR', '100.00', '20', '1000.00', '200.00', '1200.00'"),
            (2, "2, 2, 'MAT-1', 'Канцеларски материали', '1', 'C62', '250.00', '20', '250.00', '50.00', '300.00'"),
        ],
    )
    .await?;
    insert_missing(
        db,
        "journal_entries",
        "id, company_id, entry_date, document_type, document_id, document_number, description, vat_type, counterpart_id, counterpart_name, document_date, vat_period",
        &[
            (1, "1, 1, '2026-10-01', '01', 1, '0000000001', 'Продажба към Клиент Демо ЕООД', 'sales', 1, 'Клиент Демо ЕООД', '2026-10-01', '2026-10'"),
            (2, "2, 1, '2026-10-02', '01', 2, '0000000042', 'Покупка от Доставчик Демо ООД', 'purchase', 2, 'Доставчик Демо ООД', '2026-10-02', '2026-10'"),
        ],
    )
    .await?;
    insert_missing(
        db,
        "journal_lines",
        "id, entry_id, account_id, account_number, account_name, direction, amount, vat_amount",
        &[
            (1, "1, 1, 3, '411', 'Клиенти', 'debit', '1200.00', '0.00'"),
            (2, "2, 1, 7, '702', 'Приходи от продажби', 'credit', '1000.00', '0.00'"),
            (3, "3, 1, 4, '4531', 'Начислен ДДС', 'credit', '200.00', '200.00'"),
            (4, "4, 2, 6, '601', 'Разходи за материали', 'debit', '250.00', '0.00'"),
            (5, "5, 2, 4, '4531', 'Начислен ДДС', 'debit', '50.00', '50.00'"),
            (6, "6, 2, 2, '401', 'Доставчици', 'credit', '300.00', '0.00'"),
        ],
    )
    .await?;
    insert_missing(db, "schema_migrations", "id, name", &[(2, "2, 'demo_company'")]).await?;
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
