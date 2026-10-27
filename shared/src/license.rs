//! Desktop licensing, shared by the backend (issues and signs) and the desktop
//! app (verifies). Enabled with the `license` feature.
//!
//! Two artefacts:
//! - **License keys** (`SOWL-XXXX-XXXX-XXXX-XXXX`): what a person types in.
//!   80 random bits, Crockford base32 (no I/L/O/U, so nothing to misread).
//!   The server only stores a SHA-256 hash.
//! - **License certificates**: what the server hands a device after checking
//!   a key, an account's subscription or a purchase. Signed with Ed25519, so
//!   the desktop app can verify them offline with the public key built into
//!   it. They expire; while online the app renews them, which is also how a
//!   revoked key or an ended subscription takes effect.

use base64ct::{Base64UrlUnpadded, Encoding as _};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub use ed25519_dalek::{SigningKey, VerifyingKey};

/// Development only. Everyone can read it, so only debug builds of the backend
/// sign with it and only debug builds of the desktop app accept it.
pub const DEV_SIGNING_SEED: [u8; 32] = *b"sowling-DEV-ONLY-signing-key-32b";

const KEY_PREFIX: &str = "SOWL";
const KEY_GROUPS: usize = 4;
const GROUP_LEN: usize = 4;
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Where a device's right to run the desktop app comes from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseSource {
    /// Basic, Pro or CSC subscription of the signed-in account.
    Subscription,
    /// One-time purchase through Stripe.
    Purchase,
    /// Issued by the operator: tests, beta testers, loyal customers.
    Complimentary,
}

/// The signed content of a license certificate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LicenseClaims {
    /// Format version.
    pub v: u8,
    pub source: LicenseSource,
    /// The license row (purchase/complimentary); none for subscriptions.
    pub license_id: Option<Uuid>,
    /// The account, when the access comes from one.
    pub user_id: Option<Uuid>,
    /// Shown in the app ("licensed to ..."); may be absent for anonymous keys.
    pub email: Option<String>,
    /// Bound to one installation, so a certificate cannot be copied around.
    pub device_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LicenseError {
    Malformed,
    BadSignature,
    Expired,
    WrongDevice,
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LicenseError::Malformed => "license certificate is malformed",
            LicenseError::BadSignature => "license certificate signature is invalid",
            LicenseError::Expired => "license certificate has expired",
            LicenseError::WrongDevice => "license certificate belongs to another device",
        })
    }
}

// --- keys ---------------------------------------------------------------------------

/// A new random key, e.g. `SOWL-7K2M-QX9D-4HRT-B6WN`.
pub fn generate_key(random: [u8; 16]) -> String {
    // 16 characters x 5 bits = 80 bits of entropy: far beyond guessing for a
    // key that is only ever checked online, rate-limited, against a hash.
    let mut bits: u128 = u128::from_be_bytes(random);
    let mut chars = Vec::with_capacity(KEY_GROUPS * GROUP_LEN);
    for _ in 0..KEY_GROUPS * GROUP_LEN {
        chars.push(ALPHABET[(bits & 31) as usize] as char);
        bits >>= 5;
    }
    let groups: Vec<String> = chars.chunks(GROUP_LEN).map(|c| c.iter().collect()).collect();
    format!("{KEY_PREFIX}-{}", groups.join("-"))
}

