//! Web Push reminders for the web app.
//!
//! The desktop app raises reminders itself. A browser tab can only do that
//! while it is open, so for the web app the server sends a push message when a
//! task with reminders enabled falls due. The browser's push service (Google,
//! Mozilla, Apple, Microsoft) delivers it to the service worker, which shows
//! the notification even when the app is closed.
//!
//! Configuration (push is disabled when the key is missing):
//!   VAPID_PRIVATE_KEY  base64url P-256 private key (`plants-backend gen-vapid-keys`)
//!   VAPID_SUBJECT      contact for push services, `mailto:` or `https:` URL
//!   PUSH_INTERVAL_SECONDS  how often due tasks are checked (default 300)
//!
//! Privacy: the payload is end-to-end encrypted for the browser (RFC 8291);
//! the push service only sees the encrypted blob, its size and timing.

use std::{collections::HashMap, sync::{Arc, OnceLock}, time::Duration};

use base64ct::{Base64UrlUnpadded, Encoding as _};
use chrono::{DateTime, FixedOffset, Utc};
use serde::Serialize;
use uuid::Uuid;
use web_push_native::{
    jwt_simple::algorithms::{ECDSAP256KeyPairLike as _, ES256KeyPair},
    p256::PublicKey,
    Auth, WebPushBuilder,
};

use crate::db::AppState;

/// Remind this long before the due time, like the in-app check.
const LEAD_MINUTES: i64 = 30;
/// Tasks that were already overdue this long when push got enabled (or the
/// server was down) are not pushed any more; the app lists them anyway.
const MAX_OVERDUE_HOURS: i64 = 24;
/// Upper bound per check, so one run never floods the push services.
const MAX_REMINDERS_PER_RUN: i64 = 200;
/// Undelivered messages are dropped by the push service after this. A
/// reminder that arrives a day late is noise.
const MESSAGE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
/// Subscriptions that keep failing (but were not reported gone) are removed.
const MAX_FAILURES: i32 = 10;
pub const MAX_SUBSCRIPTIONS_PER_USER: i64 = 10;

/// Languages the app ships; anything else falls back to German.
pub const LANGS: [&str; 10] = ["de", "en", "fr", "la", "es", "pt", "it", "nl", "pl", "tr"];

/// Hosts of the push services browsers use. The server POSTs to whatever
/// endpoint a client registers, so anything else is refused (SSRF guard).
const ALLOWED_PUSH_HOST_SUFFIXES: [&str; 4] = [
    "fcm.googleapis.com",          // Chrome, Edge (Chromium), Android, Opera, Samsung
    "push.services.mozilla.com",   // Firefox
    "push.apple.com",              // Safari macOS 13+, iOS/iPadOS 16.4+ (home screen)
    "notify.windows.com",          // legacy Edge
];

pub struct PushConfig {
    key_pair: ES256KeyPair,
    /// Uncompressed public key, base64url: the browser's `applicationServerKey`.
    pub public_key: String,
    subject: String,
    http: reqwest::Client,
}

impl PushConfig {
    pub fn from_env() -> anyhow::Result<Option<Arc<Self>>> {
        let Ok(private) = std::env::var("VAPID_PRIVATE_KEY") else {
            tracing::info!("VAPID_PRIVATE_KEY not set: web push reminders disabled");
            return Ok(None);
        };
        let bytes = Base64UrlUnpadded::decode_vec(private.trim())
            .map_err(|_| anyhow::anyhow!("VAPID_PRIVATE_KEY is not base64url"))?;
        let key_pair = ES256KeyPair::from_bytes(&bytes)
            .map_err(|e| anyhow::anyhow!("VAPID_PRIVATE_KEY is not a P-256 key: {e}"))?;
        let subject = std::env::var("VAPID_SUBJECT")
            .map_err(|_| anyhow::anyhow!("VAPID_SUBJECT must be set when VAPID_PRIVATE_KEY is (mailto: or https: contact)"))?;
        if !(subject.starts_with("mailto:") || subject.starts_with("https://")) {
            anyhow::bail!("VAPID_SUBJECT must start with mailto: or https://");
        }
        let public_key = Base64UrlUnpadded::encode_string(&key_pair.key_pair().public_key().to_bytes_uncompressed());
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        tracing::info!("web push reminders enabled");
        Ok(Some(Arc::new(Self { key_pair, public_key, subject, http })))
    }
}

