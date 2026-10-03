//! Ред в дневника и клетките на декларацията. Сумите са стотинки.

use serde_json::{json, Value};

use crate::invoice_money::money;
use crate::vat_ex;
use crate::vat_fmt::{is_credit_doc, purch_excluded, sales_excluded};

#[derive(Clone)]
pub struct Line {
    pub kind: String,
    pub doc_type: String,
    pub doc_no: String,
    pub doc_date: String,
    pub cp_vat: String,
    pub cp_name: String,
    pub goods: String,
    pub delivery: String,
    pub p_no: i64,
    pub p_fb: i64,
    pub p_fv: i64,
    pub p_pb: i64,
    pub p_pv: i64,
    pub p_adj: i64,
    pub p_vop: i64,
    pub s_tb: i64,
    pub s_tv: i64,
    pub s_b20: i64,
    pub s_v20: i64,
    pub s_vopb: i64,
    pub s_82: i64,
    pub s_vopv: i64,
    pub s_pers: i64,
    pub s_b9: i64,
    pub s_v9: i64,
    pub s_ch3: i64,
    pub s_vod: i64,
    pub s_art: i64,
    pub s_svc: i64,
    pub s_69: i64,
    pub s_ex: i64,
    pub s_tri: i64,
}

impl Line {
    pub fn empty(kind: &str) -> Self {
        Self {
            kind: kind.to_string(),
            doc_type: "01".into(),
            doc_no: String::new(),
            doc_date: String::new(),
            cp_vat: String::new(),
            cp_name: String::new(),
            goods: String::new(),
            delivery: String::new(),
            p_no: 0, p_fb: 0, p_fv: 0, p_pb: 0, p_pv: 0, p_adj: 0, p_vop: 0,
            s_tb: 0, s_tv: 0, s_b20: 0, s_v20: 0, s_vopb: 0, s_82: 0,
            s_vopv: 0, s_pers: 0, s_b9: 0, s_v9: 0, s_ch3: 0, s_vod: 0,
            s_art: 0, s_svc: 0, s_69: 0, s_ex: 0, s_tri: 0,
        }
    }

    pub fn from_totals(kind: &str, base: i64, vat: i64) -> Self {
        let mut ln = Self::empty(kind);
        if kind == "sales" {
            let rate = if vat == 0 { "0" } else { "20" };
            ln.add_sale(base, vat, rate, "");
        } else {
            ln.add_purch(base, vat);
        }
        ln
    }

    pub fn reset_amounts(&mut self) {
        self.p_no = 0;
        self.p_fb = 0;
        self.p_fv = 0;
        self.s_tb = 0;
        self.s_tv = 0;
        self.s_b20 = 0;
        self.s_v20 = 0;
        self.s_b9 = 0;
        self.s_v9 = 0;
        self.s_ch3 = 0;
        self.s_vod = 0;
        self.s_art = 0;
        self.s_svc = 0;
        self.s_ex = 0;
    }

    pub fn add_sale(&mut self, base: i64, vat: i64, rate: &str, ex: &str) {
        if rate == "9" {
            self.s_tb += base;
            self.s_tv += vat;
            self.s_b9 += base;
            self.s_v9 += vat;
            return;
        }
        if rate == "0" {
            self.s_tb += base;
            match zero_bucket(ex) {
                "vod" => self.s_vod += base,
                "ch3" => self.s_ch3 += base,
                "art" => self.s_art += base,
                "svc" => self.s_svc += base,
                _ => self.s_ex += base,
            }
            return;
        }
        self.s_tb += base;
        self.s_tv += vat;
        self.s_b20 += base;
        self.s_v20 += vat;
    }

    pub fn add_purch(&mut self, base: i64, vat: i64) {
        if vat == 0 {
            self.p_no += base;
        } else {
            self.p_fb += base;
            self.p_fv += vat;
        }
    }

    pub fn apply_credit(&mut self) {
        if !is_credit_doc(&self.doc_type) {
            return;
        }
        if self.s_tb < 0 || self.p_fb < 0 || self.p_no < 0 {
            return;
        }
        self.negate();
    }

