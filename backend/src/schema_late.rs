//! Таблици след фаза 7. Octo няма ALTER TABLE, затова новите са отделен CREATE.

pub const SQL: &str = "
CREATE TABLE IF NOT EXISTS roles (
    id INTEGER PRIMARY KEY,
    name VARCHAR,
    description VARCHAR,
    permissions VARCHAR,
    created_at VARCHAR
);
CREATE TABLE IF NOT EXISTS user_companies (
    id INTEGER PRIMARY KEY,
    user_id INTEGER,
    company_id INTEGER,
    role_id INTEGER,
    created_at VARCHAR
);
CREATE TABLE IF NOT EXISTS vat_manual (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    period VARCHAR,
    cell_33 VARCHAR,
    cell_70 VARCHAR,
    cell_71 VARCHAR,
    cell_80 VARCHAR,
    cell_81 VARCHAR,
    cell_82 VARCHAR
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
    company_id INTEGER,
    counterpart_id INTEGER,
    scanned_name VARCHAR,
    product_id INTEGER,
    created_at VARCHAR
);
CREATE TABLE IF NOT EXISTS stock_account_rules (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    kind VARCHAR,
    inventory_account_id INTEGER,
    offset_account_id INTEGER,
    clearing_account_id INTEGER
);
CREATE TABLE IF NOT EXISTS material_issues (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    number VARCHAR,
    doc_date VARCHAR,
    location_id INTEGER,
    status VARCHAR,
    notes VARCHAR,
    journal_entry_id INTEGER
);
CREATE TABLE IF NOT EXISTS material_issue_lines (
    id INTEGER PRIMARY KEY,
    doc_id INTEGER,
    product_id INTEGER,
    quantity VARCHAR,
    unit_cost VARCHAR,
    amount VARCHAR,
    account_id INTEGER,
    inventory_account_id INTEGER,
    line_no INTEGER
);
CREATE TABLE IF NOT EXISTS invoice_flags (
    id INTEGER PRIMARY KEY,
    exclude_vat INTEGER,
    vat_article VARCHAR,
    customs_statistical VARCHAR,
    customs_duty VARCHAR,
    customs_vat VARCHAR
);
";
