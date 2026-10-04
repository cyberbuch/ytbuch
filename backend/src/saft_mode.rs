//! Годишен SAF-T, файл при поискване, данъчна таблица, плащания и продукти.
//! Месец без осчетоводено банково движение пак пише Payments с нула записа:
//! схемата иска поне един Payment и тогава файлът не минава XSD.
//! При поискване няма дневник на стокови движения: бройките са 0.

use crate::dec::money;
use crate::saft_gl;
use crate::saft_load::{Asset, Product, Snap};
use crate::saft_xml::{self, amount, clip, or_text, same_co, stock_moves, text, unit_name, Addr};

pub fn tax_table() -> String {
    let mut xml = saft_xml::open("TaxTable");
    xml.push_str(&tax_entry(
        "100",
        "ДАНЪК ВЪРХУ ДОБАВЕНАТА СТОЙНОСТ",
        &[
            ("100211", "20.00", "Данъчна основа на облагаемите доставки със ставка 20 %"),
            ("100213", "9.00", "ДО на облагаемите доставки със ставка 9 %"),
            ("100214", "0.00", "ДО на доставките със ставка 0 % по глава трета от ЗДДС"),
            ("100219", "0.00", "ДО на освободени доставки и освободените ВОП"),
        ],
    ));
    xml.push_str(&tax_entry(
        "000",
        "Неприложимо",
        &[("000000", "0.00", "Неприложим код")],
    ));
    xml.push_str(&saft_xml::close("TaxTable"));
    xml
}

fn tax_entry(kind: &str, desc: &str, codes: &[(&str, &str, &str)]) -> String {
    let mut xml = saft_xml::open("TaxTableEntry");
    xml.push_str(&text("TaxType", kind));
    xml.push_str(&text("Description", desc));
    for (code, rate, name) in codes {
        xml.push_str(&saft_xml::open("TaxCodeDetails"));
        xml.push_str(&text("TaxCode", code));
        xml.push_str(&text("Description", name));
        xml.push_str(&text("TaxPercentage", rate));
        xml.push_str(&text("Country", "BG"));
        xml.push_str(&saft_xml::close("TaxCodeDetails"));
    }
    xml.push_str(&saft_xml::close("TaxTableEntry"));
    xml
}

pub fn payments_xml(s: &Snap, year: i32, month: i32) -> String {
    let prefix = format!("{year:04}-{month:02}");
    let cur = saft_gl::currency(s);
    let mut body = String::new();
    let mut n = 0i64;
    let mut debit = 0i128;
    let mut credit = 0i128;
    for tx in &s.banks {
        if !tx.booked || !same_co(tx.company_id, s.company_id) || !tx.date.starts_with(&prefix) {
            continue;
        }
        if tx.gl.is_empty() || tx.amount == 0 || tx.date.len() < 10 {
            continue;
        }
        let neg = tx.amount < 0;
        let mag = tx.amount.abs();
        let mut ind = if tx.kind == "WITHDRAWAL" { "C" } else { "D" };
        if neg {
            ind = if ind == "D" { "C" } else { "D" };
        }
        if ind == "D" {
            debit += mag;
        } else {
            credit += mag;
        }
        let party = party_eik(s, &tx.name);
        let (customer, supplier) = if ind == "D" { (party, "0".into()) } else { ("0".into(), party) };
        let refno = {
            let r = clip(&tx.reference, 35);
            if r.is_empty() { format!("PAY-{}", tx.id) } else { r }
        };
        let txn = if tx.journal_id > 0 { tx.journal_id } else { tx.id };
        let desc = {
            let d = clip(&tx.description, 256);
            if d.is_empty() { "Плащане по банка".into() } else { d }
        };
        let icur = {
            let c = tx.currency.trim();
            if c.is_empty() { cur.clone() } else { c.to_string() }
        };
        let day: String = tx.date.chars().take(10).collect();
        body.push_str(&payment(
            &refno,
            month,
            year,
            txn,
            &day,
            &desc,
            &tx.gl,
            &customer,
            &supplier,
            ind,
            mag,
            &icur,
            &clip(&tx.reference, 35),
        ));
        n += 1;
    }
    let mut xml = saft_xml::open("Payments");
    xml.push_str(&text("NumberOfEntries", &n.to_string()));
    xml.push_str(&text("TotalDebit", &money(debit)));
    xml.push_str(&text("TotalCredit", &money(credit)));
    xml.push_str(&body);
    xml.push_str(&saft_xml::close("Payments"));
    xml
}

