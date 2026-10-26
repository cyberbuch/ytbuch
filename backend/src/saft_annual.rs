//! Годишен SAF-T: активи, оценки и придобивания за годината.

use crate::dec::money;
use crate::saft_load::{Asset, Snap};
use crate::saft_xml::{self, clip, or_text, same_co, text, Addr};

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
