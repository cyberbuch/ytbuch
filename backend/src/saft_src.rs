//! Клиенти, доставчици и фактури за месечния SAF-T.
//! Осчетоводен ред без product_id: продажба взима дебит 411 и кредит извън 453,
//! покупка взима кредит 401 и дебит извън 453. Без дневник остават 411/702 и 401/601.

use crate::dec::money;
use crate::saft_gl::{self, get};
use crate::saft_load::{InvLine, Invoice, Party, Product, Snap};
use crate::saft_xml::{self, amount, bal, clip, same_co, shown_name, text, window, Addr};

pub fn parties_xml(s: &Snap, year: i32, month: i32) -> String {
    let w = window(year, month, s.company.fys);
    let map = saft_gl::balances(s, &w);
    let mut customers = String::new();
    let mut suppliers = String::new();
    for p in &s.parties {
        if !same_co(p.company_id, s.company_id) {
            continue;
        }
        let (client, supplier) = saft_gl::roles(&p.kind);
        let id = eik_or0(&p.eik);
        if client {
            customers.push_str(&party("Customer", "CustomerID", p, &id, "411", &side(&map, p.id, "c")));
        }
        if supplier {
            suppliers.push_str(&party("Supplier", "SupplierID", p, &id, "401", &side(&map, p.id, "s")));
        }
    }
    let mut xml = saft_xml::open("Customers");
    xml.push_str(&customers);
    xml.push_str(&saft_xml::close("Customers"));
    xml.push_str(&saft_xml::open("Suppliers"));
    xml.push_str(&suppliers);
    xml.push_str(&saft_xml::close("Suppliers"));
    xml
}

fn party(tag: &str, id_tag: &str, p: &Party, id: &str, account: &str, sides: &str) -> String {
    let mut xml = saft_xml::open(tag);
    xml.push_str(&company(p));
    xml.push_str(&text(id_tag, id));
    xml.push_str(&text("AccountID", account));
    xml.push_str(sides);
    xml.push_str(&saft_xml::close(tag));
    xml
}

fn company(p: &Party) -> String {
    let name = clip(&p.name, 70);
    let mut xml = saft_xml::open("CompanyStructure");
    xml.push_str(&text("RegistrationNumber", &eik_or0(&p.eik)));
    if saft_xml::cyr(&name) || name.is_empty() {
        xml.push_str(&text("Name", &shown_name(&name, 70)));
    } else {
        xml.push_str(&text("NameLatin", &name));
    }
    xml.push_str(&saft_xml::address(
        "Address",
        &Addr {
            street: &p.street,
            number: &p.number,
            extra: &p.extra,
            building: &p.building,
            city: &p.city,
            post: &p.post,
            region: &p.region,
            country: &p.country,
            kind: &p.addr_type,
        },
    ));
    xml.push_str(&text("RelatedParty", "N"));
    xml.push_str(&saft_xml::close("CompanyStructure"));
    xml
}

fn side(map: &std::collections::HashMap<String, i128>, id: i32, role: &str) -> String {
    let od = get(map, &format!("p:{id}:{role}:od"));
    let oc = get(map, &format!("p:{id}:{role}:oc"));
    let pd = get(map, &format!("p:{id}:{role}:pd"));
    let pc = get(map, &format!("p:{id}:{role}:pc"));
    let mut xml = bal("OpeningDebitBalance", "OpeningCreditBalance", od, oc);
    xml.push_str(&bal(
        "ClosingDebitBalance",
        "ClosingCreditBalance",
        od + pd,
        oc + pc,
    ));
    xml
}

pub fn invoices_xml(s: &Snap, year: i32, month: i32) -> (String, String) {
    let prefix = format!("{year:04}-{month:02}");
    let cur = saft_gl::currency(s);
    let mut sales = String::new();
    let mut purchases = String::new();
    let mut ns = 0i64;
    let mut np = 0i64;
    let mut credit = 0i128;
    let mut debit = 0i128;
    for inv in &s.invoices {
        if inv.document_type == "proforma" || !same_co(inv.company_id, s.company_id) {
            continue;
        }
        if inv.direction != "out" && inv.direction != "in" {
            continue;
        }
        let key = month_key(inv);
        if key != prefix || inv.issue_date.len() < 10 {
            continue;
        }
        let xml = one(s, inv, year, month, &cur);
        if inv.direction == "out" {
            sales.push_str(&xml);
            credit += inv.total;
            ns += 1;
        } else {
            purchases.push_str(&xml);
            debit += inv.total;
            np += 1;
        }
    }
    let sales_xml = wrap("SalesInvoices", ns, 0, credit, &sales);
    let buy_xml = wrap("PurchaseInvoices", np, debit, 0, &purchases);
    (sales_xml, buy_xml)
}

