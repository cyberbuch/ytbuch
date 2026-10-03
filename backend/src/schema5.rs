//! Таблици за фаза 5. Нови са, затова няма ALTER.

pub const SQL: &str = r#"
CREATE TABLE IF NOT EXISTS bank_accounts (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    iban VARCHAR,
    bic VARCHAR,
    bank_name VARCHAR,
    currency VARCHAR,
    name VARCHAR,
    account_number VARCHAR,
    gl_account_id INTEGER,
    buffer_account_id INTEGER,
    balance VARCHAR
);
CREATE TABLE IF NOT EXISTS bank_transactions (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    bank_account_id INTEGER,
    txn_date VARCHAR,
    amount VARCHAR,
    currency VARCHAR,
    counterpart_name VARCHAR,
    counterpart_iban VARCHAR,
    description VARCHAR,
    reference VARCHAR,
    transaction_type VARCHAR,
    is_booked INTEGER,
    is_allocated INTEGER,
    journal_entry_id INTEGER
);
CREATE TABLE IF NOT EXISTS fixed_asset_categories (
    id INTEGER PRIMARY KEY,
    name VARCHAR,
    cita_category VARCHAR,
    min_depreciation_rate VARCHAR,
    max_depreciation_rate VARCHAR,
    default_method VARCHAR,
    asset_account_id INTEGER,
    depreciation_account_id INTEGER,
    expense_account_id INTEGER
);
CREATE TABLE IF NOT EXISTS fixed_assets (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    name VARCHAR,
    inventory_number VARCHAR,
    category_id INTEGER,
    acquisition_date VARCHAR,
    put_into_service_date VARCHAR,
    cost VARCHAR,
    salvage_value VARCHAR,
    useful_life_months VARCHAR,
    depreciation_method VARCHAR,
    accounting_depreciation_rate VARCHAR,
    tax_depreciation_rate VARCHAR,
    accumulated_depreciation VARCHAR,
    tax_accumulated_depreciation VARCHAR,
    status VARCHAR,
    serial_number VARCHAR,
    manufacturer VARCHAR,
    model VARCHAR,
    responsible_person VARCHAR,
    is_conserved INTEGER,
    location_id INTEGER
);
CREATE TABLE IF NOT EXISTS fixed_asset_events (
    id INTEGER PRIMARY KEY,
    asset_id INTEGER,
    event_type VARCHAR,
    event_date VARCHAR,
    amount VARCHAR,
    prev_value VARCHAR,
    location VARCHAR,
    reason VARCHAR
);
CREATE TABLE IF NOT EXISTS inventory_lots (
    id INTEGER PRIMARY KEY,
    product_id INTEGER,
    invoice_id INTEGER,
    invoice_line_id INTEGER,
    received_date VARCHAR,
    quantity VARCHAR,
    quantity_remaining VARCHAR,
    unit_cost VARCHAR,
    location_id INTEGER
);
CREATE TABLE IF NOT EXISTS internal_docs (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    number VARCHAR,
    doc_date VARCHAR,
    from_location_id INTEGER,
    to_location_id INTEGER,
    status VARCHAR,
    notes VARCHAR,
    handed_by VARCHAR,
    received_by VARCHAR
);
CREATE TABLE IF NOT EXISTS internal_doc_lines (
    id INTEGER PRIMARY KEY,
    doc_id INTEGER,
    line_kind VARCHAR,
    product_id INTEGER,
    asset_id INTEGER,
    quantity VARCHAR,
    unit_cost VARCHAR,
    amount VARCHAR,
    description VARCHAR
);
CREATE TABLE IF NOT EXISTS recipes (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    code VARCHAR,
    name VARCHAR,
    output_product_id INTEGER,
    output_quantity VARCHAR,
    output_unit VARCHAR,
    notes VARCHAR,
    is_active INTEGER
);
CREATE TABLE IF NOT EXISTS recipe_lines (
    id INTEGER PRIMARY KEY,
    recipe_id INTEGER,
    product_id INTEGER,
    quantity VARCHAR,
    unit VARCHAR,
    wastage_percent VARCHAR,
    line_no INTEGER
);
CREATE TABLE IF NOT EXISTS production_orders (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    number VARCHAR,
    recipe_id INTEGER,
    output_product_id INTEGER,
    quantity VARCHAR,
    location_id INTEGER,
    order_date VARCHAR,
    status VARCHAR,
    notes VARCHAR,
    material_cost VARCHAR,
    journal_entry_id INTEGER
);
CREATE TABLE IF NOT EXISTS production_order_lines (
    id INTEGER PRIMARY KEY,
    order_id INTEGER,
    product_id INTEGER,
    product_name VARCHAR,
    quantity VARCHAR,
    unit VARCHAR,
    unit_cost VARCHAR,
    amount VARCHAR
);
CREATE TABLE IF NOT EXISTS dividend_distributions (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    year INTEGER,
    total_amount VARCHAR,
    decision_date VARCHAR,
    decision_number VARCHAR,
    status VARCHAR,
    notes VARCHAR
);
CREATE TABLE IF NOT EXISTS dividends (
    id INTEGER PRIMARY KEY,
    company_id INTEGER,
    distribution_id INTEGER,
    beneficial_owner_id INTEGER,
    owner_name VARCHAR,
    owner_egn VARCHAR,
    ownership_percentage VARCHAR,
    gross_amount VARCHAR,
    tax_rate VARCHAR,
    tax_amount VARCHAR,
    net_amount VARCHAR,
    decision_date VARCHAR,
    payment_date VARCHAR,
    is_paid INTEGER,
    notes VARCHAR
);
"#;