fn payment(
    refno: &str,
    month: i32,
    year: i32,
    txn: i32,
    date: &str,
    desc: &str,
    account: &str,
    customer: &str,
    supplier: &str,
    indicator: &str,
    amount_v: i128,
    currency: &str,
    source: &str,
) -> String {
    let mut xml = saft_xml::open("Payment");
    xml.push_str(&text("PaymentRefNo", refno));
    xml.push_str(&text("Period", &month.to_string()));
    xml.push_str(&text("PeriodYear", &year.to_string()));
    xml.push_str(&text("TransactionID", &txn.to_string()));
    xml.push_str(&text("TransactionDate", date));
    xml.push_str(&text("PaymentMethod", "03"));
    xml.push_str(&text("Description", desc));
    xml.push_str(&text("SystemID", refno));
    xml.push_str(&text("SourceID", "ytbuch"));
    xml.push_str(&saft_xml::open("PaymentLine"));
    xml.push_str(&text("LineNumber", "1"));
    xml.push_str(&saft_xml::opt("SourceDocumentID", source));
    xml.push_str(&text("AccountID", account));
    xml.push_str(&text("CustomerID", customer));
    xml.push_str(&text("SupplierID", supplier));
    xml.push_str(&text("TaxPointDate", date));
    xml.push_str(&text("Description", desc));
    xml.push_str(&text("DebitCreditIndicator", indicator));
    xml.push_str(&amount("PaymentLineAmount", amount_v, currency));
    xml.push_str(&saft_xml::close("PaymentLine"));
    xml.push_str(&saft_xml::open("PaymentSettlement"));
    xml.push_str(&amount("SettlementAmount", amount_v, currency));
    xml.push_str(&text("SettlementDate", date));
    xml.push_str(&text("PaymentMechanism", "42"));
    xml.push_str(&saft_xml::close("PaymentSettlement"));
    xml.push_str(&saft_xml::open("PaymentDocumentTotals"));
    xml.push_str(&text("NetTotal", &money(amount_v)));
    xml.push_str(&text("GrossTotal", &money(amount_v)));
    xml.push_str(&saft_xml::close("PaymentDocumentTotals"));
    xml.push_str(&saft_xml::close("Payment"));
    xml
}

fn party_eik(s: &Snap, name: &str) -> String {
    let want = name.trim();
    if want.is_empty() {
        return "0".into();
    }
    for p in &s.parties {
        if p.name.trim() == want {
            let eik = clip(&p.eik, 35);
            if !eik.is_empty() {
                return eik;
            }
        }
    }
    "0".into()
}

pub fn uom_products(s: &Snap) -> String {
    let products = firm_products(s);
    let mut units = Vec::new();
    for p in &products {
        let u = unit_code(&p.unit);
        if !units.iter().any(|x: &String| x == &u) {
            units.push(u);
        }
    }
    if units.is_empty() {
        units.push("C62".into());
    }
    let mut xml = saft_xml::open("UOMTable");
    for u in &units {
        xml.push_str(&saft_xml::open("UOMTableEntry"));
        xml.push_str(&text("UnitOfMeasure", u));
        xml.push_str(&text("Description", &unit_name(u)));
        xml.push_str(&saft_xml::close("UOMTableEntry"));
    }
    xml.push_str(&saft_xml::close("UOMTable"));
    xml.push_str(&saft_xml::open("Products"));
    for p in &products {
        xml.push_str(&product(p));
    }
    xml.push_str(&saft_xml::close("Products"));
    xml
}

