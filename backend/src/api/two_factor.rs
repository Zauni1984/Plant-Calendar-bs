//! Optional two-factor sign-in: after the password, a 6-digit code from an
//! authenticator app (Google Authenticator, Microsoft Authenticator, Aegis …).
//!
//!   GET  /me/2fa            { enabled, enabled_at, recovery_codes_left }
//!   POST /me/2fa/setup      new secret + QR code (not active yet)
//!   POST /me/2fa/enable     { code }  confirms the setup, returns recovery codes once
//!   POST /me/2fa/disable    { password, code }  code or recovery code
//!
//! Offered right after registration (can be skipped) and in the settings.
//! With 2FA on, POST /auth/login answers 401 "totp_required" until the request
//! carries `totp_code`. A recovery code works once instead of an app code, for
//! a lost phone. Ten wrong codes in a row lock the second step for 15 minutes.

use axum::{extract::State, http::StatusCode, response::{IntoResponse, Response}, Json};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::{api_error, vouchers::{code_hash, generate_code, normalize_code}};
use crate::{
    db::{repos, AppState},
    services::{auth::SessionUser, totp},
};

const RECOVERY_CODES: usize = 10;
const MAX_FAILURES: i32 = 10;
const LOCK_MINUTES: i64 = 15;

/// Recovery codes like `7K2M-QX9D-4HRT` (60 bits). Stored only as hashes.
fn new_recovery_codes() -> (Vec<String>, Vec<String>) {
    use rand_core::{OsRng, RngCore};
    (0..RECOVERY_CODES)
        .map(|_| {
            let mut random = [0u8; 16];
            OsRng.fill_bytes(&mut random);
            let code: String = generate_code(random).chars().take(14).collect();
            let hash = code_hash(&normalize_code(&code).expect("generated codes are valid"));
            (code, hash)
        })
        .unzip()
}

#[derive(sqlx::FromRow)]
struct TotpRow {
    secret: String,
    enabled_at: Option<DateTime<Utc>>,
    last_step: i64,
    recovery_hashes: Vec<String>,
    locked_until: Option<DateTime<Utc>>,
}

async fn row(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<TotpRow>> {
    Ok(sqlx::query_as("SELECT secret, enabled_at, last_step, recovery_hashes, locked_until FROM user_totp WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.pg)
        .await?)
}

pub async fn is_enabled(state: &AppState, user_id: Uuid) -> anyhow::Result<bool> {
    Ok(row(state, user_id).await?.is_some_and(|r| r.enabled_at.is_some()))
}

/// Check an app code or recovery code for an account with 2FA on and spend
/// it. Wrong codes count towards the lock.
async fn check_code(state: &AppState, user_id: Uuid, r: &TotpRow, code: &str) -> anyhow::Result<Result<(), &'static str>> {
    if r.locked_until.is_some_and(|u| u > Utc::now()) {
        return Ok(Err("totp_locked"));
    }
    let spent = if let Some(step) = totp::verify(&r.secret, code, Utc::now().timestamp(), r.last_step) {
        // A parallel request with the same code loses here.
        sqlx::query("UPDATE user_totp SET last_step = $2, failed_attempts = 0, locked_until = NULL WHERE user_id = $1 AND last_step < $2")
            .bind(user_id)
            .bind(step)
            .execute(&state.pg)
            .await?
            .rows_affected()
            == 1
    } else if let Some(hash) = normalize_code(code).map(|n| code_hash(&n)).filter(|h| r.recovery_hashes.contains(h)) {
        sqlx::query("UPDATE user_totp SET recovery_hashes = array_remove(recovery_hashes, $2), failed_attempts = 0, locked_until = NULL WHERE user_id = $1 AND $2 = ANY(recovery_hashes)")
            .bind(user_id)
            .bind(&hash)
            .execute(&state.pg)
            .await?
            .rows_affected()
            == 1
    } else {
        false
    };
    if spent {
        return Ok(Ok(()));
    }
    sqlx::query(
        "UPDATE user_totp SET failed_attempts = CASE WHEN failed_attempts + 1 >= $2 THEN 0 ELSE failed_attempts + 1 END, \
            locked_until = CASE WHEN failed_attempts + 1 >= $2 THEN NOW() + make_interval(mins => $3) ELSE locked_until END \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(MAX_FAILURES)
    .bind(LOCK_MINUTES as i32)
    .execute(&state.pg)
    .await?;
    Ok(Err("totp_invalid"))
}

/// The second step of the e-mail sign-in. Ok when 2FA is off or the code is
/// right; otherwise "totp_required", "totp_invalid" or "totp_locked".
pub async fn check_login(state: &AppState, user_id: Uuid, code: Option<&str>) -> anyhow::Result<Result<(), &'static str>> {
    let Some(r) = row(state, user_id).await?.filter(|r| r.enabled_at.is_some()) else { return Ok(Ok(())) };
    match code.map(str::trim).filter(|c| !c.is_empty()) {
        None => Ok(Err("totp_required")),
        Some(code) => check_code(state, user_id, &r, code).await,
    }
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(serde_json::json!({ "code": code, "message": code }))).into_response()
}

pub fn login_error(code: &str) -> Response {
    error(if code == "totp_locked" { StatusCode::TOO_MANY_REQUESTS } else { StatusCode::UNAUTHORIZED }, code)
}

