//! One-time codes from an authenticator app (TOTP, RFC 6238: SHA-1, 6 digits,
//! 30 seconds), for the admin sign-in.

use hmac::{Hmac, Mac};
use sha1::Sha1;

const STEP_SECONDS: i64 = 30;
const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn base32_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            out.push(BASE32[((buffer >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(BASE32[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn base32_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for c in text.chars().filter(|c| !c.is_whitespace() && *c != '=') {
        let v = BASE32.iter().position(|&b| b as char == c.to_ascii_uppercase())? as u32;
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push((buffer >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

/// A new random secret (160 bits), base32 for the authenticator app.
pub fn new_secret() -> String {
    use rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 20];
    OsRng.fill_bytes(&mut bytes);
    base32_encode(&bytes)
}

fn code_at(key: &[u8], step: i64) -> u32 {
    let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(&step.to_be_bytes());
    let hash = mac.finalize().into_bytes();
    let offset = (hash[19] & 0x0f) as usize;
    let value = u32::from_be_bytes([hash[offset] & 0x7f, hash[offset + 1], hash[offset + 2], hash[offset + 3]]);
    value % 1_000_000
}

/// The time step a code belongs to (now, or one step before/after for clock
/// drift), if it is valid and newer than `last_step` (a code works only once).
pub fn verify(secret: &str, code: &str, unix_time: i64, last_step: i64) -> Option<i64> {
    let code = code.trim().replace(' ', "");
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let wanted: u32 = code.parse().ok()?;
    let key = base32_decode(secret)?;
    let now = unix_time / STEP_SECONDS;
    (now - 1..=now + 1).find(|&step| step > last_step && code_at(&key, step) == wanted)
}

/// otpauth:// link for authenticator apps (as QR code or typed in).
/// `issuer` is the name the app shows above the code ("Sowling", "Sowling Admin").
pub fn provisioning_uri(secret: &str, account: &str, issuer: &str) -> String {
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    let issuer = enc(issuer);
    format!("otpauth://totp/{issuer}:{}?secret={secret}&issuer={issuer}&digits=6&period=30", enc(account))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_reference_codes() {
        // RFC 6238 appendix B, SHA-1 secret "12345678901234567890" (8-digit
        // values; the last 6 digits are what 6-digit apps show).
        let secret = base32_encode(b"12345678901234567890");
        assert_eq!(secret, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        let key = base32_decode(&secret).unwrap();
        assert_eq!(code_at(&key, 59 / 30), 287_082);
        assert_eq!(code_at(&key, 1_111_111_109 / 30), 81_804);
        assert_eq!(code_at(&key, 2_000_000_000 / 30), 279_037);
    }

    #[test]
    fn codes_work_once_and_only_near_now() {
        let secret = base32_encode(b"12345678901234567890");
        assert_eq!(verify(&secret, "287 082", 59, 0), Some(1));
        assert_eq!(verify(&secret, "287082", 59, 1), None, "replay of the same step");
        assert_eq!(verify(&secret, "287082", 59 + 30, 0), Some(1), "one step of drift is fine");
        assert_eq!(verify(&secret, "287082", 59 + 300, 0), None, "too old");
        assert_eq!(verify(&secret, "28708", 59, 0), None);
        assert!(provisioning_uri(&secret, "a+b@example.com", "Sowling Admin").contains("a%2Bb%40example.com"));
    }
}