fn product(p: &Product) -> String {
    let code = {
        let c = clip(&p.code, 70);
        if c.is_empty() { "0".into() } else { c }
    };
    let name = {
        let n = clip(&p.name, 256);
        if n.is_empty() { code.clone() } else { n }
    };
    let u = unit_code(&p.unit);
    let mut xml = saft_xml::open("Product");
    xml.push_str(&text("ProductCode", &code));
    xml.push_str(&text("GoodsServicesID", if p.service { "02" } else { "01" }));
    xml.push_str(&text("Description", &name));
    xml.push_str(&text("ProductCommodityCode", "0"));
    xml.push_str(&text("UOMBase", &u));
    xml.push_str(&text("UOMStandard", &u));
    xml.push_str(&text("UOMToUOMBaseConversionFactor", "1"));
    xml.push_str(&saft_xml::close("Product"));
    xml
}

pub fn annual_xml(s: &Snap, year: i32) -> String {
    let y = year.to_string();
    let mut assets = String::new();
    let mut tx = String::new();
    let mut n = 0i64;
    for a in &s.assets {
        if !same_co(a.company_id, s.company_id) || a.acquired.len() < 10 {
            continue;
        }
        let day: String = a.acquired.chars().take(10).collect();
        let start = if a.startup.len() >= 10 {
            a.startup.chars().take(10).collect()
        } else {
            day.clone()
        };
        let asset_id = {
            let id = clip(&a.inventory, 35);
            if id.is_empty() { format!("FA-{}", a.id) } else { id }
        };
        assets.push_str(&asset_block(a, &asset_id, &day, &start));
        if day.starts_with(&y) {
            n += 1;
            tx.push_str(&asset_tx(&asset_id, &day, a.cost));
        }
    }
    let mut xml = saft_xml::open("MasterFilesAnnual");
    xml.push_str(&saft_xml::open("Assets"));
    xml.push_str(&assets);
    xml.push_str(&saft_xml::close("Assets"));
    xml.push_str(&saft_xml::close("MasterFilesAnnual"));
    xml.push_str(&saft_xml::open("SourceDocumentsAnnual"));
    xml.push_str(&saft_xml::open("AssetTransactions"));
    xml.push_str(&text("NumberOfAssetTransactions", &n.to_string()));
    xml.push_str(&tx);
    xml.push_str(&saft_xml::close("AssetTransactions"));
    xml.push_str(&saft_xml::close("SourceDocumentsAnnual"));
    xml
}

fn asset_block(a: &Asset, asset_id: &str, day: &str, start: &str) -> String {
    let desc = {
        let d = clip(&a.name, 256);
        if d.is_empty() { asset_id.to_string() } else { d }
    };
    let mut xml = saft_xml::open("Asset");
    xml.push_str(&text("AssetID", asset_id));
    xml.push_str(&text("AccountID", "204"));
    xml.push_str(&text("Description", &desc));
    xml.push_str(&saft_xml::open("AssetSupplier"));
    xml.push_str(&text("SupplierName", "Неизвестен"));
    xml.push_str(&text("SupplierID", "0"));
    xml.push_str(&saft_xml::address(
        "PostalAddress",
        &Addr {
            street: "",
            number: "",
            extra: "",
            building: "",
            city: "",
            post: "",
            region: "",
            country: "BG",
            kind: "",
        },
    ));
    xml.push_str(&saft_xml::close("AssetSupplier"));
    xml.push_str(&text("DateOfAcquisition", day));
    xml.push_str(&text("StartUpDate", start));
    xml.push_str(&saft_xml::open("Valuations"));
    xml.push_str(&valuation(a));
    xml.push_str(&dap(a));
    xml.push_str(&saft_xml::close("Valuations"));
    xml.push_str(&saft_xml::close("Asset"));
    xml
}

