//! Desktop app licensing.
//!
//! The desktop app runs when it holds a valid license certificate (signed
//! here, verified offline in the app; see shared/src/license.rs). A device
//! gets one in three ways:
//!   - a license key (purchase or operator-issued)         POST /licenses/activate
//!   - its signed-in account: a Basic/Pro/CSC subscription,
//!     a purchase, or an operator grant to the account's e-mail
//!                                                         POST /licenses/desktop-access
//!   - renewing the certificate it already has             POST /licenses/refresh
//! Renewal re-checks everything, so revoking a key or ending a subscription
//! takes effect at the next renewal (certificates are short-lived).

use axum::{extract::State, http::StatusCode, response::{IntoResponse, Response}, Json};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared::{
    license::{self, LicenseClaims, LicenseSource},
    SubscriptionPlan,
};
use uuid::Uuid;

use super::{api_error, require_consent};
use crate::{
    db::{repos, AppState},
    services::{auth::SessionUser, stripe::StripeClient},
};

/// Subscription certificates must be renewed often: the plan can end.
const SUBSCRIPTION_CERT_DAYS: i64 = 14;
/// Key/purchase certificates: generous offline period.
const LICENSE_CERT_DAYS: i64 = 60;
pub const DEFAULT_MAX_DEVICES: i32 = 3;

fn license_error(status: StatusCode, code: &str, message: impl ToString) -> Response {
    (status, Json(serde_json::json!({ "code": code, "message": message.to_string() }))).into_response()
}

fn is_paid(plan: SubscriptionPlan) -> bool {
    plan != SubscriptionPlan::Free
}

#[derive(sqlx::FromRow, Clone)]
pub struct LicenseRow {
    pub license_id: Uuid,
    pub kind: String,
    pub key_hint: Option<String>,
    pub email: Option<String>,
    pub user_id: Option<Uuid>,
    pub note: Option<String>,
    pub max_devices: i32,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub const LICENSE_COLUMNS: &str = "license_id, kind, key_hint, email, user_id, note, max_devices, created_at, revoked_at";

fn source_of(row: &LicenseRow) -> LicenseSource {
    if row.kind == "purchase" { LicenseSource::Purchase } else { LicenseSource::Complimentary }
}

#[derive(Deserialize)]
pub struct DeviceInfo {
    device_id: String,
    #[serde(default)]
    device_name: Option<String>,
}

fn check_device(d: &DeviceInfo) -> Result<(), Response> {
    let ok = (8..=128).contains(&d.device_id.len())
        && d.device_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && d.device_name.as_ref().is_none_or(|n| n.chars().count() <= 80);
    if ok { Ok(()) } else { Err(license_error(StatusCode::BAD_REQUEST, "invalid_device", "invalid device id or name")) }
}

#[derive(Serialize)]
pub struct CertificateResponse {
    certificate: String,
    source: LicenseSource,
    email: Option<String>,
    expires_at: DateTime<Utc>,
    max_devices: Option<i32>,
    devices_used: Option<i64>,
}

fn issue(
    state: &AppState,
    source: LicenseSource,
    license_id: Option<Uuid>,
    user_id: Option<Uuid>,
    email: Option<String>,
    device_id: &str,
) -> (String, DateTime<Utc>) {
    let now = Utc::now();
    let days = if source == LicenseSource::Subscription { SUBSCRIPTION_CERT_DAYS } else { LICENSE_CERT_DAYS };
    let claims = LicenseClaims {
        v: 1,
        source,
        license_id,
        user_id,
        email,
        device_id: device_id.to_string(),
        issued_at: now,
        expires_at: now + Duration::days(days),
    };
    (license::sign_certificate(&claims, &state.license_key), claims.expires_at)
}

/// Registers the device on the license (respecting max_devices) and returns
/// the number of devices now using it. Err(409) when the limit is reached.
async fn occupy_seat(state: &AppState, row: &LicenseRow, device: &DeviceInfo) -> Result<i64, Response> {
    let result: anyhow::Result<Option<i64>> = async {
        let mut tx = state.pg.begin().await?;
        // Serialise concurrent activations of the same license.
        sqlx::query("SELECT 1 FROM licenses WHERE license_id = $1 FOR UPDATE")
            .bind(row.license_id)
            .execute(&mut *tx)
            .await?;
        let others: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM license_activations WHERE license_id = $1 AND device_id <> $2",
        )
        .bind(row.license_id)
        .bind(&device.device_id)
        .fetch_one(&mut *tx)
        .await?;
        if others >= row.max_devices as i64 {
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO license_activations (license_id, device_id, device_name) VALUES ($1,$2,$3) \
             ON CONFLICT (license_id, device_id) DO UPDATE SET last_seen_at = NOW(), \
               device_name = COALESCE(EXCLUDED.device_name, license_activations.device_name)",
        )
        .bind(row.license_id)
        .bind(&device.device_id)
        .bind(&device.device_name)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(others + 1))
    }
    .await;
    match result {
        Ok(Some(used)) => Ok(used),
        Ok(None) => Err(license_error(
            StatusCode::CONFLICT,
            "device_limit",
            format!("this license is already used on {} devices", row.max_devices),
        )),
        Err(err) => Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, err)),
    }
}