/// `plants-backend gen-vapid-keys`: prints a fresh key pair for the env file.
pub fn generate_vapid_keys() -> (String, String) {
    let kp = ES256KeyPair::generate();
    let private = Base64UrlUnpadded::encode_string(&kp.to_bytes());
    let public = Base64UrlUnpadded::encode_string(&kp.key_pair().public_key().to_bytes_uncompressed());
    (private, public)
}

/// An https URL on a known push service, without credentials or odd ports.
pub fn endpoint_allowed(endpoint: &str) -> bool {
    if endpoint.len() > 2048 {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(endpoint) else { return false };
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else { return false };
    let host = host.to_ascii_lowercase();
    ALLOWED_PUSH_HOST_SUFFIXES
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
}

/// Validates the browser's subscription keys (they are used for encryption).
pub fn parse_keys(p256dh: &str, auth: &str) -> Result<(PublicKey, Auth), &'static str> {
    let p = Base64UrlUnpadded::decode_vec(p256dh.trim_end_matches('=')).map_err(|_| "keys.p256dh is not base64url")?;
    let public = PublicKey::from_sec1_bytes(&p).map_err(|_| "keys.p256dh is not a P-256 public key")?;
    let a = Base64UrlUnpadded::decode_vec(auth.trim_end_matches('=')).map_err(|_| "keys.auth is not base64url")?;
    if a.len() != 16 {
        return Err("keys.auth must be 16 bytes");
    }
    Ok((public, Auth::clone_from_slice(&a)))
}

pub fn normalize_lang(lang: Option<&str>) -> &'static str {
    let lang = lang.unwrap_or("de").get(..2).unwrap_or("de").to_ascii_lowercase();
    LANGS.iter().copied().find(|l| *l == lang).unwrap_or("de")
}

// --- texts -------------------------------------------------------------------
//
// The notification wording comes from the app's own locale files, so the push
// and the in-app reminder always read the same. Embedded at compile time.

const LOCALE_SOURCES: [(&str, &str); 10] = [
    ("de", include_str!("../../../desktop-ui/src/locales/de.json")),
    ("en", include_str!("../../../desktop-ui/src/locales/en.json")),
    ("fr", include_str!("../../../desktop-ui/src/locales/fr.json")),
    ("la", include_str!("../../../desktop-ui/src/locales/la.json")),
    ("es", include_str!("../../../desktop-ui/src/locales/es.json")),
    ("pt", include_str!("../../../desktop-ui/src/locales/pt.json")),
    ("it", include_str!("../../../desktop-ui/src/locales/it.json")),
    ("nl", include_str!("../../../desktop-ui/src/locales/nl.json")),
    ("pl", include_str!("../../../desktop-ui/src/locales/pl.json")),
    ("tr", include_str!("../../../desktop-ui/src/locales/tr.json")),
];

const TEXT_KEYS: [&str; 4] = ["reminder.notificationTitle", "reminder.body.due", "reminder.body.overdue", "push.testBody"];

fn texts() -> &'static HashMap<&'static str, HashMap<&'static str, String>> {
    static TEXTS: OnceLock<HashMap<&'static str, HashMap<&'static str, String>>> = OnceLock::new();
    TEXTS.get_or_init(|| {
        LOCALE_SOURCES
            .iter()
            .map(|(lang, src)| {
                let all: HashMap<String, String> = serde_json::from_str(src).unwrap_or_default();
                let picked = TEXT_KEYS
                    .iter()
                    .filter_map(|k| all.get(*k).map(|v| (*k, v.clone())))
                    .collect();
                (*lang, picked)
            })
            .collect()
    })
}

fn text(lang: &str, key: &str) -> String {
    let all = texts();
    all.get(lang)
        .and_then(|m| m.get(key))
        .or_else(|| all.get("de").and_then(|m| m.get(key)))
        .cloned()
        .unwrap_or_default()
}

fn fill(template: &str, args: &[&str]) -> String {
    args.iter()
        .enumerate()
        .fold(template.to_string(), |acc, (i, a)| acc.replace(&format!("{{{i}}}"), a))
}

