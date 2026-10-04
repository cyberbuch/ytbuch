//! Словом на български. Същите думи като екрана на фактурата.

const ONES: [&str; 10] = ["", "един", "два", "три", "четири", "пет", "шест", "седем", "осем", "девет"];
const ONES_F: [&str; 10] = ["", "една", "две", "три", "четири", "пет", "шест", "седем", "осем", "девет"];
const TEENS: [&str; 10] = [
    "десет",
    "единадесет",
    "дванадесет",
    "тринадесет",
    "четиринадесет",
    "петнадесет",
    "шестнадесет",
    "седемнадесет",
    "осемнадесет",
    "деветнадесет",
];
const TENS: [&str; 10] = [
    "",
    "",
    "двадесет",
    "тридесет",
    "четиридесет",
    "петдесет",
    "шестдесет",
    "седемдесет",
    "осемдесет",
    "деветдесет",
];
const HUNDREDS: [&str; 10] = [
    "",
    "сто",
    "двеста",
    "триста",
    "четиристотин",
    "петстотин",
    "шестстотин",
    "седемстотин",
    "осемстотин",
    "деветстотин",
];

pub fn words(amount: &str, currency: &str) -> String {
    let raw = amount.trim().replace(',', ".");
    let neg = raw.starts_with('-');
    let raw = raw.trim_start_matches(['+', '-']);
    let (whole_s, frac_s) = raw.split_once('.').unwrap_or((raw, ""));
    let whole: i64 = whole_s.parse().unwrap_or(0).max(0);
    let cents = cents_of(frac_s);
    let female = currency == "BGN";
    let (cur, cent_name) = money_name(currency, whole);
    let mut out = format!("{} {cur}", integer_words(whole, female));
    if cents > 0 {
        out.push_str(&format!(" и {cents:02} {cent_name}"));
    }
    if neg {
        out = format!("минус {out}");
    }
    out
}

fn money_name(currency: &str, whole: i64) -> (String, &'static str) {
    if currency == "BGN" {
        let cur = if whole == 1 { "лев" } else { "лева" };
        return (cur.into(), "стотинки");
    }
    if currency == "EUR" {
        return ("евро".into(), "евроцента");
    }
    (currency.to_string(), "стотинки")
}

fn cents_of(frac: &str) -> i64 {
    let b = frac.as_bytes();
    let d0 = digit(b.first().copied());
    let d1 = digit(b.get(1).copied());
    d0 * 10 + d1
}

fn digit(b: Option<u8>) -> i64 {
    match b {
        Some(c) if c.is_ascii_digit() => i64::from(c - b'0'),
        _ => 0,
    }
}

fn integer_words(n: i64, female: bool) -> String {
    if n <= 0 {
        return "нула".into();
    }
    if n < 1_000_000 {
        return below_million(n, female);
    }
    if n >= 1_000_000_000 {
        return n.to_string();
    }
    let mil = n / 1_000_000;
    let rest = n % 1_000_000;
    let head = if mil == 1 {
        "един милион".into()
    } else {
        format!("{} милиона", hundreds(mil, false))
    };
    if rest == 0 {
        return head;
    }
    if rest < 1000 {
        return format!("{head} и {}", below_million(rest, female));
    }
    format!("{head} {}", below_million(rest, female))
}

fn below_million(n: i64, female: bool) -> String {
    if n <= 0 {
        return String::new();
    }
    if n < 1000 {
        return hundreds(n, female);
    }
    let th = n / 1000;
    let rest = n % 1000;
    let head = if th == 1 {
        "хиляда".into()
    } else if th == 2 {
        "две хиляди".into()
    } else {
        format!("{} хиляди", hundreds(th, true))
    };
    if rest == 0 {
        return head;
    }
    if rest < 100 {
        return format!("{head} и {}", hundreds(rest, female));
    }
    format!("{head} {}", hundreds(rest, female))
}

fn hundreds(n: i64, female: bool) -> String {
    let ones = if female { ONES_F } else { ONES };
    if n <= 0 {
        return String::new();
    }
    if n < 10 {
        return ones[n as usize].into();
    }
    if n < 20 {
        return TEENS[(n - 10) as usize].into();
    }
    if n < 100 {
        let ten = (n / 10) as usize;
        let one = (n % 10) as usize;
        if one == 0 {
            return TENS[ten].into();
        }
        return format!("{} и {}", TENS[ten], ones[one]);
    }
    let h = (n / 100) as usize;
    let rest = n % 100;
    if rest == 0 {
        return HUNDREDS[h].into();
    }
    if rest < 10 {
        return format!("{} и {}", HUNDREDS[h], ones[rest as usize]);
    }
    if rest < 20 {
        return format!("{} и {}", HUNDREDS[h], TEENS[(rest - 10) as usize]);
    }
    let ten = (rest / 10) as usize;
    let one = (rest % 10) as usize;
    if one == 0 {
        return format!("{} и {}", HUNDREDS[h], TENS[ten]);
    }
    format!("{} {} и {}", HUNDREDS[h], TENS[ten], ones[one])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euro_and_leva() {
        assert_eq!(words("1200.00", "EUR"), "хиляда двеста евро");
        assert_eq!(words("1.05", "EUR"), "един евро и 05 евроцента");
        assert_eq!(words("-10.00", "BGN"), "минус десет лева");
        assert_eq!(words("1.00", "BGN"), "една лев");
        assert_eq!(words("21", "BGN"), "двадесет и една лева");
    }
}