fn wrap(tag: &str, n: i64, debit: i128, credit: i128, body: &str) -> String {
    let mut xml = saft_xml::open(tag);
    xml.push_str(&text("NumberOfEntries", &n.to_string()));
    xml.push_str(&text("TotalDebit", &money(debit)));
    xml.push_str(&text("TotalCredit", &money(credit)));
    xml.push_str(body);
    xml.push_str(&saft_xml::close(tag));
    xml
}

fn one(s: &Snap, inv: &Invoice, year: i32, month: i32, cur: &str) -> String {
    let day: String = inv.issue_date.chars().take(10).collect();
    let tax_day = if inv.tax_date.len() >= 10 {
        inv.tax_date.chars().take(10).collect()
    } else {
        day.clone()
    };
    let icur = {
        let c = inv.currency.trim();
        if c.is_empty() { cur.to_string() } else { c.to_string() }
    };
    let purchase = inv.direction == "in";
    let (mut header, mut line_acc) = if purchase { ("401".into(), "601".into()) } else { ("411".into(), "702".into()) };
    if inv.journal_entry_id > 0 {
        if let Some(lines) = s.lines.get(&inv.journal_entry_id) {
            let (h, l) = journal_accounts(s, lines, purchase);
            if !h.is_empty() {
                header = h;
            }
            if !l.is_empty() {
                line_acc = l;
            }
        }
    }
    let party = s.parties.iter().find(|p| p.id == inv.counterpart_id);
    let eik = party.map(|p| p.eik.clone()).unwrap_or_default();
    let name = party.map(|p| p.name.clone()).unwrap_or_default();
    let city = party.map(|p| p.city.clone()).unwrap_or_default();
    let country = party.map(|p| p.country.clone()).unwrap_or_else(|| "BG".into());
    let lines = s.inv_lines.get(&inv.id).map(|v| v.as_slice()).unwrap_or(&[]);
    let mut body = String::new();
    if lines.is_empty() {
        body.push_str(&line(
            "1",
            &line_acc,
            "01",
            "0",
            "Фактура",
            "1",
            "C62",
            inv.net,
            inv.net,
            inv.vat,
            20 * 1_000_000,
            &inv.reason,
            &tax_day,
            if purchase { "D" } else { "C" },
            &icur,
        ));
    } else {
        for (i, row) in lines.iter().enumerate() {
            let prod = product_of(s, row);
            let code = prod
                .map(|p| clip(&p.code, 70))
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| "0".into());
            let goods = if prod.map(|p| p.service).unwrap_or(false) { "02" } else { "01" };
            let unit = {
                let u = clip(&row.unit, 9);
                if u.is_empty() { "C62".into() } else { u }
            };
            let qty = {
                let q = row.quantity.trim();
                if q.is_empty() { "1".into() } else { q.to_string() }
            };
            body.push_str(&line(
                &(i + 1).to_string(),
                &line_acc,
                goods,
                &code,
                &row.description,
                &qty,
                &unit,
                row.price,
                row.net,
                row.vat,
                row.rate,
                &inv.reason,
                &tax_day,
                if purchase { "D" } else { "C" },
                &icur,
            ));
        }
    }
    let dtype = if inv.document_type.len() == 2 && inv.document_type.bytes().all(|c| c.is_ascii_digit()) {
        inv.document_type.clone()
    } else {
        "01".into()
    };
    let txn = if inv.journal_entry_id > 0 { inv.journal_entry_id } else { inv.id };
    let mut xml = saft_xml::open("Invoice");
    xml.push_str(&text("InvoiceNo", &clip(&inv.number, 70)));
    xml.push_str(&info(purchase, &eik, &name, &city, &country));
    xml.push_str(&text("AccountID", &header));
    xml.push_str(&text("Period", &month.to_string()));
    xml.push_str(&text("PeriodYear", &year.to_string()));
    xml.push_str(&text("InvoiceDate", &day));
    xml.push_str(&text("InvoiceType", &dtype));
    xml.push_str(&text("SelfBillingIndicator", "N"));
    xml.push_str(&text("TransactionID", &txn.to_string()));
    xml.push_str(&body);
    xml.push_str(&saft_xml::open("InvoiceDocumentTotals"));
    xml.push_str(&text("NetTotal", &money(inv.net)));
    xml.push_str(&text("GrossTotal", &money(inv.total)));
    xml.push_str(&saft_xml::close("InvoiceDocumentTotals"));
    xml.push_str(&saft_xml::close("Invoice"));
    xml
}

