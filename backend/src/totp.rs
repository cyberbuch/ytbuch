//! TOTP като otpbaga: SHA1, 6 цифри, 30 секунди, прозорец ±1.
//! Резервният код е 8 байта hex, показан като `aaaaaaaa-bbbbbbbb`, а в базата стои SHA-256.

use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};

const ALPH: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    let mut file = std::fs::File::open("/dev/urandom").map_err(|e| e.to_string())?;
    std::io::Read::read_exact(&mut file, &mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

pub fn b32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut buf: u64 = 0;
    let mut bits: i32 = 0;
    for byte in data {
        buf = (buf << 8) | u64::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buf >> bits) & 31) as usize;
            out.push(ALPH[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buf << (5 - bits)) & 31) as usize;
        out.push(ALPH[idx] as char);
    }
    out
}

pub fn b32_decode(text: &str) -> Result<Vec<u8>, ()> {
    let mut buf: u64 = 0;
    let mut bits: i32 = 0;
    let mut out = Vec::new();
    for ch in text.chars() {
        if ch == ' ' || ch == '=' {
            continue;
        }
        let up = ch.to_ascii_uppercase() as u8;
        let Some(pos) = ALPH.iter().position(|c| *c == up) else {
            return Err(());
        };
        buf = (buf << 5) | pos as u64;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

fn hotp(secret: &[u8], counter: u64) -> String {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("hmac key");
    mac.update(&counter.to_be_bytes());
    let hash = mac.finalize().into_bytes();
    let off = (hash[19] & 0x0f) as usize;
    let bin = u32::from_be_bytes([
        hash[off] & 0x7f,
        hash[off + 1],
        hash[off + 2],
        hash[off + 3],
    ]);
    format!("{:06}", bin % 1_000_000)
}

pub fn totp_ok(secret_b32: &str, code: &str, unix: i64) -> bool {
    let Ok(secret) = b32_decode(secret_b32) else {
        return false;
    };
    if secret.is_empty() {
        return false;
    }
    let want: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if want.len() != 6 {
        return false;
    }
    let slot = unix.max(0) / 30;
    for delta in -1i64..=1 {
        let counter = slot + delta;
        if counter >= 0 && hotp(&secret, counter as u64) == want {
            return true;
        }
    }
    false
}

fn pct(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        let ok = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~');
        if ok {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn otpauth_uri(secret_b32: &str, account: &str) -> String {
    let label = format!("{}:{}", pct("ytbuch"), pct(account));
    format!(
        "otpauth://totp/{label}?secret={secret_b32}&issuer={}&algorithm=SHA1&digits=6&period=30",
        pct("ytbuch")
    )
}

pub fn qr_svg(uri: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(uri.as_bytes()).map_err(|e| e.to_string())?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(220, 220)
        .build();
    Ok(svg)
}

pub fn new_secret() -> Result<String, String> {
    Ok(b32_encode(&random_bytes(20)?))
}

fn recovery_normalize(code: &str) -> String {
    code.chars()
        .filter(|c| *c != ' ' && *c != '-')
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_ascii_hexdigit())
        .collect()
}

pub fn recovery_hash(code: &str) -> String {
    let norm = recovery_normalize(code);
    if norm.is_empty() {
        return String::new();
    }
    hex::encode(Sha256::digest(norm.as_bytes()))
}

/// Десет кода и техните хешове. Празният низ след нормализиране не се приема при проверка.
pub fn recovery_make() -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::with_capacity(10);
    for _ in 0..10 {
        let raw = hex::encode(random_bytes(8)?);
        let code = format!("{}-{}", &raw[..8], &raw[8..]);
        let hash = recovery_hash(&code);
        out.push((code, hash));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::hotp;

    #[test]
    fn hotp_rfc4226_counter_zero() {
        assert_eq!(hotp(b"12345678901234567890", 0), "755224");
    }
}
