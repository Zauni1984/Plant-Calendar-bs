//! Desktop license check.
//!
//! The app runs when it holds a license certificate signed by our server for
//! this installation (see shared/src/license.rs). The signature is checked
//! offline with the public key compiled into the app; while online the
//! certificate is renewed, which is how revoked keys and ended subscriptions
//! take effect. Files in the app data directory:
//!   device_id     random id of this installation
//!   license.cert  the current certificate

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use serde::Serialize;
use shared::license::{self, LicenseClaims, LicenseError, VerifyingKey};

use crate::local_db::StoredSession;

/// Release builds must be given the production public key at compile time:
///   PLANTS_LICENSE_PUBLIC_KEY=... npm run tauri build
#[cfg(not(debug_assertions))]
const PUBLIC_KEY: &str = env!("PLANTS_LICENSE_PUBLIC_KEY", "set PLANTS_LICENSE_PUBLIC_KEY (from `plants-backend gen-license-keys`) for release builds");

/// Server used for key activation when not signed in. Set PLANTS_API_URL for
/// release builds; the development backend otherwise.
pub const DEFAULT_API_URL: &str = match option_env!("PLANTS_API_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8080",
};

/// The web app, where the desktop license is bought (PLANTS_WEB_URL).
pub const WEB_URL: &str = match option_env!("PLANTS_WEB_URL") {
    Some(url) => url,
    None => "http://localhost:4173",
};

/// Renew once a day while online.
const RENEW_AFTER_HOURS: i64 = 24;

fn verifying_key() -> VerifyingKey {
    #[cfg(not(debug_assertions))]
    {
        license::verifying_key_from_b64(PUBLIC_KEY).expect("PLANTS_LICENSE_PUBLIC_KEY is not a valid key")
    }
    #[cfg(debug_assertions)]
    {
        option_env!("PLANTS_LICENSE_PUBLIC_KEY")
            .and_then(license::verifying_key_from_b64)
            .unwrap_or_else(|| license::SigningKey::from_bytes(&license::DEV_SIGNING_SEED).verifying_key())
    }
}

fn cert_path() -> PathBuf {
    crate::app_data_dir().join("license.cert")
}

pub fn device_id() -> String {
    let path = crate::app_data_dir().join("device_id");
    if let Ok(id) = std::fs::read_to_string(&path) {
        let id = id.trim().to_string();
        if id.len() >= 8 {
            return id;
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::create_dir_all(crate::app_data_dir());
    let _ = std::fs::write(&path, &id);
    id
}

fn device_name() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|n| n.chars().take(80).collect())
}