    fn negate(&mut self) {
        for n in [
            &mut self.p_no, &mut self.p_fb, &mut self.p_fv, &mut self.p_pb, &mut self.p_pv,
            &mut self.p_adj, &mut self.p_vop, &mut self.s_tb, &mut self.s_tv, &mut self.s_b20,
            &mut self.s_v20, &mut self.s_vopb, &mut self.s_82, &mut self.s_vopv, &mut self.s_pers,
            &mut self.s_b9, &mut self.s_v9, &mut self.s_ch3, &mut self.s_vod, &mut self.s_art,
            &mut self.s_svc, &mut self.s_69, &mut self.s_ex, &mut self.s_tri,
        ] {
            *n = -*n;
        }
    }

    pub fn to_json(&self, id: i32, row: i32) -> Value {
        let (base, vat) = if self.kind == "purchase" {
            (self.p_no + self.p_fb + self.p_pb, self.p_fv + self.p_pv)
        } else {
            (self.s_tb, self.s_tv)
        };
        json!({
            "id": id,
            "row": row,
            "document_type": self.doc_type,
            "document_number": self.doc_no,
            "document_date": self.doc_date,
            "counterpart_name": self.cp_name,
            "counterpart_vat_number": self.cp_vat,
            "goods": self.goods,
            "delivery_code": self.delivery,
            "base_no_credit": money(self.p_no),
            "base_full_credit": money(self.p_fb),
            "vat_full_credit": money(self.p_fv),
            "base_partial": money(self.p_pb),
            "vat_partial": money(self.p_pv),
            "annual_adj": money(self.p_adj),
            "vop_base": money(self.p_vop),
            "total_base": money(self.s_tb),
            "total_vat": money(self.s_tv),
            "base_20": money(self.s_b20),
            "vat_20": money(self.s_v20),
            "base_9": money(self.s_b9),
            "vat_9": money(self.s_v9),
            "base_0": money(self.s_ch3),
            "base_vod": money(self.s_vod),
            "base_art": money(self.s_art),
            "base_services": money(self.s_svc),
            "base_exempt": money(self.s_ex),
            "base_amount": money(base),
            "vat_amount": money(vat),
        })
    }
}

fn zero_bucket(code: &str) -> &'static str {
    if code.is_empty() {
        return "ex";
    }
    match code {
        "VATEX-EU-IC" => "vod",
        "VATEX-EU-Z-173" => "art",
        "VATEX-EU-S-21" => "svc",
        _ => match vat_ex::category(code) {
            "K" => "vod",
            "G" | "Z" => "ch3",
            "O" => "svc",
            _ => "ex",
        },
    }
}

#[derive(Default)]
pub struct Cells {
    pub s01: i64, pub s20: i64, pub s11: i64, pub s21: i64, pub s12: i64, pub s22: i64,
    pub s23: i64, pub s13: i64, pub s24: i64, pub s14: i64, pub s15: i64, pub s16: i64,
    pub s17: i64, pub s18: i64, pub s19: i64,
    pub p30: i64, pub p31: i64, pub p41: i64, pub p32: i64, pub p42: i64, pub p43: i64,
    pub r40: i64, pub r50: i64, pub r60: i64,
    pub sales_n: i64, pub purch_n: i64,
}

impl Cells {
    pub fn add(&mut self, ln: &Line) {
        if ln.kind == "sales" {
            self.sales_n += 1;
            if sales_excluded(&ln.doc_type) {
                return;
            }
            self.s01 += ln.s_tb;
            self.s20 += ln.s_tv;
            self.s11 += ln.s_b20;
            self.s21 += ln.s_v20;
            self.s12 += ln.s_vopb + ln.s_82;
            self.s22 += ln.s_vopv;
            self.s23 += ln.s_pers;
            self.s13 += ln.s_b9;
            self.s24 += ln.s_v9;
            self.s14 += ln.s_ch3;
            self.s15 += ln.s_vod;
            self.s16 += ln.s_art;
            self.s17 += ln.s_svc;
            self.s18 += ln.s_69 + ln.s_tri;
            self.s19 += ln.s_ex;
            return;
        }
        self.purch_n += 1;
        if purch_excluded(&ln.doc_type) {
            return;
        }
        self.p30 += ln.p_no + ln.p_vop;
        self.p31 += ln.p_fb;
        self.p41 += ln.p_fv;
        self.p32 += ln.p_pb;
        self.p42 += ln.p_pv;
        self.p43 += ln.p_adj;
    }