fn valuation(a: &Asset) -> String {
    let book = a.cost - a.acc;
    let mut xml = saft_xml::open("ValuationSAP");
    xml.push_str(&text("ValuationClass", "Счетоводен"));
    xml.push_str(&text("AcquisitionAndProductionCostsBegin", &money(a.cost)));
    xml.push_str(&text("AcquisitionAndProductionCostsEnd", &money(a.cost)));
    xml.push_str(&text("InvestmentSupport", "0.00"));
    xml.push_str(&text("AssetLifeMonth", &or_text(&a.life, "0")));
    xml.push_str(&text("AssetAddition", "0.00"));
    xml.push_str(&text("Transfers", "0.00"));
    xml.push_str(&text("AssetDisposal", "0.00"));
    xml.push_str(&text("BookValueBegin", &money(book)));
    xml.push_str(&text("DepreciationMethod", &clip(&or_text(&a.method, "linear"), 35)));
    xml.push_str(&text("DepreciationPercentage", &or_text(&a.rate, "0")));
    xml.push_str(&text("DepreciationForPeriod", "0.00"));
    xml.push_str(&text("AppreciationForPeriod", "0.00"));
    xml.push_str(&saft_xml::open("ExtraordinaryDepreciationsForPeriod"));
    xml.push_str(&saft_xml::open("ExtraordinaryDepreciationForPeriod"));
    xml.push_str(&text("ExtraordinaryDepreciationMethod", "0"));
    xml.push_str(&text("ExtraordinaryDepreciationAmountForPeriod", "0.00"));
    xml.push_str(&saft_xml::close("ExtraordinaryDepreciationForPeriod"));
    xml.push_str(&saft_xml::close("ExtraordinaryDepreciationsForPeriod"));
    xml.push_str(&text("AccumulatedDepreciation", &money(a.acc)));
    xml.push_str(&text("BookValueEnd", &money(book)));
    xml.push_str(&saft_xml::close("ValuationSAP"));
    xml
}

fn dap(a: &Asset) -> String {
    let tax_value = a.cost - a.tax_acc;
    let mut xml = saft_xml::open("ValuationDAP");
    xml.push_str(&text("ValuationClass", "Данъчен"));
    xml.push_str(&text("CategoryTaxDepreciable", &clip(&or_text(&a.category_id.to_string(), "0"), 35)));
    xml.push_str(&text("TaxDepreciableValue", &money(a.cost)));
    xml.push_str(&text("AccruedTaxDepreciation", &money(a.tax_acc)));
    xml.push_str(&text("TaxValueAsset", &money(tax_value)));
    xml.push_str(&text("AnnualTaxDepreciationRate", &or_text(&a.tax_rate, "0")));
    xml.push_str(&text("MonthChangeAssetValue", "0"));
    xml.push_str(&text("MonthSuspensionResumptionAccrual", "0"));
    xml.push_str(&text("MonthWriteOffAccounting", "0"));
    xml.push_str(&text("MonthWriteOffTax", "0"));
    xml.push_str(&text("NumberMonthsDepreciationDuring", "0"));
    xml.push_str(&text("DepreciationForPeriod", "0.00"));
    xml.push_str(&text("AccumulatedDepreciation", &money(a.tax_acc)));
    xml.push_str(&text("TaxValueEndPeriod", &money(tax_value)));
    xml.push_str(&saft_xml::close("ValuationDAP"));
    xml
}