fn read_cert() -> Option<String> {
    std::fs::read_to_string(cert_path()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn store_cert(cert: &str) -> Result<(), String> {
    std::fs::create_dir_all(crate::app_data_dir()).map_err(|e| e.to_string())?;
    std::fs::write(cert_path(), cert).map_err(|e| e.to_string())
}

fn forget_cert() {
    let _ = std::fs::remove_file(cert_path());
}

#[derive(Debug, Serialize)]
pub struct LicenseStatus {
    pub licensed: bool,
    /// subscription | purchase | complimentary | dev
    pub source: Option<String>,
    pub email: Option<String>,
    pub expires_at: Option<String>,
    /// Why not licensed (or a warning): missing, expired, revoked, deactivated,
    /// no_license, invalid.
    pub reason: Option<String>,
}

fn status_from(claims: &LicenseClaims) -> LicenseStatus {
    LicenseStatus {
        licensed: true,
        source: serde_json::to_value(claims.source).ok().and_then(|v| v.as_str().map(String::from)),
        email: claims.email.clone(),
        expires_at: Some(claims.expires_at.to_rfc3339()),
        reason: None,
    }
}

fn unlicensed(reason: &str) -> LicenseStatus {
    LicenseStatus { licensed: false, source: None, email: None, expires_at: None, reason: Some(reason.into()) }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap_or_default()
}

/// Server answer to an activation/renewal. Err holds the server's error code
/// (invalid_key, device_limit, revoked, deactivated, no_license, ...) or
/// "network" when the server could not be reached.
async fn post_for_certificate(url: String, body: serde_json::Value, bearer: Option<&str>) -> Result<String, String> {
    let mut req = client().post(url).json(&body);
    if let Some(token) = bearer {
        req = req.bearer_auth(token);
    }
    let resp = req.send().await.map_err(|_| "network".to_string())?;
    let status = resp.status();
    let json: serde_json::Value = resp.json().await.unwrap_or_default();
    if status.is_success() {
        return json["certificate"].as_str().map(String::from).ok_or_else(|| "invalid".to_string());
    }
    if status == reqwest::StatusCode::UNAUTHORIZED && json["code"].is_null() {
        return Err("session_expired".into());
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("rate_limited".into());
    }
    if status.is_server_error() {
        return Err("network".into());
    }
    Err(json["code"].as_str().unwrap_or("invalid").to_string())
}

/// Checks the stored certificate; renews it when due and possible.
pub async fn status(api_url: &str) -> LicenseStatus {
    #[cfg(debug_assertions)]
    if std::env::var("PLANTS_LICENSE_DEV_BYPASS").as_deref() == Ok("1") {
        return LicenseStatus { licensed: true, source: Some("dev".into()), email: None, expires_at: None, reason: None };
    }

    let Some(cert) = read_cert() else { return unlicensed("missing") };
    let key = verifying_key();
    let device = device_id();
    let now = Utc::now();
    let current = license::verify_certificate(&cert, &key, &device, now);

    let due = match &current {
        Ok(claims) => now - claims.issued_at > chrono::Duration::hours(RENEW_AFTER_HOURS),
        // Expired but genuine: renewal may bring it back.
        Err(LicenseError::Expired) => true,
        Err(_) => {
            forget_cert();
            return unlicensed("invalid");
        }
    };
    if !due {
        return status_from(current.as_ref().unwrap());
    }

    let renewed = post_for_certificate(
        format!("{api_url}/licenses/refresh"),
        serde_json::json!({ "certificate": cert, "device_name": device_name() }),
        None,
    )
    .await;
    match renewed {
        Ok(new_cert) => match license::verify_certificate(&new_cert, &key, &device, Utc::now()) {
            Ok(claims) => {
                let _ = store_cert(&new_cert);
                status_from(&claims)
            }
            Err(_) => current.as_ref().map(status_from).unwrap_or_else(|_| unlicensed("invalid")),
        },
        // Offline or server trouble: the current certificate stays valid
        // until it expires.
        Err(code) if code == "network" || code == "rate_limited" => match current {
            Ok(claims) => status_from(&claims),
            Err(_) => unlicensed("expired"),
        },
        // The server said no: revoked, removed from this device, plan ended.
        Err(code) => {
            forget_cert();
            unlicensed(&code)
        }
    }
}

fn accept(cert: String) -> Result<LicenseStatus, String> {
    let claims = license::verify_certificate(&cert, &verifying_key(), &device_id(), Utc::now()).map_err(|_| "invalid".to_string())?;
    store_cert(&cert)?;
    Ok(status_from(&claims))
}

pub async fn activate_key(api_url: &str, key: &str) -> Result<LicenseStatus, String> {
    if license::normalize_key(key).is_none() {
        return Err("invalid_key".into());
    }
    let cert = post_for_certificate(
        format!("{api_url}/licenses/activate"),
        serde_json::json!({ "key": key, "device_id": device_id(), "device_name": device_name() }),
        None,
    )
    .await?;
    accept(cert)
}

/// Via the signed-in account: subscription, purchase or e-mail grant.
/// Err("session_expired") lets the caller refresh the login and retry.
pub async fn activate_account(session: &StoredSession) -> Result<LicenseStatus, String> {
    let cert = post_for_certificate(
        format!("{}/licenses/desktop-access", session.backend_url),
        serde_json::json!({ "device_id": device_id(), "device_name": device_name() }),
        Some(&session.access_token),
    )
    .await?;
    accept(cert)
}

/// Frees this computer's seat on the license and forgets the certificate.
pub async fn remove(api_url: &str) {
    if let Some(cert) = read_cert() {
        let _ = client()
            .post(format!("{api_url}/licenses/deactivate"))
            .json(&serde_json::json!({ "certificate": cert }))
            .send()
            .await;
    }
    forget_cert();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end against a local debug backend (development signing key):
    ///   plants-backend license create --devices 1     -> PLANTS_TEST_LICENSE_KEY
    ///   cargo test -p plants_calendar_desktop -- --ignored license
    #[test]
    #[ignore = "needs a running debug backend and PLANTS_TEST_LICENSE_KEY"]
    fn key_activation_against_local_backend() {
        let dir = std::env::temp_dir().join(format!("plants-license-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        crate::APP_DATA_DIR.set(dir.clone()).ok();
        let key = std::env::var("PLANTS_TEST_LICENSE_KEY").expect("PLANTS_TEST_LICENSE_KEY");
        let api = std::env::var("PLANTS_TEST_API").unwrap_or_else(|_| DEFAULT_API_URL.to_string());

        tauri::async_runtime::block_on(async {
            let st = status(&api).await;
            assert!(!st.licensed);
            assert_eq!(st.reason.as_deref(), Some("missing"));

            assert_eq!(activate_key(&api, "garbage").await.unwrap_err(), "invalid_key");
            assert_eq!(activate_key(&api, "PLNT-0000-0000-0000-0000").await.unwrap_err(), "invalid_key");

            let st = activate_key(&api, &key).await.expect("activation");
            assert!(st.licensed);
            assert_eq!(st.source.as_deref(), Some("complimentary"));

            // Fresh certificate: verified offline, no network needed.
            let st = status("http://127.0.0.1:9").await;
            assert!(st.licensed, "offline check failed: {:?}", st.reason);

            // A certificate copied from another installation is rejected.
            let cert = std::fs::read_to_string(cert_path()).unwrap();
            std::fs::write(dir.join("device_id"), "another-device-0001").unwrap();
            assert_eq!(status(&api).await.reason.as_deref(), Some("invalid"));
            std::fs::write(dir.join("device_id"), uuid::Uuid::new_v4().to_string()).unwrap();

            // The key allows one device; this "new" device is refused.
            assert_eq!(activate_key(&api, &key).await.unwrap_err(), "device_limit");

            // Back on the original device, removing frees the seat.
            let original = license::read_certificate(&cert, &verifying_key()).unwrap().device_id;
            std::fs::write(dir.join("device_id"), &original).unwrap();
            store_cert(&cert).unwrap();
            assert!(status(&api).await.licensed);
            remove(&api).await;
            assert!(!status(&api).await.licensed);
            std::fs::write(dir.join("device_id"), "yet-another-device-01").unwrap();
            assert!(activate_key(&api, &key).await.unwrap().licensed, "seat was not freed");
            remove(&api).await;
        });
        let _ = std::fs::remove_dir_all(dir);
    }
}
