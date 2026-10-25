//! Годишен SAF-T, файл при поискване, данъчна таблица, плащания и продукти.
//! Месец без осчетоводено банково движение пак пише Payments с нула записа:
//! схемата иска поне един Payment и тогава файлът не минава XSD.
//! При поискване няма дневник на стокови движения: бройките са 0.

use std::collections::HashMap;

use crate::dec::{self, money};
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
    if body.is_empty() {
        return String::new();
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
    xml.push_str(&text("ProductCommodityCode", &saft_xml::cn8_code(&p.cn8_code, p.service)));
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
    if !tx.is_empty() {
        xml.push_str(&saft_xml::open("AssetTransactions"));
        xml.push_str(&text("NumberOfAssetTransactions", &n.to_string()));
        xml.push_str(&tx);
        xml.push_str(&saft_xml::close("AssetTransactions"));
    }
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

struct MoveLine {
    no: i32,
    account: String,
    txn: i32,
    customer: String,
    supplier: String,
    product: String,
    qty: String,
    uom: String,
    value: i128,
    subtype: String,
}

struct Move {
    reference: String,
    date: String,
    mtype: &'static str,
    doc_type: String,
    doc_number: String,
    lines: Vec<MoveLine>,
}

fn move_xml(s: &Snap, m: &Move) -> String {
    let soft = {
        let n = clip(&s.company.soft_name, 70);
        if n.is_empty() { "ytbuch".into() } else { n }
    };
    let mut xml = saft_xml::open("StockMovement");
    xml.push_str(&text("MovementReference", &clip(&m.reference, 35)));
    xml.push_str(&text("MovementDate", &m.date));
    xml.push_str(&text("MovementType", m.mtype));
    xml.push_str(&text("SourceID", &soft));
    xml.push_str(&text("SystemID", &clip(&m.reference, 35)));
    xml.push_str(&saft_xml::open("DocumentReference"));
    xml.push_str(&text("DocumentType", &clip(&m.doc_type, 35)));
    xml.push_str(&text("DocumentNumber", &clip(&m.doc_number, 35)));
    xml.push_str(&saft_xml::close("DocumentReference"));
    for l in &m.lines {
        xml.push_str(&saft_xml::open("StockMovementLine"));
        xml.push_str(&text("LineNumber", &l.no.to_string()));
        xml.push_str(&text("AccountID", &l.account));
        xml.push_str(&text("TransactionID", &l.txn.to_string()));
        xml.push_str(&text("CustomerID", &l.customer));
        xml.push_str(&text("SupplierID", &l.supplier));
        xml.push_str(&text("ProductCode", &clip(&l.product, 70)));
        xml.push_str(&text("Quantity", &l.qty));
        xml.push_str(&text("UnitOfMeasure", &l.uom));
        xml.push_str(&text("UOMToUOMPhysicalStockConversionFactor", "1"));
        xml.push_str(&text("BookValue", &money(l.value)));
        xml.push_str(&text("MovementSubType", &clip(&l.subtype, 35)));
        xml.push_str(&saft_xml::close("StockMovementLine"));
    }
    xml.push_str(&saft_xml::close("StockMovement"));
    xml
}

fn collect_moves(s: &Snap) -> Vec<Move> {
    let mut moves = Vec::new();
    let prod: HashMap<i32, &Product> = s
        .products
        .iter()
        .filter(|p| same_co(p.company_id, s.company_id))
        .map(|p| (p.id, p))
        .collect();
    let acc_of = |id: i32| -> String {
        if id <= 0 {
            return String::new();
        }
        s.nra.get(&id).cloned().unwrap_or_default()
    };
    let eik_of = |cp: i32| -> String {
        if cp <= 0 {
            return "0".into();
        }
        s.parties
            .iter()
            .find(|p| p.id == cp && same_co(p.company_id, s.company_id))
            .map(|p| clip(&p.eik, 35))
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "0".into())
    };
    let day = |d: &str| -> String { d.chars().take(10).collect() };

    for inv in &s.invoices {
        if !same_co(inv.company_id, s.company_id) || inv.journal_entry_id <= 0 {
            continue;
        }
        if inv.document_type == "proforma" || inv.issue_date.len() < 10 {
            continue;
        }
        let Some(lines) = s.inv_lines.get(&inv.id) else {
            continue;
        };
        let out = inv.direction == "out";
        let mut mlines = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            let Some(p) = prod.get(&l.product_id) else {
                continue;
            };
            if !p.inventory {
                continue;
            }
            let acc = acc_of(p.inv_acc);
            if acc.is_empty() {
                continue;
            }
            let q = or_text(&l.quantity, "0");
            if dec::or0(&q) <= 0 {
                continue;
            }
            mlines.push(MoveLine {
                no: (i + 1) as i32,
                account: acc,
                txn: inv.journal_entry_id,
                customer: if out { eik_of(inv.counterpart_id) } else { "0".into() },
                supplier: if out { "0".into() } else { eik_of(inv.counterpart_id) },
                product: clip(&p.code, 70),
                qty: q,
                uom: unit_code(&p.unit),
                value: l.net,
                subtype: if out { "Продажба".into() } else { "Покупка".into() },
            });
        }
        if mlines.is_empty() {
            continue;
        }
        moves.push(Move {
            reference: clip(&inv.number, 35),
            date: day(&inv.issue_date),
            mtype: if out { "30" } else { "10" },
            doc_type: clip(&inv.document_type, 35),
            doc_number: clip(&inv.number, 35),
            lines: mlines,
        });
    }

    for o in &s.prod_orders {
        if !same_co(o.company_id, s.company_id) || o.status != "completed" || o.date.len() < 10 {
            continue;
        }
        if o.output_product_id > 0 {
            if let Some(p) = prod.get(&o.output_product_id) {
                let acc = acc_of(p.inv_acc);
                let q = or_text(&o.quantity, "0");
                if p.inventory && !acc.is_empty() && dec::or0(&q) > 0 {
                    moves.push(Move {
                        reference: clip(&o.number, 35),
                        date: day(&o.date),
                        mtype: "20",
                        doc_type: "ПР".into(),
                        doc_number: clip(&o.number, 35),
                        lines: vec![MoveLine {
                            no: 1,
                            account: acc,
                            txn: if o.journal_id > 0 { o.journal_id } else { o.id },
                            customer: "0".into(),
                            supplier: "0".into(),
                            product: clip(&p.code, 70),
                            qty: q,
                            uom: unit_code(&p.unit),
                            value: o.cost,
                            subtype: "Производство".into(),
                        }],
                    });
                }
            }
        }
        let Some(lines) = s.prod_lines.get(&o.id) else {
            continue;
        };
        let mut mlines = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            let Some(p) = prod.get(&l.product_id) else {
                continue;
            };
            if !p.inventory {
                continue;
            }
            let acc = acc_of(p.inv_acc);
            if acc.is_empty() {
                continue;
            }
            let q = or_text(&l.quantity, "0");
            if dec::or0(&q) <= 0 {
                continue;
            }
            mlines.push(MoveLine {
                no: (i + 1) as i32,
                account: acc,
                txn: if o.journal_id > 0 { o.journal_id } else { o.id },
                customer: "0".into(),
                supplier: "0".into(),
                product: clip(&p.code, 70),
                qty: q,
                uom: unit_code(&l.unit),
                value: l.amount,
                subtype: "Материал за производство".into(),
            });
        }
        if !mlines.is_empty() {
            moves.push(Move {
                reference: clip(&o.number, 35),
                date: day(&o.date),
                mtype: "70",
                doc_type: "ПР".into(),
                doc_number: clip(&o.number, 35),
                lines: mlines,
            });
        }
    }

    for d in &s.mat_issues {
        if !same_co(d.company_id, s.company_id) || d.status != "confirmed" || d.date.len() < 10 {
            continue;
        }
        let Some(lines) = s.mat_lines.get(&d.id) else {
            continue;
        };
        let mut mlines = Vec::new();
        for l in lines {
            let Some(p) = prod.get(&l.product_id) else {
                continue;
            };
            if !p.inventory {
                continue;
            }
            let acc = {
                let a = acc_of(l.inventory_account_id);
                if a.is_empty() { acc_of(p.inv_acc) } else { a }
            };
            if acc.is_empty() {
                continue;
            }
            let q = or_text(&l.quantity, "0");
            if dec::or0(&q) <= 0 {
                continue;
            }
            mlines.push(MoveLine {
                no: if l.line_no > 0 { l.line_no } else { (mlines.len() + 1) as i32 },
                account: acc,
                txn: if d.journal_id > 0 { d.journal_id } else { d.id },
                customer: "0".into(),
                supplier: "0".into(),
                product: clip(&p.code, 70),
                qty: q,
                uom: unit_code(&p.unit),
                value: l.amount,
                subtype: "Изписване за производство".into(),
            });
        }
        if !mlines.is_empty() {
            moves.push(Move {
                reference: clip(&d.number, 35),
                date: day(&d.date),
                mtype: "70",
                doc_type: "МИ".into(),
                doc_number: clip(&d.number, 35),
                lines: mlines,
            });
        }
    }

    for d in &s.idocs {
        if !same_co(d.company_id, s.company_id) || d.status != "confirmed" || d.date.len() < 10 {
            continue;
        }
        let Some(lines) = s.idoc_lines.get(&d.id) else {
            continue;
        };
        let mut mlines = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            let Some(p) = prod.get(&l.product_id) else {
                continue;
            };
            if !p.inventory {
                continue;
            }
            let acc = acc_of(p.inv_acc);
            if acc.is_empty() {
                continue;
            }
            let q = or_text(&l.quantity, "0");
            if dec::or0(&q) <= 0 {
                continue;
            }
            mlines.push(MoveLine {
                no: (i + 1) as i32,
                account: acc,
                txn: d.id,
                customer: "0".into(),
                supplier: "0".into(),
                product: clip(&p.code, 70),
                qty: q,
                uom: unit_code(&p.unit),
                value: l.amount,
                subtype: "Вътрешен трансфер".into(),
            });
        }
        if !mlines.is_empty() {
            moves.push(Move {
                reference: clip(&d.number, 35),
                date: day(&d.date),
                mtype: "80",
                doc_type: "ВП".into(),
                doc_number: clip(&d.number, 35),
                lines: mlines,
            });
        }
    }

    moves
}