/// Canonical form for hashing: upper case, separators dropped, look-alike
/// characters mapped (O→0, I/L→1), prefix optional. None if it cannot be a key.
pub fn normalize_key(input: &str) -> Option<String> {
    let compact: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    // The prefix goes first: it contains an L, which the mapping below
    // would otherwise turn into a 1.
    let body = compact.strip_prefix(KEY_PREFIX).unwrap_or(&compact);
    let s: String = body
        .chars()
        .map(|c| match c {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect();
    let valid = s.len() == KEY_GROUPS * GROUP_LEN && s.bytes().all(|b| ALPHABET.contains(&b));
    valid.then_some(s)
}

/// What the server stores instead of the key.
pub fn key_hash(normalized: &str) -> String {
    hex_lower(&Sha256::digest(normalized.as_bytes()))
}

/// Last group, to recognise a key in lists without storing it.
pub fn key_hint(normalized: &str) -> String {
    normalized[normalized.len() - GROUP_LEN..].to_string()
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// --- certificates -------------------------------------------------------------------

pub fn signing_key_from_b64(b64: &str) -> Option<SigningKey> {
    let bytes: [u8; 32] = Base64UrlUnpadded::decode_vec(b64.trim()).ok()?.try_into().ok()?;
    Some(SigningKey::from_bytes(&bytes))
}

pub fn verifying_key_from_b64(b64: &str) -> Option<VerifyingKey> {
    let bytes: [u8; 32] = Base64UrlUnpadded::decode_vec(b64.trim()).ok()?.try_into().ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

/// (private, public) as base64url, for the env file and the desktop build.
pub fn keypair_from_seed(seed: [u8; 32]) -> (String, String) {
    let sk = SigningKey::from_bytes(&seed);
    (
        Base64UrlUnpadded::encode_string(&sk.to_bytes()),
        Base64UrlUnpadded::encode_string(sk.verifying_key().as_bytes()),
    )
}

/// `base64url(json).base64url(signature)`.
pub fn sign_certificate(claims: &LicenseClaims, key: &SigningKey) -> String {
    let payload = serde_json::to_vec(claims).expect("claims serialize");
    let signature = key.sign(&payload);
    format!(
        "{}.{}",
        Base64UrlUnpadded::encode_string(&payload),
        Base64UrlUnpadded::encode_string(&signature.to_bytes())
    )
}

/// Signature only; expiry and device are checked by `verify_certificate`.
pub fn read_certificate(token: &str, key: &VerifyingKey) -> Result<LicenseClaims, LicenseError> {
    let (payload_b64, sig_b64) = token.trim().split_once('.').ok_or(LicenseError::Malformed)?;
    let payload = Base64UrlUnpadded::decode_vec(payload_b64).map_err(|_| LicenseError::Malformed)?;
    let sig_bytes: [u8; 64] = Base64UrlUnpadded::decode_vec(sig_b64)
        .map_err(|_| LicenseError::Malformed)?
        .try_into()
        .map_err(|_| LicenseError::Malformed)?;
    key.verify(&payload, &Signature::from_bytes(&sig_bytes)).map_err(|_| LicenseError::BadSignature)?;
    serde_json::from_slice(&payload).map_err(|_| LicenseError::Malformed)
}

/// Valid signature, this device, not expired.
pub fn verify_certificate(
    token: &str,
    key: &VerifyingKey,
    device_id: &str,
    now: DateTime<Utc>,
) -> Result<LicenseClaims, LicenseError> {
    let claims = read_certificate(token, key)?;
    if claims.device_id != device_id {
        return Err(LicenseError::WrongDevice);
    }
    if now >= claims.expires_at {
        return Err(LicenseError::Expired);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn claims(device: &str, expires_in: Duration) -> LicenseClaims {
        let now = Utc::now();
        LicenseClaims {
            v: 1,
            source: LicenseSource::Complimentary,
            license_id: Some(Uuid::new_v4()),
            user_id: None,
            email: Some("beta@example.com".into()),
            device_id: device.into(),
            issued_at: now,
            expires_at: now + expires_in,
        }
    }

    #[test]
    fn keys_round_trip_through_normalization() {
        let key = generate_key([0xA5; 16]);
        assert!(key.starts_with("SOWL-") && key.len() == 24, "{key}");
        let n = normalize_key(&key).unwrap();
        // Typed sloppily: lower case, spaces, no prefix, O for 0.
        let sloppy = key.to_lowercase().replace('-', " ").replace("sowl", "").replace('0', "o");
        assert_eq!(normalize_key(&sloppy).as_deref(), Some(n.as_str()));
        assert_eq!(key_hash(&n).len(), 64);
        assert_eq!(key_hint(&n).len(), 4);
        assert_ne!(generate_key([1; 16]), generate_key([2; 16]));
    }

    #[test]
    fn garbage_is_not_a_key() {
        for bad in ["", "SOWL-1234", "hello world", "SOWL-UUUU-UUUU-UUUU-UUUU", "SOWL-1234-5678-9ABC-DEFG-HJKM"] {
            assert_eq!(normalize_key(bad), None, "{bad}");
        }
    }

    #[test]
    fn certificates_verify_and_reject_tampering() {
        let (private, public) = keypair_from_seed([7; 32]);
        let sk = signing_key_from_b64(&private).unwrap();
        let vk = verifying_key_from_b64(&public).unwrap();
        let token = sign_certificate(&claims("dev-1", Duration::days(30)), &sk);
        let now = Utc::now();
        assert_eq!(verify_certificate(&token, &vk, "dev-1", now).unwrap().email.as_deref(), Some("beta@example.com"));
        assert_eq!(verify_certificate(&token, &vk, "dev-2", now), Err(LicenseError::WrongDevice));
        assert_eq!(verify_certificate(&token, &vk, "dev-1", now + Duration::days(31)), Err(LicenseError::Expired));

        // Changing a single claim (e.g. the device) breaks the signature.
        let (payload, sig) = token.split_once('.').unwrap();
        let mut forged: serde_json::Value = serde_json::from_slice(&Base64UrlUnpadded::decode_vec(payload).unwrap()).unwrap();
        forged["device_id"] = "dev-2".into();
        let forged = format!("{}.{sig}", Base64UrlUnpadded::encode_string(&serde_json::to_vec(&forged).unwrap()));
        assert_eq!(verify_certificate(&forged, &vk, "dev-2", now), Err(LicenseError::BadSignature));

        // Another key pair's signature is worthless.
        let (other, _) = keypair_from_seed([8; 32]);
        let foreign = sign_certificate(&claims("dev-1", Duration::days(30)), &signing_key_from_b64(&other).unwrap());
        assert_eq!(verify_certificate(&foreign, &vk, "dev-1", now), Err(LicenseError::BadSignature));
        assert_eq!(verify_certificate("nonsense", &vk, "dev-1", now), Err(LicenseError::Malformed));
    }
}
