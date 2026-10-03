//! Суми с 6 знака вътре. Навън: пари с 2 знака, количество с 4.
//! Празен или нечетим низ е нула при `or0`.

const SCALE: i128 = 1_000_000;

pub fn or0(raw: &str) -> i128 {
    parse(raw).unwrap_or(0)
}

pub fn parse(raw: &str) -> Option<i128> {
    let s = raw.trim().replace(' ', "").replace('\u{00a0}', "");
    if s.is_empty() {
        return Some(0);
    }
    let neg = s.starts_with('-');
    let s = s.trim_start_matches(['+', '-']);
    if s.is_empty() {
        return None;
    }
    let (whole, frac) = if let Some((w, f)) = s.split_once('.') {
        (w, f)
    } else if let Some((w, f)) = s.split_once(',') {
        (w, f)
    } else {
        (s, "")
    };
    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if whole.is_empty() && frac.is_empty() {
        return None;
    }
    let whole: i128 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
    let mut frac_v: i128 = 0;
    let mut place = SCALE / 10;
    for (i, b) in frac.bytes().enumerate() {
        if i == 6 {
            if b >= b'5' {
                frac_v += 1;
            }
            break;
        }
        frac_v += i128::from(b - b'0') * place;
        place /= 10;
    }
    let mut v = whole.checked_mul(SCALE)?.checked_add(frac_v)?;
    if neg {
        v = v.checked_neg()?;
    }
    Some(v)
}

pub fn add(a: i128, b: i128) -> Option<i128> {
    a.checked_add(b)
}

pub fn sub(a: i128, b: i128) -> Option<i128> {
    a.checked_sub(b)
}

pub fn mul(a: i128, b: i128) -> Option<i128> {
    let p = a.checked_mul(b)?;
    let half = SCALE / 2;
    let p = if p >= 0 { p.checked_add(half)? } else { p.checked_sub(half)? };
    Some(p / SCALE)
}

pub fn div(a: i128, b: i128) -> Option<i128> {
    if b == 0 {
        return None;
    }
    let num = a.checked_mul(SCALE)?;
    let half = b.abs() / 2;
    let num = if num >= 0 { num.checked_add(half)? } else { num.checked_sub(half)? };
    Some(num / b)
}

fn round_to(v: i128, dp: u32) -> i128 {
    let unit = 10i128.pow(6 - dp);
    let half = unit / 2;
    if v >= 0 {
        (v + half) / unit * unit
    } else {
        -(((-v) + half) / unit * unit)
    }
}

pub fn text(v: i128, dp: u32) -> String {
    let r = round_to(v, dp);
    let neg = r < 0;
    let u = r.unsigned_abs();
    let scale = SCALE as u128;
    let ip = u / scale;
    let frac = u % scale;
    let frac_s = format!("{frac:06}");
    let s = format!("{ip}.{}", &frac_s[..dp as usize]);
    if neg { format!("-{s}") } else { s }
}

pub fn money(v: i128) -> String {
    text(v, 2)
}

pub fn qty(v: i128) -> String {
    text(v, 4)
}

pub fn abs(v: i128) -> i128 {
    v.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_money() {
        assert_eq!(money(parse("1.005").unwrap()), "1.01");
        assert_eq!(money(parse("-10").unwrap()), "-10.00");
        assert_eq!(qty(parse("1.23456").unwrap()), "1.2346");
    }
}