async fn license_by_id(state: &AppState, id: Uuid) -> anyhow::Result<Option<LicenseRow>> {
    Ok(sqlx::query_as(&format!("SELECT {LICENSE_COLUMNS} FROM licenses WHERE license_id = $1"))
        .bind(id)
        .fetch_optional(&state.pg)
        .await?)
}

/// A valid (not revoked) license belonging to the account: bought by it, or
/// granted to its e-mail address. Purchases first.
async fn license_for_account(state: &AppState, user_id: Uuid, email: Option<&str>) -> anyhow::Result<Option<LicenseRow>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {LICENSE_COLUMNS} FROM licenses \
         WHERE revoked_at IS NULL AND (user_id = $1 OR ($2::text IS NOT NULL AND lower(email) = lower($2))) \
         ORDER BY (kind = 'purchase') DESC, created_at LIMIT 1"
    ))
    .bind(user_id)
    .bind(email)
    .fetch_optional(&state.pg)
    .await?)
}

// --- POST /licenses/activate --------------------------------------------------------

#[derive(Deserialize)]
pub struct ActivateBody {
    key: String,
    #[serde(flatten)]
    device: DeviceInfo,
}

pub async fn activate(State(state): State<AppState>, Json(body): Json<ActivateBody>) -> Response {
    if let Err(resp) = check_device(&body.device) {
        return resp;
    }
    let Some(normalized) = license::normalize_key(&body.key) else {
        return license_error(StatusCode::NOT_FOUND, "invalid_key", "unknown license key");
    };
    let row: Option<LicenseRow> = match sqlx::query_as(&format!("SELECT {LICENSE_COLUMNS} FROM licenses WHERE key_hash = $1"))
        .bind(license::key_hash(&normalized))
        .fetch_optional(&state.pg)
        .await
    {
        Ok(r) => r,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let Some(row) = row else {
        return license_error(StatusCode::NOT_FOUND, "invalid_key", "unknown license key");
    };
    if row.revoked_at.is_some() {
        return license_error(StatusCode::FORBIDDEN, "revoked", "this license key has been revoked");
    }
    let used = match occupy_seat(&state, &row, &body.device).await {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    let (certificate, expires_at) = issue(&state, source_of(&row), Some(row.license_id), row.user_id, row.email.clone(), &body.device.device_id);
    Json(CertificateResponse {
        certificate,
        source: source_of(&row),
        email: row.email,
        expires_at,
        max_devices: Some(row.max_devices),
        devices_used: Some(used),
    })
    .into_response()
}

// --- POST /licenses/desktop-access (signed in) --------------------------------------

pub async fn desktop_access(State(state): State<AppState>, session: SessionUser, Json(device): Json<DeviceInfo>) -> Response {
    if let Err(resp) = check_device(&device) {
        return resp;
    }
    account_certificate(&state, session.user_id, &device).await
}

async fn account_certificate(state: &AppState, user_id: Uuid, device: &DeviceInfo) -> Response {
    let user = match repos::get_user(state, user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return license_error(StatusCode::FORBIDDEN, "no_license", "account not found"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let email = repos::get_user_email(state, user_id).await.ok().flatten();

    if is_paid(user.plan) {
        let (certificate, expires_at) = issue(state, LicenseSource::Subscription, None, Some(user_id), email.clone(), &device.device_id);
        return Json(CertificateResponse { certificate, source: LicenseSource::Subscription, email, expires_at, max_devices: None, devices_used: None })
            .into_response();
    }

    let row = match license_for_account(state, user_id, email.as_deref()).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return license_error(
                StatusCode::PAYMENT_REQUIRED,
                "no_license",
                "this account has no desktop license (one-time purchase, or included in Basic/Pro/CSC)",
            )
        }
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    // An operator grant by e-mail becomes the account's on first use.
    if row.user_id.is_none() {
        let _ = sqlx::query("UPDATE licenses SET user_id = $1 WHERE license_id = $2 AND user_id IS NULL")
            .bind(user_id)
            .bind(row.license_id)
            .execute(&state.pg)
            .await;
    }
    let used = match occupy_seat(state, &row, device).await {
        Ok(n) => n,
        Err(resp) => return resp,
    };
    let (certificate, expires_at) = issue(state, source_of(&row), Some(row.license_id), Some(user_id), email.clone(), &device.device_id);
    Json(CertificateResponse {
        certificate,
        source: source_of(&row),
        email,
        expires_at,
        max_devices: Some(row.max_devices),
        devices_used: Some(used),
    })
    .into_response()
}

// --- POST /licenses/refresh ---------------------------------------------------------

#[derive(Deserialize)]
pub struct CertificateBody {
    certificate: String,
    #[serde(default)]
    device_name: Option<String>,
}

/// Renews a certificate this server signed, after checking again that the
/// right to use the app still holds. Works without a session, so a desktop
/// signed out for months keeps working as long as its license is valid.
pub async fn refresh(State(state): State<AppState>, Json(body): Json<CertificateBody>) -> Response {
    // Signature only: an expired certificate may be renewed.
    let claims = match license::read_certificate(&body.certificate, &state.license_key.verifying_key()) {
        Ok(c) => c,
        Err(_) => return license_error(StatusCode::UNAUTHORIZED, "invalid_certificate", "invalid license certificate"),
    };
    let device = DeviceInfo { device_id: claims.device_id.clone(), device_name: body.device_name };
    if let Err(resp) = check_device(&device) {
        return resp;
    }

    if let Some(license_id) = claims.license_id {
        let row = match license_by_id(&state, license_id).await {
            Ok(Some(r)) => r,
            Ok(None) => return license_error(StatusCode::FORBIDDEN, "revoked", "license no longer exists"),
            Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        };
        if row.revoked_at.is_some() {
            return license_error(StatusCode::FORBIDDEN, "revoked", "this license has been revoked");
        }
        // Removed from this device (or by the operator): no silent re-seat.
        let seated: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM license_activations WHERE license_id = $1 AND device_id = $2)",
        )
        .bind(license_id)
        .bind(&device.device_id)
        .fetch_one(&state.pg)
        .await
        .unwrap_or(false);
        if !seated {
            return license_error(StatusCode::FORBIDDEN, "deactivated", "this device is no longer activated");
        }
        let used = match occupy_seat(&state, &row, &device).await {
            Ok(n) => n,
            Err(resp) => return resp,
        };
        let (certificate, expires_at) = issue(&state, source_of(&row), Some(license_id), claims.user_id.or(row.user_id), row.email.clone().or(claims.email), &device.device_id);
        return Json(CertificateResponse {
            certificate,
            source: source_of(&row),
            email: row.email,
            expires_at,
            max_devices: Some(row.max_devices),
            devices_used: Some(used),
        })
        .into_response();
    }

    // Subscription certificate: the account decides again (plan, or a
    // purchase/grant made since).
    match claims.user_id {
        Some(user_id) => account_certificate(&state, user_id, &device).await,
        None => license_error(StatusCode::UNAUTHORIZED, "invalid_certificate", "invalid license certificate"),
    }
}

// --- POST /licenses/deactivate ------------------------------------------------------

/// Frees this device's seat ("remove license from this computer").
pub async fn deactivate(State(state): State<AppState>, Json(body): Json<CertificateBody>) -> Response {
    let claims = match license::read_certificate(&body.certificate, &state.license_key.verifying_key()) {
        Ok(c) => c,
        Err(_) => return license_error(StatusCode::UNAUTHORIZED, "invalid_certificate", "invalid license certificate"),
    };
    if let Some(license_id) = claims.license_id {
        if let Err(err) = sqlx::query("DELETE FROM license_activations WHERE license_id = $1 AND device_id = $2")
            .bind(license_id)
            .bind(&claims.device_id)
            .execute(&state.pg)
            .await
        {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

// --- GET /licenses/me (web app account card) ----------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct DeviceView {
    device_name: Option<String>,
    activated_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

pub async fn my_license(State(state): State<AppState>, session: SessionUser) -> Response {
    let user = match repos::get_user(&state, session.user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "account not found"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let email = repos::get_user_email(&state, session.user_id).await.ok().flatten();
    let row = match license_for_account(&state, session.user_id, email.as_deref()).await {
        Ok(r) => r,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let devices: Vec<DeviceView> = match &row {
        Some(r) => sqlx::query_as(
            "SELECT device_name, activated_at, last_seen_at FROM license_activations WHERE license_id = $1 ORDER BY activated_at",
        )
        .bind(r.license_id)
        .fetch_all(&state.pg)
        .await
        .unwrap_or_default(),
        None => vec![],
    };
    let access = if let Some(r) = &row {
        Some(source_of(r))
    } else if is_paid(user.plan) {
        Some(LicenseSource::Subscription)
    } else {
        None
    };
    Json(serde_json::json!({
        "access": access,
        "subscription_included": is_paid(user.plan),
        "max_devices": row.as_ref().map(|r| r.max_devices),
        "devices": devices,
        "purchase_available": purchase_available(),
        "download_url": std::env::var("DESKTOP_DOWNLOAD_URL").ok().filter(|u| !u.trim().is_empty()),
    }))
    .into_response()
}

fn purchase_available() -> bool {
    std::env::var("STRIPE_PRICE_DESKTOP").is_ok_and(|v| !v.trim().is_empty()) || cfg!(debug_assertions)
}

// --- POST /licenses/checkout (one-time purchase) ------------------------------------

pub async fn checkout(State(state): State<AppState>, session: SessionUser) -> Response {
    if let Err(resp) = require_consent(&state, session.user_id, "billing_stripe").await {
        return resp;
    }
    let email = repos::get_user_email(&state, session.user_id).await.ok().flatten().unwrap_or_default();
    match license_for_account(&state, session.user_id, Some(&email).filter(|e| !e.is_empty()).map(|e| e.as_str())).await {
        Ok(Some(_)) => return license_error(StatusCode::CONFLICT, "already_licensed", "this account already has a desktop license"),
        Ok(None) => {}
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }

    let price = std::env::var("STRIPE_PRICE_DESKTOP").ok().filter(|v| !v.trim().is_empty());
    match (StripeClient::from_env(), price) {
        (Some(stripe), Some(price_id)) => {
            if email.is_empty() {
                return api_error(StatusCode::BAD_REQUEST, "user email required for Stripe checkout");
            }
            let base = std::env::var("CHECKOUT_BASE_URL").unwrap_or_else(|_| "http://localhost:1420".to_string());
            let success = format!("{base}/?desktop_purchase=success");
            let cancel = format!("{base}/?desktop_purchase=cancel");
            match stripe.create_payment_session(&email, &price_id, session.user_id, "desktop_license", &success, &cancel).await {
                Ok(url) => Json(serde_json::json!({ "checkout_url": url })).into_response(),
                Err(err) => {
                    tracing::error!("stripe desktop checkout: {err}");
                    api_error(StatusCode::BAD_GATEWAY, "checkout service temporarily unavailable")
                }
            }
        }
        // Development without Stripe: grant directly so the flow can be tried.
        _ if cfg!(debug_assertions) => match create_license(&state, "purchase", Some(&email), Some(session.user_id), Some("dev purchase (no Stripe)"), DEFAULT_MAX_DEVICES, false, None).await {
            Ok(_) => Json(serde_json::json!({ "checkout_url": null, "granted": true })).into_response(),
            Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        _ => api_error(StatusCode::SERVICE_UNAVAILABLE, "desktop purchase is not configured"),
    }
}

/// Inserts a license; returns its id and, when `with_key`, the plain key
/// (shown once, never stored).
#[allow(clippy::too_many_arguments)]
pub async fn create_license(
    state: &AppState,
    kind: &str,
    email: Option<&str>,
    user_id: Option<Uuid>,
    note: Option<&str>,
    max_devices: i32,
    with_key: bool,
    stripe_session_id: Option<&str>,
) -> anyhow::Result<(Uuid, Option<String>)> {
    use rand_core::{OsRng, RngCore};
    let key = with_key.then(|| {
        let mut random = [0u8; 16];
        OsRng.fill_bytes(&mut random);
        license::generate_key(random)
    });
    let normalized = key.as_deref().and_then(license::normalize_key);
    let id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO licenses (license_id, kind, key_hash, key_hint, email, user_id, note, max_devices, stripe_session_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (stripe_session_id) DO NOTHING",
    )
    .bind(id)
    .bind(kind)
    .bind(normalized.as_deref().map(license::key_hash))
    .bind(normalized.as_deref().map(license::key_hint))
    .bind(email.map(|e| e.trim().to_lowercase()).filter(|e| !e.is_empty()))
    .bind(user_id)
    .bind(note)
    .bind(max_devices)
    .bind(stripe_session_id)
    .execute(&state.pg)
    .await?;
    if inserted.rows_affected() == 0 {
        anyhow::bail!("license for this checkout already exists");
    }
    Ok((id, key))
}

/// Stripe `checkout.session.completed` for a desktop purchase. Idempotent:
/// Stripe retries webhooks, the session id is unique.
pub async fn handle_purchase_completed(state: &AppState, session: &serde_json::Value) -> Response {
    if session["payment_status"].as_str() != Some("paid") {
        return Json(serde_json::json!({ "status": "ignored", "reason": "not paid" })).into_response();
    }
    let Some(user_id) = session["client_reference_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) else {
        tracing::error!("desktop purchase without client_reference_id: {}", session["id"]);
        return Json(serde_json::json!({ "status": "ignored", "reason": "no user" })).into_response();
    };
    let email = session["customer_details"]["email"].as_str().or(session["customer_email"].as_str());
    match create_license(state, "purchase", email, Some(user_id), Some("Stripe one-time purchase"), DEFAULT_MAX_DEVICES, false, session["id"].as_str()).await {
        Ok((id, _)) => {
            tracing::info!(%user_id, license = %id, "desktop license purchased");
            Json(serde_json::json!({ "status": "processed", "kind": "desktop_license" })).into_response()
        }
        Err(err) if err.to_string().contains("already exists") => {
            Json(serde_json::json!({ "status": "processed", "duplicate": true })).into_response()
        }
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

/// GDPR export: the account's licenses and devices.
pub async fn export_for_user(state: &AppState, user_id: Uuid, email: Option<&str>) -> anyhow::Result<serde_json::Value> {
    let rows: Vec<LicenseRow> = sqlx::query_as(&format!(
        "SELECT {LICENSE_COLUMNS} FROM licenses WHERE user_id = $1 OR ($2::text IS NOT NULL AND lower(email) = lower($2))"
    ))
    .bind(user_id)
    .bind(email)
    .fetch_all(&state.pg)
    .await?;
    let mut out = vec![];
    for r in rows {
        let devices: Vec<DeviceView> = sqlx::query_as(
            "SELECT device_name, activated_at, last_seen_at FROM license_activations WHERE license_id = $1",
        )
        .bind(r.license_id)
        .fetch_all(&state.pg)
        .await?;
        out.push(serde_json::json!({
            "kind": r.kind, "email": r.email, "key_hint": r.key_hint, "max_devices": r.max_devices,
            "created_at": r.created_at, "revoked_at": r.revoked_at, "devices": devices,
        }));
    }
    Ok(serde_json::Value::Array(out))
}
