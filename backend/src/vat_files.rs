//! DEKLAR.TXT, POKUPKI.TXT, PRODAGBI.TXT и ZIP без компресия.
//! Редовете са Windows-1251, фиксирана ширина, CRLF.

use crate::vat_fmt::{self, amt, sym};
use crate::vat_line::{Cells, Line};

fn crlf() -> Vec<u8> {
    vec![13, 10]
}

fn head(vat: &str, period: &str, branch: &str) -> Vec<u8> {
    let mut b = sym(&vat_fmt::with_bg(vat), 15);
    b.extend(sym(period, 6));
    let trimmed = branch.trim();
    let br = if trimmed.is_empty() { "0" } else { trimmed };
    b.extend(vat_fmt::num_str(br, 4));
    b
}

fn push_amts(b: &mut Vec<u8>, vals: &[i64]) {
    for v in vals {
        b.extend(amt(*v));
    }
}

pub fn pok_line(vat: &str, period: &str, branch: &str, row: i64, ln: &Line) -> Vec<u8> {
    let mut b = head(vat, period, branch);
    b.extend(vat_fmt::num_str(&row.to_string(), 15));
    b.extend(sym(&ln.doc_type, 2));
    b.extend(sym(&ln.doc_no, 20));
    b.extend(sym(&vat_fmt::nap_date(&ln.doc_date), 10));
    b.extend(sym(&ln.cp_vat, 15));
    b.extend(sym(&ln.cp_name, 50));
    b.extend(sym(&ln.goods, 30));
    push_amts(&mut b, &[ln.p_no, ln.p_fb, ln.p_fv, ln.p_pb, ln.p_pv, ln.p_adj, ln.p_vop]);
    b.extend(sym(&ln.delivery, 2));
    b.extend(crlf());
    b
}

pub fn pro_line(vat: &str, period: &str, branch: &str, row: i64, ln: &Line) -> Vec<u8> {
    let mut b = head(vat, period, branch);
    b.extend(vat_fmt::num_str(&row.to_string(), 15));
    b.extend(sym(&ln.doc_type, 2));
    b.extend(sym(&ln.doc_no, 20));
    b.extend(sym(&vat_fmt::nap_date(&ln.doc_date), 10));
    b.extend(sym(&ln.cp_vat, 15));
    b.extend(sym(&ln.cp_name, 50));
    b.extend(sym(&ln.goods, 30));
    push_amts(
        &mut b,
        &[
            ln.s_tb, ln.s_tv, ln.s_b20, ln.s_v20, ln.s_vopb, ln.s_82, ln.s_vopv, ln.s_pers,
            ln.s_b9, ln.s_v9, ln.s_ch3, ln.s_vod, ln.s_art, ln.s_svc, ln.s_69, ln.s_ex, ln.s_tri,
        ],
    );
    b.extend(sym(&ln.delivery, 2));
    b.extend(crlf());
    b
}

fn person(egn: &str, name: &str) -> String {
    let e = egn.trim();
    let n = name.trim();
    if !e.is_empty() && !n.is_empty() {
        format!("{e}/{n}")
    } else if !n.is_empty() {
        n.to_string()
    } else {
        e.to_string()
    }
}

pub fn submitter(accountant_egn: &str, accountant_name: &str, auth_egn: &str, auth_name: &str, manager_egn: &str, manager_name: &str) -> String {
    if !accountant_name.trim().is_empty() || !accountant_egn.trim().is_empty() {
        return person(accountant_egn, accountant_name);
    }
    if !auth_name.trim().is_empty() || !auth_egn.trim().is_empty() {
        return person(auth_egn, auth_name);
    }
    person(manager_egn, manager_name)
}