    pub fn finish(&mut self) {
        self.r40 = self.p41 + self.p43;
        let diff = self.s20 - self.r40;
        if diff < 0 {
            self.r50 = 0;
            self.r60 = -diff;
        } else {
            self.r50 = diff;
            self.r60 = 0;
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "01-01": money(self.s01), "01-20": money(self.s20),
            "01-11": money(self.s11), "01-21": money(self.s21),
            "01-12": money(self.s12), "01-22": money(self.s22),
            "01-23": money(self.s23), "01-13": money(self.s13),
            "01-24": money(self.s24), "01-14": money(self.s14),
            "01-15": money(self.s15), "01-16": money(self.s16),
            "01-17": money(self.s17), "01-18": money(self.s18),
            "01-19": money(self.s19), "01-30": money(self.p30),
            "01-31": money(self.p31), "01-41": money(self.p41),
            "01-32": money(self.p32), "01-42": money(self.p42),
            "01-43": money(self.p43), "01-33": "0.00",
            "01-40": money(self.r40), "01-50": money(self.r50),
            "01-60": money(self.r60), "01-70": "0.00",
            "01-71": "0.00", "01-80": "0.00", "01-81": "0.00", "01-82": "0.00",
        })
    }
}

pub fn pack_json(
    period: &str,
    period_nap: &str,
    name: &str,
    vat: &str,
    submitter: &str,
    purchases: Vec<Value>,
    sales: Vec<Value>,
    cells: &Cells,
) -> Value {
    json!({
        "period": period,
        "period_nap": period_nap,
        "company_name": name,
        "company_vat": vat,
        "submitter": submitter,
        "sales_count": cells.sales_n,
        "purchase_count": cells.purch_n,
        "purchases": purchases,
        "sales": sales,
        "cells": cells.to_json(),
        "due": money(cells.r50),
        "refund": money(cells.r60),
        "cell_01": money(cells.s01),
        "cell_11": money(cells.s11),
        "cell_31": money(cells.s21),
        "cell_41": money(cells.p31),
        "cell_71": money(cells.p41),
        "cell_81": money(cells.r50),
        "cell_82": money(cells.r60),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_month_due() {
        let mut cells = Cells::default();
        let mut sale = Line::from_totals("sales", 100_000, 20_000);
        sale.doc_type = "01".into();
        cells.add(&sale);
        let mut buy = Line::from_totals("purchase", 25_000, 5_000);
        buy.doc_type = "01".into();
        cells.add(&buy);
        cells.finish();
        assert_eq!(money(cells.s11), "1000.00");
        assert_eq!(money(cells.s21), "200.00");
        assert_eq!(money(cells.p31), "250.00");
        assert_eq!(money(cells.p41), "50.00");
        assert_eq!(money(cells.r50), "150.00");
        assert_eq!(cells.r60, 0);
    }

    #[test]
    fn credit_flips_once() {
        let mut ln = Line::from_totals("sales", 5_000, 1_000);
        ln.doc_type = "03".into();
        ln.apply_credit();
        assert_eq!(ln.s_tb, -5_000);
        ln.apply_credit();
        assert_eq!(ln.s_tb, -5_000);
    }

    #[test]
    fn zero_rate_vod() {
        let mut ln = Line::empty("sales");
        ln.add_sale(8_000, 0, "0", "VATEX-EU-IC");
        assert_eq!(ln.s_vod, 8_000);
        assert_eq!(ln.s_tv, 0);
    }
}