/// The due time in the device's local time, in that language's usual order.
fn format_due(due_at: DateTime<Utc>, utc_offset_minutes: i32, lang: &str) -> String {
    let offset = FixedOffset::east_opt(utc_offset_minutes * 60).unwrap_or(FixedOffset::east_opt(0).unwrap());
    let local = due_at.with_timezone(&offset);
    let pattern = match lang {
        "en" => "%d %b %Y, %H:%M",
        "nl" => "%d-%m-%Y %H:%M",
        "fr" | "es" | "pt" | "it" | "la" => "%d/%m/%Y %H:%M",
        _ => "%d.%m.%Y %H:%M",
    };
    local.format(pattern).to_string()
}

#[derive(Serialize)]
pub struct Payload {
    title: String,
    body: String,
    /// Same tag replaces an older notification instead of stacking.
    tag: String,
    /// Opened when the notification is tapped.
    url: String,
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars - 1).collect::<String>())
    }
}

pub fn reminder_payload(task_id: Uuid, task_title: &str, due_at: DateTime<Utc>, now: DateTime<Utc>, lang: &str, utc_offset_minutes: i32) -> Payload {
    let due_label = format_due(due_at, utc_offset_minutes, lang);
    let title_arg = truncate(task_title, 120);
    let key = if due_at < now { "reminder.body.overdue" } else { "reminder.body.due" };
    Payload {
        title: text(lang, "reminder.notificationTitle"),
        body: fill(&text(lang, key), &[&title_arg, &due_label]),
        tag: format!("task-{task_id}"),
        url: "/".to_string(),
    }
}

pub fn test_payload(lang: &str) -> Payload {
    Payload {
        title: text(lang, "reminder.notificationTitle"),
        body: text(lang, "push.testBody"),
        tag: "push-test".to_string(),
        url: "/".to_string(),
    }
}

// --- sending -----------------------------------------------------------------

#[derive(sqlx::FromRow)]
pub struct SubscriptionRow {
    pub subscription_id: Uuid,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub lang: String,
    pub utc_offset_minutes: i32,
}

pub enum Outcome {
    Delivered,
    /// 404/410: the browser dropped the subscription; delete it.
    Gone,
    Failed(String),
}

/// The encrypted, VAPID-signed POST for one subscription.
fn build_request(config: &PushConfig, sub: &SubscriptionRow, payload: &Payload) -> anyhow::Result<reqwest::Request> {
    let (public, auth) = parse_keys(&sub.p256dh, &sub.auth).map_err(anyhow::Error::msg)?;
    let body = serde_json::to_vec(payload)?;
    let mut request = WebPushBuilder::new(sub.endpoint.parse()?, public, auth)
        .with_valid_duration(MESSAGE_TTL)
        .with_vapid(&config.key_pair, &config.subject)
        .build(body)?;
    request.headers_mut().insert("Urgency", reqwest::header::HeaderValue::from_static("normal"));
    Ok(reqwest::Request::try_from(request)?)
}

pub async fn send(config: &PushConfig, sub: &SubscriptionRow, payload: &Payload) -> Outcome {
    let request = match build_request(config, sub, payload) {
        Ok(r) => r,
        Err(err) => return Outcome::Failed(format!("build: {err}")),
    };
    match config.http.execute(request).await {
        Ok(resp) if resp.status().is_success() => Outcome::Delivered,
        Ok(resp) if matches!(resp.status().as_u16(), 404 | 410) => Outcome::Gone,
        Ok(resp) => Outcome::Failed(format!("push service answered {}", resp.status())),
        Err(err) => Outcome::Failed(format!("request: {err}")),
    }
}

/// Records the result on the subscription row. Returns true if it was removed.
pub async fn record_outcome(state: &AppState, sub: &SubscriptionRow, outcome: &Outcome) -> bool {
    let result = match outcome {
        Outcome::Delivered => sqlx::query(
            "UPDATE push_subscriptions SET last_success_at = NOW(), failure_count = 0 WHERE subscription_id = $1",
        )
        .bind(sub.subscription_id)
        .execute(&state.pg)
        .await
        .map(|_| false),
        Outcome::Gone => sqlx::query("DELETE FROM push_subscriptions WHERE subscription_id = $1")
            .bind(sub.subscription_id)
            .execute(&state.pg)
            .await
            .map(|_| true),
        Outcome::Failed(reason) => {
            tracing::warn!(subscription = %sub.subscription_id, "push failed: {reason}");
            sqlx::query(
                "WITH bumped AS ( \
                   UPDATE push_subscriptions SET failure_count = failure_count + 1 \
                   WHERE subscription_id = $1 RETURNING subscription_id, failure_count) \
                 DELETE FROM push_subscriptions WHERE subscription_id IN \
                   (SELECT subscription_id FROM bumped WHERE failure_count >= $2)",
            )
            .bind(sub.subscription_id)
            .bind(MAX_FAILURES)
            .execute(&state.pg)
            .await
            .map(|r| r.rows_affected() > 0)
        }
    };
    result.unwrap_or_else(|err| {
        tracing::warn!("could not record push outcome: {err}");
        false
    })
}

