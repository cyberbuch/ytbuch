//! Финансови отчети по формули. Празен низ е нула, освен сборът на група.
//! Предходната година още е нула.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::reports_data::{self, acc_match, by_number, correspondences, money, turns, Books, Built};
use crate::reports_nom::{self, Line};

#[derive(Clone, Debug, PartialEq)]
enum Piece {
    At(String),
    Acct { pat: String, which: String },
    Corr { debit: String, credit: String },
}

#[derive(Clone, Debug, PartialEq)]
struct Tok {
    neg: bool,
    piece: Piece,
}

pub fn check(raw: &str) -> Result<usize, String> {
    parse(raw).map(|t| t.len())
}

fn parse(raw: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = raw.trim().chars().collect();
    if chars.is_empty() {
        return Ok(Vec::new());
    }
    let mut i = 0;
    let mut first = true;
    let mut out = Vec::new();
    while i < chars.len() {
        i = skip(&chars, i);
        if i >= chars.len() {
            break;
        }
        let mut neg = false;
        if chars[i] == '+' {
            i += 1;
            first = false;
        } else if chars[i] == '-' || chars[i] == '−' {
            neg = true;
            i += 1;
            first = false;
        } else if first {
            first = false;
        } else {
            return Err("очакван знак + или −".into());
        }
        i = skip(&chars, i);
        if i >= chars.len() {
            return Err("формулата свършва след знака".into());
        }
        let piece = if chars[i] == '@' {
            i += 1;
            let (code, n) = read_until(&chars, i, |c| c == '+' || c == '-' || c == '−' || c == ' ' || c == '\t');
            if code.is_empty() {
                return Err("липсва код след @".into());
            }
            i = n;
            Piece::At(code)
        } else {
            let (acc, n) = read_acc(&chars, i);
            if acc.is_empty() {
                return Err("липсва сметка".into());
            }
            i = n;
            let Some((kind, kn)) = kind_at(&chars, i) else {
                return Err(format!("липсва Д/К/ОД/ОК/НД/НК след {acc}"));
            };
            i += kn;
            i = skip(&chars, i);
            if i < chars.len() && chars[i] == '/' {
                i += 1;
                i = skip(&chars, i);
                let (acc2, n2) = read_acc(&chars, i);
                if acc2.is_empty() {
                    return Err("липсва кредитна сметка след /".into());
                }
                i = n2;
                let Some((_, kn2)) = kind_at(&chars, i) else {
                    return Err("липсва Д или К след втората сметка".into());
                };
                i += kn2;
                Piece::Corr { debit: acc, credit: acc2 }
            } else {
                Piece::Acct { pat: acc, which: kind.into() }
            }
        };
        out.push(Tok { neg, piece });
    }
    Ok(out)
}

fn skip(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
        i += 1;
    }
    i
}

fn read_acc(chars: &[char], i: usize) -> (String, usize) {
    let mut p = i;
    while p < chars.len() && (chars[p].is_ascii_digit() || chars[p] == '*') {
        p += 1;
    }
    (chars[i..p].iter().collect(), p)
}

fn read_until(chars: &[char], i: usize, stop: impl Fn(char) -> bool) -> (String, usize) {
    let mut p = i;
    while p < chars.len() && !stop(chars[p]) {
        p += 1;
    }
    (chars[i..p].iter().collect(), p)
}

fn kind_at(chars: &[char], i: usize) -> Option<(&'static str, usize)> {
    let rest: String = chars[i..].iter().collect();
    for (src, code) in [("ОД", "TD"), ("ОК", "TC"), ("НД", "OD"), ("НК", "OC"), ("Д", "D"), ("К", "C")] {
        if rest.starts_with(src) {
            return Some((code, src.chars().count()));
        }
    }
    None
}

fn amount_of(which: &str, num: &str, maps: &[HashMap<String, i128>; 4]) -> i128 {
    let g = |i: usize| maps[i].get(num).copied().unwrap_or(0);
    match which {
        "D" => g(0).saturating_add(g(2)),
        "C" => g(1).saturating_add(g(3)),
        "TD" => g(2),
        "TC" => g(3),
        "OD" => g(0),
        "OC" => g(1),
        _ => 0,
    }
}

fn sum_accounts(pat: &str, which: &str, books: &Books, maps: &[HashMap<String, i128>; 4]) -> i128 {
    let mut seen = HashSet::new();
    let mut total = 0i128;
    for acc in &books.accounts {
        if !seen.insert(acc.number.clone()) {
            continue;
        }
        if acc_match(&acc.number, pat) {
            total = total.saturating_add(amount_of(which, &acc.number, maps));
        }
    }
    total
}