fn journal_accounts(s: &Snap, lines: &[crate::saft_load::Line], purchase: bool) -> (String, String) {
    let want_h = if purchase { "credit" } else { "debit" };
    let want_l = if purchase { "debit" } else { "credit" };
    let prefix = if purchase { "401" } else { "411" };
    let mut header = String::new();
    let mut line = String::new();
    for row in lines {
        let Some(code) = s.nra.get(&row.account_id) else {
            continue;
        };
        if row.direction == want_h && saft_xml::pref(code, prefix) {
            header = code.clone();
        }
        if row.direction == want_l && !code.starts_with("453") && row.vat == 0 {
            line = code.clone();
        }
    }
    (header, line)
}

fn product_of<'a>(s: &'a Snap, line: &InvLine) -> Option<&'a Product> {
    if line.product_id > 0 {
        if let Some(p) = s.products.iter().find(|p| p.id == line.product_id && same_co(p.company_id, s.company_id)) {
            return Some(p);
        }
    }
    let code = line.code.trim();
    if code.is_empty() {
        return None;
    }
    s.products
        .iter()
        .find(|p| p.code.trim() == code && same_co(p.company_id, s.company_id))
}

fn info(purchase: bool, eik: &str, name: &str, city: &str, country: &str) -> String {
    let (tag, id_tag) = if purchase { ("SupplierInfo", "SupplierID") } else { ("CustomerInfo", "CustomerID") };
    let mut xml = saft_xml::open(tag);
    xml.push_str(&text(id_tag, &eik_or0(eik)));
    xml.push_str(&text("Name", &shown_name(name, 70)));
    xml.push_str(&saft_xml::open("BillingAddress"));
    xml.push_str(&text("City", &clip(city, 35)));
    xml.push_str(&text("Country", &saft_xml::country(country)));
    xml.push_str(&saft_xml::close("BillingAddress"));
    xml.push_str(&saft_xml::close(tag));
    xml
}

fn line(
    number: &str,
    account: &str,
    goods: &str,
    code: &str,
    desc: &str,
    qty: &str,
    unit: &str,
    price: i128,
    net: i128,
    vat: i128,
    rate: i128,
    reason: &str,
    day: &str,
    indicator: &str,
    currency: &str,
) -> String {
    let text_d = {
        let d = clip(desc, 256);
        if d.is_empty() { "Ред".into() } else { d }
    };
    let mut xml = saft_xml::open("InvoiceLine");
    xml.push_str(&text("LineNumber", number));
    xml.push_str(&text("AccountID", account));
    xml.push_str(&text("GoodsServicesID", goods));
    xml.push_str(&text("ProductCode", &clip(code, 70)));
    xml.push_str(&text("ProductDescription", &text_d));
    xml.push_str(&text("Quantity", qty));
    xml.push_str(&text("InvoiceUOM", unit));
    xml.push_str(&text("UnitPrice", &money(price)));
    xml.push_str(&text("TaxPointDate", day));
    xml.push_str(&text("Description", &text_d));
    xml.push_str(&amount("InvoiceLineAmount", net, currency));
    xml.push_str(&text("DebitCreditIndicator", indicator));
    xml.push_str(&saft_xml::open("TaxInformation"));
    xml.push_str(&text("TaxType", "100"));
    xml.push_str(&text("TaxCode", tax_code(rate, vat, reason)));
    xml.push_str(&text("TaxBase", &money(net)));
    xml.push_str(&amount("TaxAmount", vat, currency));
    xml.push_str(&saft_xml::close("TaxInformation"));
    xml.push_str(&saft_xml::close("InvoiceLine"));
    xml
}

fn tax_code(rate: i128, vat: i128, reason: &str) -> &'static str {
    const S: i128 = 1_000_000;
    if rate == 20 * S {
        "100211"
    } else if rate == 9 * S {
        "100213"
    } else if vat == 0 && !reason.trim().is_empty() {
        "100219"
    } else if rate == 0 {
        "100214"
    } else {
        "100211"
    }
}

fn month_key(inv: &Invoice) -> String {
    if inv.accounting_month.len() >= 7 {
        inv.accounting_month.chars().take(7).collect()
    } else {
        inv.issue_date.chars().take(7).collect()
    }
}

fn eik_or0(eik: &str) -> String {
    let t = clip(eik, 35);
    if t.is_empty() { "0".into() } else { t }
}