#[derive(sqlx::FromRow)]
struct DueTask {
    task_id: Uuid,
    title: String,
    due_at: DateTime<Utc>,
    user_id: Uuid,
}

#[derive(Default, Debug)]
pub struct RunReport {
    pub reminders: usize,
    pub delivered: usize,
    pub removed: usize,
    pub failed: usize,
}

/// One pass: claim due reminders, then push each to all of the owner's devices.
pub async fn run_due_reminders(state: &AppState, config: &PushConfig) -> anyhow::Result<RunReport> {
    // Claiming (INSERT … RETURNING) before sending makes this at-most-once and
    // safe if two backend instances run the loop at the same time.
    let due: Vec<DueTask> = sqlx::query_as(
        "WITH candidates AS ( \
           SELECT t.task_id, t.title, t.due_at, p.user_id \
           FROM tasks t JOIN plants p ON p.plant_id = t.plant_id \
           WHERE t.status = 'open' AND t.notification_enabled \
             AND t.due_at <= NOW() + make_interval(mins => $1) \
             AND t.due_at >= NOW() - make_interval(hours => $2) \
             AND EXISTS (SELECT 1 FROM push_subscriptions s WHERE s.user_id = p.user_id) \
             AND NOT EXISTS (SELECT 1 FROM push_reminders_sent r WHERE r.task_id = t.task_id AND r.due_at = t.due_at) \
           ORDER BY t.due_at \
           LIMIT $3), \
         claimed AS ( \
           INSERT INTO push_reminders_sent (task_id, due_at) \
           SELECT task_id, due_at FROM candidates \
           ON CONFLICT DO NOTHING RETURNING task_id, due_at) \
         SELECT c.task_id, c.title, c.due_at, c.user_id \
         FROM candidates c JOIN claimed USING (task_id, due_at)",
    )
    .bind(LEAD_MINUTES as i32)
    .bind(MAX_OVERDUE_HOURS as i32)
    .bind(MAX_REMINDERS_PER_RUN)
    .fetch_all(&state.pg)
    .await?;

    let mut report = RunReport { reminders: due.len(), ..Default::default() };
    let now = Utc::now();
    for task in due {
        let subs: Vec<SubscriptionRow> = sqlx::query_as(
            "SELECT subscription_id, endpoint, p256dh, auth, lang, utc_offset_minutes \
             FROM push_subscriptions WHERE user_id = $1",
        )
        .bind(task.user_id)
        .fetch_all(&state.pg)
        .await?;
        for sub in &subs {
            let payload = reminder_payload(task.task_id, &task.title, task.due_at, now, &sub.lang, sub.utc_offset_minutes);
            let outcome = send(config, sub, &payload).await;
            match outcome {
                Outcome::Delivered => report.delivered += 1,
                Outcome::Gone => {}
                Outcome::Failed(_) => report.failed += 1,
            }
            if record_outcome(state, sub, &outcome).await {
                report.removed += 1;
            }
        }
    }
    Ok(report)
}