pub fn movements_xml(s: &Snap) -> String {
    let moves = collect_moves(s);
    let mut body = String::new();
    let mut lines = 0i64;
    let mut received = 0i128;
    let mut issued = 0i128;
    for m in &moves {
        for l in &m.lines {
            lines += 1;
            let q = dec::or0(&l.qty);
            match m.mtype {
                "10" | "20" => received += q,
                _ => issued += q,
            }
        }
        body.push_str(&move_xml(s, m));
    }
    let mut xml = saft_xml::open("MovementOfGoods");
    xml.push_str(&text("NumberOfMovementLines", &lines.to_string()));
    xml.push_str(&text("TotalQuantityReceived", &dec::text(received, 4)));
    xml.push_str(&text("TotalQuantityIssued", &dec::text(issued, 4)));
    xml.push_str(&body);
    xml.push_str(&saft_xml::close("MovementOfGoods"));
    xml
}

pub fn ondemand_xml(s: &Snap) -> String {
    let owner = {
        let e = clip(&s.company.eik, 35);
        if e.is_empty() { "0".into() } else { e }
    };
    let products = firm_products(s);
    let mut open_qty: HashMap<i32, String> = HashMap::new();
    let mut open_val: HashMap<i32, i128> = HashMap::new();
    for o in &s.openings {
        if o.product_id > 0 {
            open_qty.insert(o.product_id, or_text(&o.qty, "0"));
            open_val.insert(o.product_id, o.debit);
        }
    }
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
        let oq = open_qty.get(&p.id).cloned().unwrap_or_else(|| "0".into());
        let ov = open_val.get(&p.id).copied().unwrap_or(0);
        stock.push_str(&saft_xml::open("PhysicalStockEntry"));
        stock.push_str(&text("WarehouseID", "1"));
        stock.push_str(&text("ProductCode", &code));
        stock.push_str(&text("ProductType", &saft_xml::product_type(&p.product_type)));
        stock.push_str(&text("StockAccountCommodityCode", &saft_xml::cn8_code(&p.cn8_code, false)));
        stock.push_str(&text("OwnerID", &owner));
        stock.push_str(&text("UOMPhysicalStock", &u));
        stock.push_str(&text("UOMToUOMBaseConversionFactor", "1"));
        stock.push_str(&text("UnitPriceBegin", &money(p.cost)));
        stock.push_str(&text("UnitPriceEnd", &money(p.cost)));
        stock.push_str(&text("OpeningStockQuantity", &oq));
        stock.push_str(&text("OpeningStockValue", &money(ov)));
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
    xml.push_str(&movements_xml(s));
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
