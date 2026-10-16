//! Web Push subscriptions (see services/push.rs for sending and scheduling).

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::api_error;
use crate::{
    db::AppState,
    services::{
        auth::SessionUser,
        push::{self, SubscriptionRow},
    },
};

const DISABLED: &str = "push notifications are not configured on this server";

/// The VAPID public key the browser needs to subscribe. 404 means push is off.
pub async fn public_key(State(state): State<AppState>) -> impl IntoResponse {
    match &state.push {
        Some(config) => Json(serde_json::json!({ "public_key": config.public_key })).into_response(),
        None => api_error(StatusCode::NOT_FOUND, DISABLED),
    }
}

#[derive(Deserialize)]
pub struct SubscriptionKeys {
    p256dh: String,
    auth: String,
}

/// `PushSubscription.toJSON()` plus the device's language and UTC offset.
#[derive(Deserialize)]
pub struct SubscribeBody {
    endpoint: String,
    keys: SubscriptionKeys,
    lang: Option<String>,
    /// Local time minus UTC in minutes (CEST = 120).
    utc_offset_minutes: Option<i32>,
}

pub async fn subscribe(
    State(state): State<AppState>,
    session: SessionUser,
    Json(body): Json<SubscribeBody>,
) -> impl IntoResponse {
    if state.push.is_none() {
        return api_error(StatusCode::NOT_FOUND, DISABLED);
    }
    if !push::endpoint_allowed(&body.endpoint) {
        return api_error(StatusCode::BAD_REQUEST, "endpoint is not a supported push service");
    }
    if let Err(msg) = push::parse_keys(&body.keys.p256dh, &body.keys.auth) {
        return api_error(StatusCode::BAD_REQUEST, msg);
    }
    let lang = push::normalize_lang(body.lang.as_deref());
    // Real offsets range from UTC-12 to UTC+14.
    let offset = body.utc_offset_minutes.unwrap_or(0).clamp(-12 * 60, 14 * 60);

    let result = async {
        let mut tx = state.pg.begin().await?;
        sqlx::query(
            "INSERT INTO push_subscriptions \
               (subscription_id, user_id, endpoint, p256dh, auth, lang, utc_offset_minutes) \
             VALUES ($1,$2,$3,$4,$5,$6,$7) \
             ON CONFLICT (endpoint) DO UPDATE SET \
               user_id = EXCLUDED.user_id, p256dh = EXCLUDED.p256dh, auth = EXCLUDED.auth, \
               lang = EXCLUDED.lang, utc_offset_minutes = EXCLUDED.utc_offset_minutes, \
               updated_at = NOW(), failure_count = 0",
        )
        .bind(Uuid::new_v4())
        .bind(session.user_id)
        .bind(&body.endpoint)
        .bind(&body.keys.p256dh)
        .bind(&body.keys.auth)
        .bind(lang)
        .bind(offset)
        .execute(&mut *tx)
        .await?;
        // Browsers re-subscribe with a new endpoint now and then; keep the
        // most recently used devices only.
        sqlx::query(
            "DELETE FROM push_subscriptions WHERE user_id = $1 AND subscription_id NOT IN ( \
               SELECT subscription_id FROM push_subscriptions WHERE user_id = $1 \
               ORDER BY updated_at DESC LIMIT $2)",
        )
        .bind(session.user_id)
        .bind(push::MAX_SUBSCRIPTIONS_PER_USER)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }
    .await;
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct UnsubscribeBody {
    endpoint: String,
}

/// Removes this browser's subscription (push turned off, or signing out).
pub async fn unsubscribe(
    State(state): State<AppState>,
    session: SessionUser,
    Json(body): Json<UnsubscribeBody>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM push_subscriptions WHERE user_id = $1 AND endpoint = $2")
        .bind(session.user_id)
        .bind(&body.endpoint)
        .execute(&state.pg)
        .await;
    match result {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct TestBody {
    /// Only this browser; without it, all of the user's devices.
    endpoint: Option<String>,
}

/// Sends a test notification so the user can see that push works.
pub async fn send_test(
    State(state): State<AppState>,
    session: SessionUser,
    Json(body): Json<TestBody>,
) -> impl IntoResponse {
    let Some(config) = state.push.clone() else {
        return api_error(StatusCode::NOT_FOUND, DISABLED);
    };
    let subs: Vec<SubscriptionRow> = match sqlx::query_as(
        "SELECT subscription_id, endpoint, p256dh, auth, lang, utc_offset_minutes \
         FROM push_subscriptions WHERE user_id = $1 AND ($2::text IS NULL OR endpoint = $2)",
    )
    .bind(session.user_id)
    .bind(body.endpoint.as_deref())
    .fetch_all(&state.pg)
    .await
    {
        Ok(rows) => rows,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    if subs.is_empty() {
        return api_error(StatusCode::NOT_FOUND, "no push subscription for this account");
    }
    let (mut delivered, mut failed) = (0, 0);
    for sub in &subs {
        let outcome = push::send(&config, sub, &push::test_payload(&sub.lang)).await;
        match outcome {
            push::Outcome::Delivered => delivered += 1,
            _ => failed += 1,
        }
        push::record_outcome(&state, sub, &outcome).await;
    }
    Json(serde_json::json!({ "delivered": delivered, "failed": failed })).into_response()
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ExportedSubscription {
    /// Only the push service's origin; the full endpoint is a capability URL.
    push_service: String,
    lang: String,
    utc_offset_minutes: i32,
    created_at: DateTime<Utc>,
    last_success_at: Option<DateTime<Utc>>,
}

pub async fn export_for_user(state: &AppState, user_id: Uuid) -> anyhow::Result<Vec<ExportedSubscription>> {
    Ok(sqlx::query_as(
        "SELECT COALESCE(substring(endpoint from '^https://[^/]+'), '') AS push_service, lang, utc_offset_minutes, \
                created_at, last_success_at \
         FROM push_subscriptions WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(&state.pg)
    .await?)
}