fn sum_corr(debit: &str, credit: &str, pairs: &HashMap<(String, String), i128>) -> i128 {
    let mut total = 0i128;
    for ((d, c), amount) in pairs {
        if acc_match(d, debit) && acc_match(c, credit) {
            total = total.saturating_add(*amount);
        }
    }
    total
}

fn child_sum(cat: &[Line], parent: &str, self_code: &str, done: &HashSet<String>, val: &HashMap<String, i128>) -> Option<i128> {
    let mut total = 0i128;
    for line in cat {
        if line.parent != parent || line.code == self_code {
            continue;
        }
        if line.kind != "line" && line.kind != "total" {
            continue;
        }
        if !done.contains(&line.code) {
            return None;
        }
        total = total.saturating_add(val.get(&line.code).copied().unwrap_or(0));
    }
    Some(total)
}

pub fn build(kind: &str, books: &Books, formulas: &HashMap<String, String>) -> Built {
    let statement = reports_data::fs_statement(kind).unwrap_or("");
    let cat = reports_nom::lines(statement);
    let turn = turns(books);
    let maps = by_number(books, &turn);
    let pairs = correspondences(books);
    let mut done = HashSet::new();
    let mut val: HashMap<String, i128> = HashMap::new();
    for _ in 0..cat.len().saturating_add(4) {
        let mut progress = false;
        for line in &cat {
            if done.contains(&line.code) {
                continue;
            }
            let formula = formulas.get(&line.code).map(String::as_str).unwrap_or("");
            if formula.is_empty() {
                if line.kind == "total" {
                    if let Some(sum) = child_sum(&cat, &line.parent, &line.code, &done, &val) {
                        val.insert(line.code.clone(), sum);
                        done.insert(line.code.clone());
                        progress = true;
                    }
                } else {
                    val.insert(line.code.clone(), 0);
                    done.insert(line.code.clone());
                    progress = true;
                }
                continue;
            }
            let toks = match parse(formula) {
                Ok(t) => t,
                Err(_) => {
                    val.insert(line.code.clone(), 0);
                    done.insert(line.code.clone());
                    progress = true;
                    continue;
                }
            };
            let waiting = toks.iter().any(|t| matches!(&t.piece, Piece::At(code) if !done.contains(code)));
            if waiting {
                continue;
            }
            let mut acc = 0i128;
            for tok in &toks {
                let piece = match &tok.piece {
                    Piece::At(code) => val.get(code).copied().unwrap_or(0),
                    Piece::Acct { pat, which } => sum_accounts(pat, which, books, &maps),
                    Piece::Corr { debit, credit } => sum_corr(debit, credit, &pairs),
                };
                acc = if tok.neg { acc.saturating_sub(piece) } else { acc.saturating_add(piece) };
            }
            val.insert(line.code.clone(), acc);
            done.insert(line.code.clone());
            progress = true;
        }
        if !progress {
            break;
        }
    }
    let mut rows = Vec::new();
    let mut grid = vec![
        vec![reports_data::title(kind).to_string()],
        vec![format!("{} — {}", books.from, books.to)],
        vec!["Статия".into(), "Текуща".into(), "Предходна".into()],
    ];
    for line in &cat {
        let amt = val.get(&line.code).copied().unwrap_or(0);
        rows.push(json!({
            "code": line.code,
            "parent": line.parent,
            "kind": line.kind,
            "side": line.side,
            "name": line.name,
            "current": money(amt),
            "prior": money(0),
        }));
        if line.kind == "header" {
            grid.push(vec![line.name.clone(), String::new(), String::new()]);
        } else {
            grid.push(vec![line.name.clone(), money(amt), money(0)]);
        }
    }
    let mut body = reports_data::head(books, kind);
    body["statement"] = json!(statement);
    body["rows"] = Value::Array(rows);
    Built {
        body,
        slug: reports_data::slug(kind).into(),
        grid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_account_ref_and_corr() {
        let t = parse("+304Д -241К").unwrap();
        assert_eq!(t.len(), 2);
        assert!(!t[0].neg);
        assert!(t[1].neg);
        let c = parse("702Д/304К").unwrap();
        assert_eq!(
            c[0].piece,
            Piece::Corr { debit: "702".into(), credit: "304".into() }
        );
        let r = parse("+601ОД+@pl.e.5a").unwrap();
        assert!(matches!(r[0].piece, Piece::Acct { ref which, .. } if which == "TD"));
        assert!(matches!(r[1].piece, Piece::At(ref s) if s == "pl.e.5a"));
        assert!(parse("+304").is_err());
    }
}
