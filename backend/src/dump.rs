//! Логически dump: INSERT редове от текущите таблици.
//! Octo няма COPY и не записва вътре в транзакция, затова файлът е поредица от INSERT.

use crate::db::{cell_i32_at, cell_str, lit, Db, Row};
use crate::form::db_err;
use crate::ApiError;

const TABLES: &[(&str, &str)] = &[
    ("users", "id:i,email,name,password_hash,role,is_active:i,is_super_admin:i,role_id:i,totp_enabled:i,created_at"),
    ("settings", "id:i,setting_key,value"),
    ("companies", "id:i,name,eik,vat_number,is_vat_registered:i,vat_period,currency,address,city,post_code,phone,email,website,mol,manager_eik,accountant_name,accountant_egn,tax_authority,nap_office,iban,bic,inventory_valuation_method,fiscal_year_start_month:i,country,manager_name,manager_egn,authorized_person_name,authorized_person_egn,vat_branch_number,tax_accounting_basis,tax_entity,software_company_name,software_id,software_version,is_part_of_group,street_name,building_number,address_building,additional_address_detail,address_type,region,settings,created_at"),
    ("company_locations", "id:i,company_id:i,name,location_type,street_name,building_number,city,post_code,region,country,is_main:i"),
    ("beneficial_owners", "id:i,company_id:i,first_name_bg,last_name_bg,egn,first_name_latin,last_name_latin,country,ownership_percentage"),
    ("ultimate_parents", "id:i,company_id:i,name_bg,uic,name_latin,country"),
    ("document_series", "id:i,company_id:i,name,document_types,pad_width:i,start_number:i,next_number"),
    ("accounts", "id:i,company_id:i,number,name,saft_account_type,analytic_type,is_currency:i"),
    ("counterparts", "id:i,company_id:i,name,counterpart_type,eik,vat_number,address,city,post_code,country,contact_person"),
    ("counterpart_extra", "id:i,vies_address,street_name,building_number,address_building,additional_address_detail,address_type,region,email,phone"),
    ("products", "id:i,company_id:i,name,code,description,unit,price,cost_price,vat_rate,is_service:i,is_inventory:i,inventory_account_id:i,revenue_account_id:i,stock_quantity,stock_value,is_active:i"),
    ("product_extra", "id:i,product_type,cn8_code,cn8_name,expense_account_id:i,auto_write_off:i"),
    ("invoices", "id:i,company_id:i,direction,document_type,number,issue_date,due_date,counterpart_id:i,counterpart_name,currency,net_amount,vat_amount,total_amount,status,payment_method,notes"),
    ("invoice_lines", "id:i,invoice_id:i,code,description,quantity,unit,unit_price,vat_rate,net_amount,vat_amount,total_amount"),
    ("invoice_extra", "id:i,tax_event_date,accounting_month,currency_rate,discount_percent,discount_amount,vat_exemption_reason,original_invoice_id:i,location_id:i,journal_entry_id:i"),
    ("invoice_line_extra", "id:i,product_id:i"),
    ("journal_entries", "id:i,company_id:i,entry_date,document_type,document_id:i,document_number,description,vat_type,counterpart_id:i,counterpart_name,document_date,vat_period"),
    ("journal_lines", "id:i,entry_id:i,account_id:i,account_number,account_name,direction,amount,vat_amount,currency,fx_quantity,fx_rate"),
    ("accounting_periods", "id:i,company_id:i,year:i,month:i,status,closed_at,notes"),
    ("opening_balances", "id:i,company_id:i,fiscal_year:i,account_id:i,debit,credit,counterpart_id:i,product_id:i,quantity,description"),
    ("bank_accounts", "id:i,company_id:i,iban,bic,bank_name,currency,name,account_number,gl_account_id:i,buffer_account_id:i,balance"),
    ("bank_transactions", "id:i,company_id:i,bank_account_id:i,txn_date,amount,currency,counterpart_name,counterpart_iban,description,reference,transaction_type,is_booked:i,is_allocated:i,journal_entry_id:i"),
    ("fixed_asset_categories", "id:i,company_id:i,name,cita_category,min_depreciation_rate,max_depreciation_rate,default_method,asset_account_id:i,depreciation_account_id:i,expense_account_id:i"),
    ("fixed_assets", "id:i,company_id:i,name,inventory_number,category_id:i,acquisition_date,put_into_service_date,cost,salvage_value,useful_life_months,depreciation_method,accounting_depreciation_rate,tax_depreciation_rate,accumulated_depreciation,tax_accumulated_depreciation,status,serial_number,manufacturer,model,responsible_person,is_conserved:i,location_id:i"),
    ("fixed_asset_events", "id:i,asset_id:i,event_type,event_date,amount,prev_value,location,reason"),
    ("inventory_lots", "id:i,product_id:i,invoice_id:i,invoice_line_id:i,received_date,quantity,quantity_remaining,unit_cost,location_id:i"),
    ("internal_docs", "id:i,company_id:i,number,doc_date,from_location_id:i,to_location_id:i,status,notes,handed_by,received_by"),
    ("internal_doc_lines", "id:i,doc_id:i,line_kind,product_id:i,asset_id:i,quantity,unit_cost,amount,description"),
    ("recipes", "id:i,company_id:i,code,name,output_product_id:i,output_quantity,output_unit,notes,is_active:i"),
    ("recipe_lines", "id:i,recipe_id:i,product_id:i,quantity,unit,wastage_percent,line_no:i"),
    ("production_orders", "id:i,company_id:i,number,recipe_id:i,output_product_id:i,quantity,location_id:i,order_date,status,notes,material_cost,journal_entry_id:i"),
    ("production_order_lines", "id:i,order_id:i,product_id:i,product_name,quantity,unit,unit_cost,amount"),
    ("stock_account_rules", "id:i,company_id:i,kind,inventory_account_id:i,offset_account_id:i,clearing_account_id:i"),
    ("material_issues", "id:i,company_id:i,number,doc_date,location_id:i,status,notes,journal_entry_id:i"),
    ("material_issue_lines", "id:i,doc_id:i,product_id:i,quantity,unit_cost,amount,account_id:i,inventory_account_id:i,line_no:i"),
    ("dividend_distributions", "id:i,company_id:i,year:i,total_amount,decision_date,decision_number,status,notes"),
    ("dividends", "id:i,company_id:i,distribution_id:i,beneficial_owner_id:i,owner_name,owner_egn,ownership_percentage,gross_amount,tax_rate,tax_amount,net_amount,decision_date,payment_date,is_paid:i,notes"),
    ("user_login", "id:i,username,totp_secret_b32,totp_pending_b32"),
    ("user_recovery_codes", "id:i,user_id:i,code_hash,used:i"),
    ("password_reset_tokens", "id:i,user_id:i,token_hash,expires_at:i,used:i"),
    ("fs_line_formulas", "id:i,company_id:i,statement,line_code,formula"),
    ("roles", "id:i,name,description,permissions,created_at"),
    ("exchange_rates", "id:i,currency_code,rate,rate_date,created_at"),
    ("saft_account_mappings", "id:i,company_id:i,kind,type_code,debit_account,credit_account,debit_analytical,credit_analytical,description,is_active:i"),
    ("product_name_mappings", "id:i,company_id:i,counterpart_id:i,scanned_name,product_id:i,created_at"),
    ("invoice_flags", "id:i,exclude_vat:i,vat_article,customs_statistical,customs_duty,customs_vat"),
    ("audit_logs", "id:i,company_id:i,user_id:i,user_login,user_name,entity_type,entity_id:i,entity_number,action,summary,old_amount,new_amount,old_data,new_data,created_at"),
];

