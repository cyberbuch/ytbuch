//! Суми като низ и знак на кредитно известие.
//! Черното сторно е минус върху същите Дт/Кт. Страните не се разменят.

pub fn is_credit(t: &str) -> bool {
    matches!(t, "03" | "13" | "23")
}

pub fn is_proforma(t: &str) -> bool {
    t == "proforma"
}

pub fn requires_original(t: &str) -> bool {
    matches!(t, "02" | "03" | "12" | "13" | "23")
}

pub fn valid_for(direction: &str, t: &str) -> bool {
    if is_proforma(t) {
        return true;
    }
    let common = ["01", "02", "03", "07", "09", "11", "12", "13"];
    if direction == "out" {
        return common.contains(&t)
            || matches!(t, "04" | "23" | "29" | "50" | "81" | "82" | "83" | "84" | "85" | "91" | "93" | "94" | "95")
    }
    common.contains(&t) || matches!(t, "05" | "23" | "91" | "92" | "93" | "94")
}

pub fn title(t: &str) -> &'static str {
    match t {
        "02" => "ДЕБИТНО ИЗВЕСТИЕ",
        "03" => "КРЕДИТНО ИЗВЕСТИЕ",
        "04" | "05" => "РЕГИСТЪР НА СТОКИ ПОД РЕЖИМ СКЛАДИРАНЕ ДО ПОИСКВАНЕ",
        "07" => "МИТНИЧЕСКА ДЕКЛАРАЦИЯ",
        "09" => "ПРОТОКОЛ",
        "11" => "ФАКТУРА - КАСОВА ОТЧЕТНОСТ",
        "12" => "ДЕБИТНО ИЗВЕСТИЕ - КАСОВА ОТЧЕТНОСТ",
        "13" => "КРЕДИТНО ИЗВЕСТИЕ - КАСОВА ОТЧЕТНОСТ",
        "23" => "КРЕДИТНО ИЗВЕСТИЕ ПО ЧЛ. 126Б, АЛ. 1 ОТ ЗДДС",
        "29" => "ПРОТОКОЛ ПО ЧЛ. 126Б, АЛ. 2 И 7 ОТ ЗДДС",
        "50" => "ПРОТОКОЛ ЗА ИЗИСКУЕМИЯ ДДС ЗА ГОРИВА С КОМПЕНСАЦИИ",
        "81" | "83" | "84" | "85" => "ОТЧЕТ ЗА ИЗВЪРШЕНИТЕ ПРОДАЖБИ",
        "82" => "ОТЧЕТ ЗА ПРОДАЖБИ ПРИ СПЕЦИАЛЕН РЕД НА ОБЛАГАНЕ",
        "91" | "92" | "93" | "94" => "ПРОТОКОЛ ЗА ДАНЪК ПО ГЛАВА ДВАНАДЕСЕТА ОТ ЗДДС",
        "95" => "ПРОТОКОЛ ЗА БЕЗВЪЗМЕЗДНО ПРЕДОСТАВЯНЕ НА ХРАНИТЕЛНИ СТОКИ",
        "proforma" => "ПРОФОРМА ФАКТУРА",
        _ => "ФАКТУРА",
    }
}

/// Стотинки. `None`, ако низът не е число. Третият знак след точката закръгля.
pub fn cents(raw: &str) -> Option<i64> {
    let s = raw.trim().replace(' ', "").replace(',', ".");
    if s.is_empty() {
        return Some(0);
    }
    let neg = s.starts_with('-');
    let s = s.trim_start_matches(['+', '-']);
    if s.is_empty() {
        return None;
    }
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if whole.is_empty() && frac.is_empty() {
        return None;
    }
    let whole: i64 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
    let bytes = frac.as_bytes();
    let mut frac_cents: i64 = 0;
    if !bytes.is_empty() {
        frac_cents += i64::from(bytes[0] - b'0') * 10;
    }
    if bytes.len() > 1 {
        frac_cents += i64::from(bytes[1] - b'0');
    }
    if bytes.len() > 2 && bytes[2] >= b'5' {
        frac_cents += 1;
    }
    let mut v = whole.checked_mul(100)?.checked_add(frac_cents)?;
    if neg {
        v = v.checked_neg()?;
    }
    Some(v)
}

pub fn money(cents: i64) -> String {
    let neg = cents < 0;
    let v = cents.unsigned_abs();
    let s = format!("{}.{:02}", v / 100, v % 100);
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

/// Кредитно и още плюс: един минус. Вече отрицателна или нула не се пипа.
pub fn amount_for_doc(doc_type: &str, amount: &str) -> String {
    if !is_credit(doc_type) {
        return amount.to_string();
    }
    match cents(amount) {
        Some(c) if c > 0 => money(-c),
        _ => amount.to_string(),
    }
}

pub fn sum_amounts(amounts: &[String]) -> String {
    let total = amounts.iter().map(|a| cents(a).unwrap_or(0)).sum();
    money(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credit_negates_once() {
        assert_eq!(amount_for_doc("03", "120.00"), "-120.00");
        assert_eq!(amount_for_doc("03", "-120.00"), "-120.00");
        assert_eq!(amount_for_doc("03", "0.00"), "0.00");
        assert_eq!(amount_for_doc("01", "120.00"), "120.00");
        assert_eq!(amount_for_doc("13", "10"), "-10.00");
    }

    #[test]
    fn sums_strings() {
        assert_eq!(sum_amounts(&["1000.00".into(), "200.50".into()]), "1200.50");
        assert_eq!(sum_amounts(&["-10.00".into(), "3.00".into()]), "-7.00");
        assert_eq!(cents("1.005"), Some(101));
    }
}