pub fn deklar(vat: &str, name: &str, period: &str, submitter: &str, c: &Cells) -> Vec<u8> {
    let mut b = sym(&vat_fmt::with_bg(vat), 15);
    b.extend(sym(name, 50));
    b.extend(sym(period, 6));
    b.extend(sym(submitter, 50));
    b.extend(vat_fmt::num_str(&c.sales_n.to_string(), 15));
    b.extend(vat_fmt::num_str(&c.purch_n.to_string(), 15));
    push_amts(
        &mut b,
        &[
            c.s01, c.s20, c.s11, c.s21, c.s12, c.s22, c.s23, c.s13, c.s24, c.s14, c.s15, c.s16,
            c.s17, c.s18, c.s19, c.p30, c.p31, c.p41, c.p32, c.p42, c.p43,
        ],
    );
    let coef = {
        let t = c.m33.trim();
        if t.is_empty() { "0.00".to_string() } else { t.to_string() }
    };
    b.extend(sym(&coef, 4));
    push_amts(
        &mut b,
        &[c.r40, c.r50, c.r60, c.m70, c.m71, c.m80, c.m81, c.m82],
    );
    b.extend(crlf());
    b
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn le_u16(n: u16) -> [u8; 2] {
    n.to_le_bytes()
}

fn le_u32(n: u32) -> [u8; 4] {
    n.to_le_bytes()
}

pub fn zip3(deklar: &[u8], pok: &[u8], pro: &[u8]) -> Vec<u8> {
    let files = [("DEKLAR.TXT", deklar), ("POKUPKI.TXT", pok), ("PRODAGBI.TXT", pro)];
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let name_b = name.as_bytes();
        let crc = crc32(data);
        let offset = out.len() as u32;
        let size = data.len() as u32;
        out.extend(le_u32(0x0403_4B50));
        out.extend(le_u16(20));
        out.extend(le_u16(0));
        out.extend(le_u16(0));
        out.extend(le_u16(0));
        out.extend(le_u16(0));
        out.extend(le_u32(crc));
        out.extend(le_u32(size));
        out.extend(le_u32(size));
        out.extend(le_u16(name_b.len() as u16));
        out.extend(le_u16(0));
        out.extend(name_b);
        out.extend(data);
        central.extend(le_u32(0x0201_4B50));
        central.extend(le_u16(20));
        central.extend(le_u16(20));
        central.extend(le_u16(0));
        central.extend(le_u16(0));
        central.extend(le_u16(0));
        central.extend(le_u16(0));
        central.extend(le_u32(crc));
        central.extend(le_u32(size));
        central.extend(le_u32(size));
        central.extend(le_u16(name_b.len() as u16));
        central.extend(le_u16(0));
        central.extend(le_u16(0));
        central.extend(le_u16(0));
        central.extend(le_u16(0));
        central.extend(le_u32(0));
        central.extend(le_u32(offset));
        central.extend(name_b);
    }
    let cd_off = out.len() as u32;
    let cd_len = central.len() as u32;
    out.extend(central);
    out.extend(le_u32(0x0605_4B50));
    out.extend(le_u16(0));
    out.extend(le_u16(0));
    out.extend(le_u16(3));
    out.extend(le_u16(3));
    out.extend(le_u32(cd_len));
    out.extend(le_u32(cd_off));
    out.extend(le_u16(0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoice_money::money;
    use crate::vat_line::Line;

    #[test]
    fn widths() {
        let ln = Line::empty("purchase");
        assert_eq!(pok_line("BG1", "202610", "", 1, &ln).len(), 276);
        let sale = Line::empty("sales");
        assert_eq!(pro_line("BG1", "202610", "", 1, &sale).len(), 426);
        let cells = Cells::default();
        assert_eq!(deklar("BG1", "Бага", "202610", "Иван", &cells).len(), 592);
        assert_eq!(money(0), "0.00");
    }

    #[test]
    fn zip_opens() {
        let z = zip3(b"a", b"b", b"c");
        assert_eq!(&z[..4], b"PK\x03\x04");
        assert!(z.windows(4).any(|w| w == b"PK\x05\x06"));
    }
}
