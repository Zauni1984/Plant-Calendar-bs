use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::AuthMode;
use uuid::Uuid;

use crate::db::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub auth_mode: String,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct SessionUser {
    pub user_id: Uuid,
    pub auth_mode: AuthMode,
}

impl SessionUser {
    pub fn is_guest(&self) -> bool {
        matches!(self.auth_mode, AuthMode::Anonymous)
    }
}

impl FromRef<AppState> for String {
    fn from_ref(state: &AppState) -> Self {
        state.jwt_secret.clone()
    }
}

pub fn hash_password(input: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(input.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash error: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(input: &str, stored_hash: &str) -> bool {
    // Migration shim: if stored hash is 64 hex chars (old SHA256), verify with old method.
    // A 64-char hex string cannot be a valid argon2 hash (those start with `$argon2`),
    // so this branch is safe and unambiguous.
    if stored_hash.len() == 64 && stored_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        return hex::encode(hasher.finalize()) == stored_hash;
    }
    // New argon2 verification
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(input.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Verifies `plain` against `stored_hash`, transparently upgrading legacy SHA-256
/// hashes to Argon2. Returns:
///   * `Ok(Some(new_argon2_hash))` — verification succeeded AND the stored hash was
///     legacy SHA-256. Caller should persist the new hash to the DB.
///   * `Ok(None)`                  — verification succeeded against an Argon2 hash; no upgrade needed.
///   * `Err(_)`                    — verification failed, or the new hash could not be generated.
pub fn verify_password_and_upgrade(plain: &str, stored_hash: &str) -> anyhow::Result<Option<String>> {
    // Legacy SHA-256 branch: 64 hex chars.
    if stored_hash.len() == 64 && stored_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut hasher = Sha256::new();
        hasher.update(plain.as_bytes());
        if hex::encode(hasher.finalize()) == stored_hash {
            let upgraded = hash_password(plain)?;
            return Ok(Some(upgraded));
        }
        return Err(anyhow::anyhow!("invalid credentials"));
    }
    // Argon2 branch.
    let parsed = PasswordHash::new(stored_hash)
        .map_err(|e| anyhow::anyhow!("invalid stored hash: {e}"))?;
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .map_err(|_| anyhow::anyhow!("invalid credentials"))?;
    Ok(None)
}

pub fn issue_token(user_id: Uuid, auth_mode: AuthMode, secret: &str) -> anyhow::Result<String> {
    let auth_mode = match auth_mode {
        AuthMode::Anonymous => "anonymous",
        AuthMode::Email => "email",
        AuthMode::Google => "google",
    };
    let claims = AuthClaims {
        sub: user_id.to_string(),
        auth_mode: auth_mode.to_string(),
        exp: (Utc::now() + Duration::minutes(60)).timestamp() as usize,
    };
    Ok(encode(&Header::new(jsonwebtoken::Algorithm::HS256), &claims, &EncodingKey::from_secret(secret.as_bytes()))?)
}

/// Generate a refresh token: returns (raw_token_hex, sha256_hash_hex).
/// The raw token is sent to the client; the hash is stored in the DB.
pub fn issue_refresh_token() -> (String, String) {
    let mut buf = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut buf);
    let raw = hex::encode(buf);
    let hash = {
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        hex::encode(hasher.finalize())
    };
    (raw, hash)
}

/// Verify a raw refresh token against its stored hash.
pub fn verify_refresh_token(raw_token: &str, stored_hash: &str) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(raw_token.as_bytes());
    hex::encode(hasher.finalize()) == stored_hash
}

pub fn decode_token(token: &str, secret: &str) -> anyhow::Result<SessionUser> {
    let data = decode::<AuthClaims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::new(jsonwebtoken::Algorithm::HS256))?;
    let user_id = Uuid::parse_str(&data.claims.sub)?;
    let auth_mode = match data.claims.auth_mode.as_str() {
        "email" => AuthMode::Email,
        "google" => AuthMode::Google,
        _ => AuthMode::Anonymous,
    };
    Ok(SessionUser { user_id, auth_mode })
}

impl<S> FromRequestParts<S> for SessionUser
where
    S: Send + Sync,
    String: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let secret = String::from_ref(state);
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "missing authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or((StatusCode::UNAUTHORIZED, "invalid authorization scheme".to_string()))?;

        decode_token(token, &secret).map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token".to_string()))
    }
}
