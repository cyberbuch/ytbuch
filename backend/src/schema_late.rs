//! Таблици след фаза 7. Octo няма ALTER TABLE, затова новите са отделен CREATE.

pub const SQL: &str = "
CREATE TABLE IF NOT EXISTS roles (
    id INTEGER PRIMARY KEY,
    name VARCHAR,
    description VARCHAR,
    permissions VARCHAR,
    created_at VARCHAR
);
CREATE TABLE IF NOT EXISTS exchange_rates (
    id INTEGER PRIMARY KEY,
    currency_code VARCHAR,
    rate VARCHAR,
    rate_date VARCHAR,
    created_at VARCHAR
);
CREATE TABLE IF NOT EXISTS saft_account_mappings (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    kind VARCHAR,
    type_code VARCHAR,
    debit_account VARCHAR,
    credit_account VARCHAR,
    debit_analytical VARCHAR,
    credit_analytical VARCHAR,
    description VARCHAR,
    is_active INTEGER
);
CREATE TABLE IF NOT EXISTS product_name_mappings (
    id INTEGER PRIMARY KEY,
    counterpart_id INTEGER,
    scanned_name VARCHAR,
    product_id INTEGER,
    created_at VARCHAR
);
";