pub fn interval_from_env() -> Duration {
    let secs = std::env::var("PUSH_INTERVAL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(300)
        .clamp(30, 3600);
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use web_push_native::p256::elliptic_curve::sec1::ToEncodedPoint as _;

    #[test]
    fn only_known_push_services_are_accepted() {
        for ok in [
            "https://fcm.googleapis.com/fcm/send/abc:def",
            "https://updates.push.services.mozilla.com/wpush/v2/gAAA",
            "https://web.push.apple.com/QGxj",
            "https://wns2-par02p.notify.windows.com/w/?token=x",
        ] {
            assert!(endpoint_allowed(ok), "{ok}");
        }
        for bad in [
            "http://fcm.googleapis.com/fcm/send/x",
            "https://localhost/push",
            "https://169.254.169.254/latest/meta-data",
            "https://evilfcm.googleapis.com.attacker.net/x",
            "https://fcm.googleapis.com:8443/x",
            "https://user:pw@fcm.googleapis.com/x",
            "https://notfcm.googleapis.comx/x",
            "not a url",
        ] {
            assert!(!endpoint_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn keys_are_validated() {
        let (private, public) = generate_vapid_keys();
        assert_eq!(Base64UrlUnpadded::decode_vec(&private).unwrap().len(), 32);
        // Any uncompressed P-256 point works as a browser key for this check.
        assert!(parse_keys(&public, "AAAAAAAAAAAAAAAAAAAAAA").is_ok());
        assert!(parse_keys(&public, "AAAA").is_err());
        assert!(parse_keys("AAAA", "AAAAAAAAAAAAAAAAAAAAAA").is_err());
    }

    #[test]
    fn texts_exist_in_every_language() {
        for lang in LANGS {
            for key in TEXT_KEYS {
                let v = texts().get(lang).and_then(|m| m.get(key));
                assert!(v.is_some_and(|v| !v.is_empty()), "{lang}: {key} missing");
            }
        }
    }

    #[test]
    fn reminder_text_uses_local_time_and_language() {
        let due = DateTime::parse_from_rfc3339("2026-10-16T08:00:00Z").unwrap().with_timezone(&Utc);
        let before = due - chrono::Duration::minutes(20);
        let p = reminder_payload(Uuid::nil(), "Gießen", due, before, "de", 120);
        assert!(p.body.contains("Gießen") && p.body.contains("16.10.2026 10:00"), "{}", p.body);
        assert!(!p.body.contains("{0}") && !p.body.contains("{1}"));
        let late = reminder_payload(Uuid::nil(), "Water", due, due + chrono::Duration::hours(1), "en", 0);
        assert!(late.body.contains("16 Oct 2026, 08:00"), "{}", late.body);
        assert_ne!(late.body, reminder_payload(Uuid::nil(), "Water", due, before, "en", 0).body);
    }

    #[test]
    fn payload_decrypts_with_the_browser_key() {
        use web_push_native::p256::SecretKey;
        let browser_secret = SecretKey::from_slice(&[7u8; 32]).unwrap();
        let browser_public = Base64UrlUnpadded::encode_string(
            browser_secret.public_key().to_encoded_point(false).as_bytes(),
        );
        let auth_bytes = [3u8; 16];
        let sub = SubscriptionRow {
            subscription_id: Uuid::nil(),
            endpoint: "https://fcm.googleapis.com/fcm/send/test".into(),
            p256dh: browser_public,
            auth: Base64UrlUnpadded::encode_string(&auth_bytes),
            lang: "de".into(),
            utc_offset_minutes: 120,
        };
        let config = PushConfig {
            key_pair: ES256KeyPair::generate(),
            public_key: String::new(),
            subject: "mailto:test@example.com".into(),
            http: reqwest::Client::new(),
        };
        let request = build_request(&config, &sub, &test_payload("de")).unwrap();
        let headers = request.headers();
        assert_eq!(headers["content-encoding"], "aes128gcm");
        assert_eq!(headers["ttl"], MESSAGE_TTL.as_secs().to_string().as_str());
        assert_eq!(headers["urgency"], "normal");
        assert!(headers["authorization"].to_str().unwrap().starts_with("vapid t="));
        let body = request.body().and_then(|b| b.as_bytes()).unwrap().to_vec();
        let plain = web_push_native::decrypt(body, &browser_secret, &Auth::clone_from_slice(&auth_bytes)).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(json["tag"], "push-test");
        assert!(json["body"].as_str().unwrap().starts_with("Push-Benachrichtigungen funktionieren"));
    }

    #[test]
    fn lang_falls_back_to_german() {
        assert_eq!(normalize_lang(Some("fr-CA")), "fr");
        assert_eq!(normalize_lang(Some("EN")), "en");
        assert_eq!(normalize_lang(Some("xx")), "de");
        assert_eq!(normalize_lang(None), "de");
    }
}