fn asset_tx(asset_id: &str, day: &str, cost: i128) -> String {
    let id = clip(&format!("ACQ-{asset_id}"), 70);
    let mut xml = saft_xml::open("AssetTransaction");
    xml.push_str(&text("AssetTransactionID", &id));
    xml.push_str(&text("AssetID", asset_id));
    xml.push_str(&text("AssetTransactionType", "10"));
    xml.push_str(&text("Description", "Придобиване"));
    xml.push_str(&text("AssetTransactionDate", day));
    xml.push_str(&text("TransactionID", &id));
    xml.push_str(&saft_xml::open("AssetTransactionValuations"));
    xml.push_str(&saft_xml::open("AssetTransactionValuation"));
    xml.push_str(&text("AcquisitionAndProductionCostsOnTransaction", &money(cost)));
    xml.push_str(&text("BookValueOnTransaction", &money(cost)));
    xml.push_str(&text("AssetTransactionAmount", &money(cost)));
    xml.push_str(&saft_xml::close("AssetTransactionValuation"));
    xml.push_str(&saft_xml::close("AssetTransactionValuations"));
    xml.push_str(&saft_xml::close("AssetTransaction"));
    xml
}

pub fn ondemand_xml(s: &Snap) -> String {
    let owner = {
        let e = clip(&s.company.eik, 35);
        if e.is_empty() { "0".into() } else { e }
    };
    let products = firm_products(s);
    let mut stock = String::new();
    for p in &products {
        if !p.inventory {
            continue;
        }
        let code = {
            let c = clip(&p.code, 70);
            if c.is_empty() { "0".into() } else { c }
        };
        let u = unit_code(&p.unit);
        let qty = or_text(&p.qty, "0");
        stock.push_str(&saft_xml::open("PhysicalStockEntry"));
        stock.push_str(&text("WarehouseID", "1"));
        stock.push_str(&text("ProductCode", &code));
        stock.push_str(&text("ProductType", "10"));
        stock.push_str(&text("StockAccountCommodityCode", "0"));
        stock.push_str(&text("OwnerID", &owner));
        stock.push_str(&text("UOMPhysicalStock", &u));
        stock.push_str(&text("UOMToUOMBaseConversionFactor", "1"));
        stock.push_str(&text("UnitPriceBegin", &money(p.cost)));
        stock.push_str(&text("UnitPriceEnd", &money(p.cost)));
        stock.push_str(&text("OpeningStockQuantity", &qty));
        stock.push_str(&text("OpeningStockValue", &money(p.value)));
        stock.push_str(&text("ClosingStockQuantity", &qty));
        stock.push_str(&text("ClosingStockValue", &money(p.value)));
        stock.push_str(&saft_xml::close("PhysicalStockEntry"));
    }
    let mut xml = saft_xml::open("MasterFilesOnDemand");
    xml.push_str(&moves());
    xml.push_str(&uom_products(s));
    xml.push_str(&saft_xml::open("PhysicalStock"));
    xml.push_str(&stock);
    xml.push_str(&saft_xml::close("PhysicalStock"));
    xml.push_str(&saft_xml::close("MasterFilesOnDemand"));
    xml.push_str(&saft_xml::open("SourceDocumentsOnDemand"));
    xml.push_str(&saft_xml::open("MovementOfGoods"));
    xml.push_str(&text("NumberOfMovementLines", "0"));
    xml.push_str(&text("TotalQuantityReceived", "0"));
    xml.push_str(&text("TotalQuantityIssued", "0"));
    xml.push_str(&saft_xml::close("MovementOfGoods"));
    xml.push_str(&saft_xml::close("SourceDocumentsOnDemand"));
    xml
}

fn moves() -> String {
    let mut xml = saft_xml::open("MovementTypeTable");
    for (code, name) in stock_moves() {
        xml.push_str(&saft_xml::open("MovementTypeTableEntry"));
        xml.push_str(&text("MovementType", &code));
        xml.push_str(&text("Description", &name));
        xml.push_str(&saft_xml::close("MovementTypeTableEntry"));
    }
    xml.push_str(&saft_xml::close("MovementTypeTable"));
    xml
}

fn firm_products(s: &Snap) -> Vec<&Product> {
    s.products.iter().filter(|p| same_co(p.company_id, s.company_id)).collect()
}

fn unit_code(unit: &str) -> String {
    let u = clip(unit, 9);
    if u.is_empty() { "C62".into() } else { u }
}