fn columns(spec: &str) -> Vec<(&str, bool)> {
    spec.split(',')
        .map(|part| {
            let part = part.trim();
            if let Some(name) = part.strip_suffix(":i") {
                (name, true)
            } else {
                (part, false)
            }
        })
        .collect()
}

fn insert_sql(table: &str, cols: &[(&str, bool)], row: &Row) -> String {
    let names = cols.iter().map(|(name, _)| *name).collect::<Vec<_>>().join(", ");
    let vals = cols
        .iter()
        .enumerate()
        .map(|(i, (_, is_int))| {
            if *is_int {
                cell_i32_at(row, i).to_string()
            } else {
                lit(&cell_str(row, i))
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("INSERT INTO {table} ({names}) VALUES ({vals});\n")
}

/// Връща SQL и броя таблици, които са прочетени.
pub async fn logical_sql(db: &Db) -> Result<(String, i32), ApiError> {
    let mut out = String::from(
        "-- ytbuch logical dump\n-- Поредица от INSERT. Octo не изпълнява запис вътре в транзакция.\n",
    );
    let mut tables = 0;
    for (name, spec) in TABLES {
        let cols = columns(spec);
        let list = cols.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ");
        let rows = db
            .query(&format!("SELECT {list} FROM {name} ORDER BY id"))
            .await
            .map_err(db_err)?;
        tables += 1;
        for row in &rows {
            out.push_str(&insert_sql(name, &cols, row));
        }
    }
    Ok((out, tables))
}