pub async fn status(State(state): State<AppState>, session: SessionUser) -> Response {
    match row(&state, session.user_id).await {
        Ok(r) => {
            let r = r.filter(|r| r.enabled_at.is_some());
            Json(serde_json::json!({
                "enabled": r.is_some(),
                "enabled_at": r.as_ref().and_then(|r| r.enabled_at),
                "recovery_codes_left": r.as_ref().map(|r| r.recovery_hashes.len()).unwrap_or(0),
            }))
            .into_response()
        }
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

/// QR code as SVG, black on white with the quiet zone (also for voucher flyers).
pub(crate) fn qr_svg(text: &str, size: u32) -> String {
    use qrcode::{render::svg, EcLevel, QrCode};
    QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map(|qr| qr.render::<svg::Color>().min_dimensions(size, size).quiet_zone(true).dark_color(svg::Color("#000000")).light_color(svg::Color("#ffffff")).build())
        .unwrap_or_default()
}

/// Start (or restart) the setup: a new secret that becomes active only once
/// a code from it is confirmed.
pub async fn setup(State(state): State<AppState>, session: SessionUser) -> Response {
    let email = match repos::get_user_email(&state, session.user_id).await {
        Ok(Some(e)) => e,
        Ok(None) => return error(StatusCode::BAD_REQUEST, "email_account_required"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    match is_enabled(&state, session.user_id).await {
        Ok(true) => return error(StatusCode::CONFLICT, "already_enabled"),
        Ok(false) => {}
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
    let secret = totp::new_secret();
    let stored = sqlx::query(
        "INSERT INTO user_totp (user_id, secret) VALUES ($1, $2) \
         ON CONFLICT (user_id) DO UPDATE SET secret = EXCLUDED.secret, last_step = 0, recovery_hashes = '{}', failed_attempts = 0, locked_until = NULL, created_at = NOW() \
         WHERE user_totp.enabled_at IS NULL",
    )
    .bind(session.user_id)
    .bind(&secret)
    .execute(&state.pg)
    .await;
    if let Err(err) = stored {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }
    let uri = totp::provisioning_uri(&secret, &email, "Sowling");
    Json(serde_json::json!({ "secret": secret, "otpauth_uri": uri, "qr_svg": qr_svg(&uri, 200), "account": email })).into_response()
}

#[derive(Deserialize)]
pub struct CodePayload {
    code: String,
}

pub async fn enable(State(state): State<AppState>, session: SessionUser, Json(p): Json<CodePayload>) -> Response {
    let r = match row(&state, session.user_id).await {
        Ok(Some(r)) if r.enabled_at.is_none() => r,
        Ok(Some(_)) => return error(StatusCode::CONFLICT, "already_enabled"),
        Ok(None) => return error(StatusCode::BAD_REQUEST, "setup_required"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let Some(step) = totp::verify(&r.secret, &p.code, Utc::now().timestamp(), r.last_step) else {
        return error(StatusCode::BAD_REQUEST, "totp_invalid");
    };
    let (codes, hashes) = new_recovery_codes();
    let updated = sqlx::query(
        "UPDATE user_totp SET enabled_at = NOW(), last_step = $2, recovery_hashes = $3, failed_attempts = 0, locked_until = NULL \
         WHERE user_id = $1 AND enabled_at IS NULL",
    )
    .bind(session.user_id)
    .bind(step)
    .bind(&hashes)
    .execute(&state.pg)
    .await;
    match updated {
        Ok(r) if r.rows_affected() == 1 => Json(serde_json::json!({ "enabled": true, "recovery_codes": codes })).into_response(),
        Ok(_) => error(StatusCode::CONFLICT, "already_enabled"),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct DisablePayload {
    password: String,
    code: String,
}

/// Turning it off needs the password and a current code (or recovery code),
/// so a session left open somewhere is not enough.
pub async fn disable(State(state): State<AppState>, session: SessionUser, Json(p): Json<DisablePayload>) -> Response {
    let hash = match repos::get_password_hash_by_user_id(&state, session.user_id).await {
        Ok(Some(h)) => h,
        Ok(None) => return error(StatusCode::BAD_REQUEST, "email_account_required"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    if p.password.len() > 256 || !crate::services::auth::verify_password(&p.password, &hash) {
        return error(StatusCode::UNAUTHORIZED, "wrong_password");
    }
    let r = match row(&state, session.user_id).await {
        Ok(Some(r)) if r.enabled_at.is_some() => r,
        Ok(_) => return error(StatusCode::BAD_REQUEST, "not_enabled"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    match check_code(&state, session.user_id, &r, &p.code).await {
        Ok(Ok(())) => {}
        Ok(Err(code)) => return login_error(code),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
    match sqlx::query("DELETE FROM user_totp WHERE user_id = $1").bind(session.user_id).execute(&state.pg).await {
        Ok(_) => Json(serde_json::json!({ "enabled": false })).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

/// Admin: remove 2FA from an account whose owner lost phone and recovery
/// codes (after checking who is asking).
pub async fn reset(state: &AppState, user_id: Uuid) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM user_totp WHERE user_id = $1").bind(user_id).execute(&state.pg).await?.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_codes_are_unique_and_normalize() {
        let (codes, hashes) = new_recovery_codes();
        assert_eq!(codes.len(), RECOVERY_CODES);
        assert!(codes.iter().all(|c| c.len() == 14), "{codes:?}");
        let unique: std::collections::HashSet<_> = hashes.iter().collect();
        assert_eq!(unique.len(), RECOVERY_CODES);
        // Typed in lower case without dashes, it still matches.
        let typed = codes[0].to_lowercase().replace('-', "");
        assert_eq!(code_hash(&normalize_code(&typed).unwrap()), hashes[0]);
    }
}
