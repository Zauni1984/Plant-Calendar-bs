use axum::{body::Bytes, extract::{Path, Query, State}, http::{HeaderMap, StatusCode}, response::{IntoResponse, Response}, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use shared::{
    AuthMode, LogEntry, LogType, PlantPhase, PlantStatus, SubscriptionPlan, SubscriptionStatusSummary, SyncCursor, SyncEnvelope,
    SyncPullResponse, SyncPushRequest, Task, TaskCategory, TaskStatus,
};
use uuid::Uuid;

use crate::{
    db::{repos, AppState},
    services::{auth::SessionUser, plants, stripe::StripeClient},
};

#[derive(Serialize)]
pub struct ApiMessage {
    message: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user_id: Uuid,
    pub auth_mode: AuthMode,
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

fn api_error(status: StatusCode, message: impl ToString) -> Response {
    (status, Json(ApiMessage { message: message.to_string() })).into_response()
}

/// Server-side consent enforcement (defence-in-depth). Returns:
/// - `Ok(())` when a non-revoked, granted consent of `consent_type` exists.
/// - `Err(403)` with a structured `consent_required` body if missing/revoked.
/// - `Err(503)` if the consent table cannot be queried (transient DB failure).
async fn require_consent(state: &AppState, user_id: Uuid, consent_type: &str) -> Result<(), Response> {
    let row: Result<Option<(bool, Option<chrono::DateTime<Utc>>)>, sqlx::Error> = sqlx::query_as(
        "SELECT granted, revoked_at FROM user_consents \
         WHERE user_id = $1 AND consent_type = $2 \
         ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(consent_type)
    .fetch_optional(&state.pg)
    .await;

    match row {
        Ok(Some((true, None))) => Ok(()),
        Ok(_) => Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "consent_required",
                "consent_type": consent_type,
            })),
        )
            .into_response()),
        Err(err) => {
            tracing::error!(?err, "consent lookup failed");
            Err(api_error(StatusCode::SERVICE_UNAVAILABLE, "consent service unavailable"))
        }
    }
}

/// Redact an email for logs: keep first 3 chars of the local-part and the TLD.
/// E.g. "alice.smith@example.com" -> "ali***@***com".
fn pseudonymize_email(email: &str) -> String {
    let (local, domain) = match email.split_once('@') {
        Some(parts) => parts,
        None => return "***@***".to_string(),
    };
    let prefix: String = local.chars().take(3).collect();
    let tld = domain.rsplit('.').next().unwrap_or("");
    format!("{prefix}***@***{tld}")
}

/// Maximum accepted repeat interval: one year in hours.
/// Anything larger is rejected rather than clamped, so a bogus client value
/// surfaces as a validation error instead of silently becoming a huge series.
pub const MAX_REPEAT_INTERVAL_HOURS: i32 = 8760;

fn next_recurring_due_after(current_due: chrono::DateTime<Utc>, repeat_interval_hours: Option<i32>, now: chrono::DateTime<Utc>) -> Option<chrono::DateTime<Utc>> {
    let hours = repeat_interval_hours?;
    if hours <= 0 || hours > MAX_REPEAT_INTERVAL_HOURS {
        return None;
    }
    let step = chrono::Duration::try_hours(hours as i64)?;

    // Jump straight to the first slot after `now` instead of stepping one
    // interval at a time — a due date far in the past used to spin for
    // millions of iterations and block a Tokio worker.
    let mut next_due = current_due.checked_add_signed(step)?;
    if next_due <= now {
        let gap_secs = (now - next_due).num_seconds();
        let step_secs = step.num_seconds();
        if step_secs <= 0 {
            return None;
        }
        // +1 so the result is strictly after `now`.
        let skips = gap_secs / step_secs + 1;
        let advance = chrono::Duration::try_seconds(skips.checked_mul(step_secs)?)?;
        next_due = next_due.checked_add_signed(advance)?;
    }
    Some(next_due)
}

pub async fn health() -> impl IntoResponse {
    Json(ApiMessage { message: "ok".into() })
}

pub async fn get_catalog() -> impl IntoResponse {
    Json(shared::plant_catalog().clone())
}

#[derive(Deserialize)]
pub struct RegisterPayload {
    pub email: String,
    pub password: String,
}

pub async fn auth_register(State(state): State<AppState>, Json(payload): Json<RegisterPayload>) -> impl IntoResponse {
    if payload.email.trim().is_empty() || !payload.email.contains('@') || payload.email.len() > 320 {
        return api_error(StatusCode::BAD_REQUEST, "invalid email");
    }
    if payload.password.len() < 8 {
        return api_error(StatusCode::BAD_REQUEST, "password must be at least 8 characters");
    }
    if payload.password.len() > 256 {
        return api_error(StatusCode::BAD_REQUEST, "password too long");
    }
    match repos::get_user_by_email(&state, &payload.email).await {
        Ok(Some(_)) => return api_error(StatusCode::CONFLICT, "email already registered"),
        Ok(None) => {},
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
    let user_id = Uuid::new_v4();
    let password_hash = match crate::services::auth::hash_password(&payload.password) {
        Ok(h) => h,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    match repos::create_user_with_credentials(&state, user_id, payload.email, password_hash, AuthMode::Email).await {
        Ok(user) => match crate::services::auth::issue_token(user.user_id, AuthMode::Email, &state.jwt_secret) {
            Ok(token) => {
                let (raw_refresh, refresh_hash) = crate::services::auth::issue_refresh_token();
                let expires_at = Utc::now() + chrono::Duration::days(90);
                if let Err(err) = repos::store_refresh_token(&state, &refresh_hash, user.user_id, expires_at).await {
                    return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
                }
                Json(AuthResponse { user_id: user.user_id, auth_mode: AuthMode::Email, access_token: token, refresh_token: Some(raw_refresh) }).into_response()
            },
            Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub email: String,
    pub password: String,
}

pub async fn auth_login(State(state): State<AppState>, Json(payload): Json<LoginPayload>) -> impl IntoResponse {
    if payload.email.trim().is_empty() || !payload.email.contains('@') || payload.email.len() > 320 {
        return api_error(StatusCode::BAD_REQUEST, "invalid email");
    }
    if payload.password.is_empty() || payload.password.len() > 256 {
        return api_error(StatusCode::BAD_REQUEST, "invalid credentials");
    }
    let user = match repos::get_user_by_email(&state, &payload.email).await {
        Ok(Some(user)) => user,
        Ok(None) => return api_error(StatusCode::UNAUTHORIZED, "invalid credentials"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let password_hash = match repos::get_password_hash_by_user_id(&state, user.user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => return api_error(StatusCode::UNAUTHORIZED, "invalid credentials"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    match crate::services::auth::verify_password_and_upgrade(&payload.password, &password_hash) {
        Ok(Some(upgraded_hash)) => {
            // Legacy SHA-256 hash verified; persist the new Argon2 hash.
            if let Err(err) = repos::change_user_password(&state, user.user_id, &upgraded_hash).await {
                tracing::warn!("failed to upgrade legacy password hash for user {}: {}", user.user_id, err);
            }
        },
        Ok(None) => {},
        Err(_) => return api_error(StatusCode::UNAUTHORIZED, "invalid credentials"),
    }
    match crate::services::auth::issue_token(user.user_id, AuthMode::Email, &state.jwt_secret) {
        Ok(token) => {
            let (raw_refresh, refresh_hash) = crate::services::auth::issue_refresh_token();
            let expires_at = Utc::now() + chrono::Duration::days(90);
            if let Err(err) = repos::store_refresh_token(&state, &refresh_hash, user.user_id, expires_at).await {
                return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
            }
            Json(AuthResponse { user_id: user.user_id, auth_mode: AuthMode::Email, access_token: token, refresh_token: Some(raw_refresh) }).into_response()
        },
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct GooglePayload {
    pub id_token: String,
}

// TODO: Implement full Google OAuth JWKS verification once Google Cloud Console is configured
pub async fn auth_google(State(_state): State<AppState>, Json(_payload): Json<GooglePayload>) -> impl IntoResponse {
    api_error(StatusCode::NOT_IMPLEMENTED, "Google OAuth not yet configured. Use email registration.")
}

#[derive(Deserialize)]
pub struct LogoutPayload {
    pub refresh_token: Option<String>,
}

pub async fn auth_logout(State(state): State<AppState>, session: SessionUser, Json(payload): Json<LogoutPayload>) -> impl IntoResponse {
    if let Some(raw_token) = payload.refresh_token {
        let hash = hex::encode(sha2::Sha256::digest(raw_token.as_bytes()));
        // Scope the revoke to the caller: revoking purely by hash would let a
        // logged-in user invalidate another account's session.
        match repos::revoke_refresh_token_for_user(&state, &hash, session.user_id).await {
            Ok(_) => {}
            Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct RefreshPayload {
    pub refresh_token: String,
}

pub async fn auth_refresh(State(state): State<AppState>, Json(payload): Json<RefreshPayload>) -> impl IntoResponse {
    // Hash the raw token to look it up
    let token_hash = hex::encode(sha2::Sha256::digest(payload.refresh_token.as_bytes()));

    let stored = match repos::find_refresh_token(&state, &token_hash).await {
        Ok(Some(row)) => row,
        Ok(None) => return api_error(StatusCode::UNAUTHORIZED, "invalid refresh token"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    // Check if revoked
    if stored.revoked_at.is_some() {
        return api_error(StatusCode::UNAUTHORIZED, "refresh token has been revoked");
    }

    // Check if expired
    if stored.expires_at < Utc::now() {
        return api_error(StatusCode::UNAUTHORIZED, "refresh token has expired");
    }

    // Revoke old refresh token (rotation).
    // This must succeed before a new pair is handed out — swallowing the error
    // would leave the presented token valid for its full 90-day lifetime.
    if let Err(err) = repos::revoke_refresh_token(&state, &token_hash).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }

    // Look up user to get auth_mode
    let user = match repos::get_user(&state, stored.user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return api_error(StatusCode::UNAUTHORIZED, "user not found"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    // Issue new access token
    let access_token = match crate::services::auth::issue_token(user.user_id, user.auth_mode.clone(), &state.jwt_secret) {
        Ok(t) => t,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    // Issue new refresh token
    let (new_raw_refresh, new_refresh_hash) = crate::services::auth::issue_refresh_token();
    let expires_at = Utc::now() + chrono::Duration::days(90);
    if let Err(err) = repos::store_refresh_token(&state, &new_refresh_hash, user.user_id, expires_at).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }

    Json(AuthResponse {
        user_id: user.user_id,
        auth_mode: user.auth_mode,
        access_token,
        refresh_token: Some(new_raw_refresh),
    }).into_response()
}

pub async fn get_me(State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::get_or_create_user(&state, session.user_id, session.auth_mode).await {
        Ok(user) => Json(user).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct AdultConfirmationPayload {
    pub adult_confirmed: bool,
}

pub async fn patch_adult_confirmation(
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<AdultConfirmationPayload>,
) -> impl IntoResponse {
    match repos::update_adult_confirmation(&state, session.user_id, payload.adult_confirmed).await {
        Ok(user) => Json(user).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn list_plants(State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::list_plants(&state, session.user_id).await {
        Ok(plants) => Json(plants).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct CreatePlantPayload {
    pub name: String,
    pub color_tag: String,
    pub phase: PlantPhase,
}

pub async fn create_plant(
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<CreatePlantPayload>,
) -> impl IntoResponse {
    if payload.name.trim().is_empty() || payload.name.len() > 120 {
        return api_error(StatusCode::BAD_REQUEST, "plant name must be 1-120 characters");
    }
    if payload.color_tag.trim().is_empty() || payload.color_tag.len() > 30 {
        return api_error(StatusCode::BAD_REQUEST, "invalid color tag");
    }

    let user = match repos::get_or_create_user(&state, session.user_id, session.auth_mode).await {
        Ok(user) => user,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    let existing_plants = match repos::list_plants(&state, user.user_id).await {
        Ok(plants) => plants,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    if let Err(err) = plants::validate_create_from_plants(&existing_plants, user.plan) {
        return api_error(StatusCode::FORBIDDEN, err);
    }

    let plant = plants::build_plant(user.user_id, payload.name, payload.color_tag, payload.phase);
    match repos::insert_plant(&state, &plant).await {
        Ok(_) => (StatusCode::CREATED, Json(plant)).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn get_plant(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::get_plant(&state, session.user_id, id).await {
        Ok(Some(plant)) => Json(plant).into_response(),
        Ok(None) => api_error(StatusCode::NOT_FOUND, "plant not found"),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct UpdatePlantPayload {
    pub name: Option<String>,
    pub color_tag: Option<String>,
    pub phase: Option<PlantPhase>,
    #[serde(default)]
    pub archived: Option<bool>,
    pub status: Option<PlantStatus>,
}

pub async fn update_plant(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<UpdatePlantPayload>,
) -> impl IntoResponse {
    let user = match repos::get_or_create_user(&state, session.user_id, session.auth_mode).await {
        Ok(user) => user,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let current = match repos::get_plant(&state, session.user_id, id).await {
        Ok(Some(plant)) => plant,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "plant not found"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let target_status = shared::resolve_requested_plant_status(current.status, payload.status, payload.archived, None);
    if shared::is_reactivation_transition(current.status, target_status) {
        let existing_plants = match repos::list_plants(&state, session.user_id).await {
            Ok(plants) => plants,
            Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        };
        if let Err(err) = plants::validate_reactivate_from_plants(&existing_plants, id, user.plan) {
            return api_error(StatusCode::FORBIDDEN, err);
        }
    }

    match repos::update_plant(&state, session.user_id, id, payload.name, payload.color_tag, payload.phase, Some(target_status)).await {
        Ok(plant) => Json(plant).into_response(),
        Err(err) if err.to_string().contains("not found") => api_error(StatusCode::NOT_FOUND, err),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn delete_plant(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::delete_plant(&state, session.user_id, id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn list_tasks_for_plant(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    if let Err(err) = repos::verify_plant_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    match repos::list_tasks_for_plant(&state, id).await {
        Ok(tasks) => Json(tasks).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct CreateTaskPayload {
    pub title: String,
    pub category: TaskCategory,
    pub due_at: chrono::DateTime<Utc>,
    pub repeat_interval_hours: Option<i32>,
    pub notification_enabled: bool,
}

pub async fn create_task_for_plant(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<CreateTaskPayload>,
) -> impl IntoResponse {
    if payload.title.trim().is_empty() || payload.title.len() > 120 {
        return api_error(StatusCode::BAD_REQUEST, "task title must be 1-120 characters");
    }
    if let Err(err) = repos::verify_plant_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    let now = Utc::now();
    let task = Task {
        task_id: Uuid::new_v4(),
        plant_id: id,
        title: payload.title,
        category: payload.category,
        due_at: payload.due_at,
        repeat_interval_hours: payload.repeat_interval_hours,
        repeat_until: None,
        notification_enabled: payload.notification_enabled,
        status: TaskStatus::Open,
        created_at: now,
        updated_at: now,
    };
    match repos::insert_task(&state, &task).await {
        Ok(_) => (StatusCode::CREATED, Json(task)).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct UpdateTaskPayload {
    pub title: Option<String>,
    pub status: Option<TaskStatus>,
    pub due_at: Option<chrono::DateTime<Utc>>,
    pub repeat_interval_hours: Option<i32>,
    pub notification_enabled: Option<bool>,
}

pub async fn update_task(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser, Json(payload): Json<UpdateTaskPayload>) -> impl IntoResponse {
    if let Err(err) = repos::verify_task_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    let existing = match repos::get_task(&state, id).await {
        Ok(Some(task)) => task,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "task not found"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    let requested_status = payload.status;
    let repeat_interval_hours = payload.repeat_interval_hours.or(existing.repeat_interval_hours);
    let mut due_at = payload.due_at;
    let mut final_status = requested_status.unwrap_or(existing.status);

    if requested_status == Some(TaskStatus::Done) && due_at.is_none() {
        if let Some(next_due) = next_recurring_due_after(existing.due_at, repeat_interval_hours, Utc::now()) {
            due_at = Some(next_due);
            final_status = TaskStatus::Open;
        }
    }

    match repos::update_task(&state, id, payload.title, Some(final_status), due_at, payload.repeat_interval_hours, payload.notification_enabled).await {
        Ok(task) => Json(task).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn delete_task(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::delete_task(&state, session.user_id, id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn list_logs_for_plant(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    if let Err(err) = repos::verify_plant_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    match repos::list_logs_for_plant(&state, id).await {
        Ok(logs) => Json(logs).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct CreateLogPayload {
    pub log_type: Option<LogType>,
    pub created_at: Option<chrono::DateTime<Utc>>,
    pub text: Option<String>,
    pub metrics: Option<shared::Metrics>,
    pub action: Option<serde_json::Value>,
    pub photo_refs: Option<Vec<String>>,
}

pub async fn create_log_for_plant(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<CreateLogPayload>,
) -> impl IntoResponse {
    if let Some(ref text) = payload.text {
        if text.len() > 5000 {
            return api_error(StatusCode::BAD_REQUEST, "log text too long (max 5000 characters)");
        }
    }
    if let Err(err) = repos::verify_plant_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    let log = LogEntry {
        log_id: Uuid::new_v4(),
        plant_id: id,
        log_type: payload.log_type.unwrap_or(LogType::Note),
        text: payload.text,
        metrics: payload.metrics,
        action: payload.action.and_then(|v| serde_json::from_value(v).ok()),
        photo_refs: payload.photo_refs,
        created_at: payload.created_at.unwrap_or_else(Utc::now),
    };
    match repos::insert_log(&state, &log).await {
        Ok(_) => (StatusCode::CREATED, Json(log)).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct UpdateLogPayload {
    pub log_type: Option<LogType>,
    pub text: Option<String>,
    pub metrics: Option<shared::Metrics>,
    pub action: Option<serde_json::Value>,
    pub photo_refs: Option<Vec<String>>,
}

pub async fn update_log(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser, Json(payload): Json<UpdateLogPayload>) -> impl IntoResponse {
    if let Err(err) = repos::verify_log_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    match repos::update_log(&state, id, payload.log_type, payload.text, payload.metrics, payload.action, payload.photo_refs).await {
        Ok(log) => Json(log).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn delete_log(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::delete_log(&state, session.user_id, id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

// ── Location endpoints ──

pub async fn list_locations(State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::list_locations(&state, session.user_id).await {
        Ok(locs) => Json(locs).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct CreateLocationPayload {
    pub name: String,
    pub location_type: Option<String>,
    pub description: Option<String>,
}

pub async fn create_location(State(state): State<AppState>, session: SessionUser, Json(payload): Json<CreateLocationPayload>) -> impl IntoResponse {
    if payload.name.trim().is_empty() || payload.name.len() > 120 {
        return api_error(StatusCode::BAD_REQUEST, "location name must be 1-120 characters");
    }
    match repos::create_location(&state, session.user_id, payload.name.trim().to_string(), payload.location_type.unwrap_or_else(|| "custom".into()), payload.description).await {
        Ok(loc) => (StatusCode::CREATED, Json(loc)).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct UpdateLocationPayload {
    pub name: Option<String>,
    pub location_type: Option<String>,
    pub description: Option<String>,
}

pub async fn update_location(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser, Json(payload): Json<UpdateLocationPayload>) -> impl IntoResponse {
    if let Some(ref name) = payload.name {
        if name.trim().is_empty() || name.len() > 120 {
            return api_error(StatusCode::BAD_REQUEST, "location name must be 1-120 characters");
        }
    }
    match repos::update_location(&state, session.user_id, id, payload.name, payload.location_type, payload.description).await {
        Ok(loc) => Json(loc).into_response(),
        Err(err) if err.to_string().contains("not found") => api_error(StatusCode::NOT_FOUND, err),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn delete_location(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::delete_location(&state, session.user_id, id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn get_subscription(State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::get_or_create_user(&state, session.user_id, session.auth_mode).await {
        Ok(user) => {
            let history = repos::list_subscription_history(&state, session.user_id, 10).await.unwrap_or_default();
            let summary = repos::subscription_status_summary(&state, session.user_id, user.plan)
                .await
                .unwrap_or_else(|_| SubscriptionStatusSummary::empty_for_plan(user.plan));
            Json(serde_json::json!({
                "plan": format!("{:?}", user.plan).to_lowercase(),
                "plant_limit": user.plan.plant_limit(),
                "history": history,
                "summary": summary,
            })).into_response()
        },
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn get_subscription_status(State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    let user = match repos::get_or_create_user(&state, session.user_id, session.auth_mode).await {
        Ok(user) => user,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    match repos::subscription_status_summary(&state, session.user_id, user.plan).await {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn get_subscription_history(State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::list_subscription_history(&state, session.user_id, 50).await {
        Ok(history) => Json(serde_json::json!({"history": history})).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct CheckoutPayload {
    pub plan: SubscriptionPlan,
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub status: String,
    pub plan: String,
    pub mode: String,
    pub message: String,
    pub checkout_url: Option<String>,
    pub plant_limit: usize,
    pub price_eur_month: f32,
    pub activated_immediately: bool,
}

#[derive(Deserialize)]
pub struct CheckoutReturnQuery {
    pub plan: Option<String>,
    pub user_id: Option<Uuid>,
    pub checkout_status: Option<String>,
    pub checkout_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CheckoutWebhookPayload {
    pub user_id: Uuid,
    pub plan: SubscriptionPlan,
    pub checkout_status: String,
    pub checkout_id: Option<String>,
    pub event_type: Option<String>,
    pub provider: Option<String>,
}

#[derive(Serialize)]
pub struct CheckoutWebhookResponse {
    pub status: String,
    pub applied: bool,
    pub plan: String,
    pub user_id: Uuid,
    pub checkout_status: String,
    pub event_type: String,
    pub provider: String,
    pub checkout_id: Option<String>,
    pub activated_immediately: bool,
    pub message: String,
}

fn checkout_price_eur_month(plan: SubscriptionPlan) -> f32 {
    match plan {
        SubscriptionPlan::Free => 0.0,
        SubscriptionPlan::Basic => 0.99,
        SubscriptionPlan::Pro => 5.99,
        SubscriptionPlan::Csc => 50.0,
    }
}

fn checkout_mode_for(plan: SubscriptionPlan) -> &'static str {
    match plan {
        SubscriptionPlan::Free => "direct_plan_change",
        SubscriptionPlan::Basic | SubscriptionPlan::Pro | SubscriptionPlan::Csc => "external_checkout_stub",
    }
}

fn build_checkout_url(base: Option<&str>, user_id: Uuid, plan: SubscriptionPlan) -> Option<String> {
    let base = base?.trim();
    if base.is_empty() {
        return None;
    }
    let separator = if base.contains('?') { '&' } else { '?' };
    Some(format!(
        "{base}{separator}plan={plan}&user_id={user_id}",
        plan = format!("{:?}", plan).to_lowercase()
    ))
}

fn checkout_status_allows_activation(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "paid" | "success" | "completed" | "active")
}

pub async fn get_subscription_checkout_return(Query(query): Query<CheckoutReturnQuery>) -> impl IntoResponse {
    let status = query.checkout_status.unwrap_or_else(|| "pending".to_string());
    let plan = query.plan.unwrap_or_else(|| "unknown".to_string());
    let message = if checkout_status_allows_activation(&status) {
        format!("Checkout-Rückkehr für Plan {plan} erkannt. Der finale Aktivierungsstand kann jetzt über Webhook oder Bestätigung verarbeitet werden.")
    } else {
        format!("Checkout-Rückkehr für Plan {plan} mit Status {status} erkannt. Eine Aktivierung erfolgt erst bei erfolgreicher Zahlungsbestätigung.")
    };
    Json(serde_json::json!({
        "status": "return_received",
        "plan": plan,
        "checkout_status": status,
        "user_id": query.user_id,
        "checkout_id": query.checkout_id,
        "message": message,
    }))
}

fn stripe_price_id_for(plan: SubscriptionPlan) -> Option<String> {
    match plan {
        SubscriptionPlan::Free => None,
        SubscriptionPlan::Basic => std::env::var("STRIPE_PRICE_BASIC").ok(),
        SubscriptionPlan::Pro => std::env::var("STRIPE_PRICE_PRO").ok(),
        SubscriptionPlan::Csc => std::env::var("STRIPE_PRICE_CSC").ok(),
    }
}

pub async fn post_subscription_checkout(State(state): State<AppState>, session: SessionUser, Json(payload): Json<CheckoutPayload>) -> Response {
    if let Err(resp) = require_consent(&state, session.user_id, "billing_stripe").await {
        return resp;
    }
    let current_user = match repos::get_or_create_user(&state, session.user_id, session.auth_mode).await {
        Ok(user) => user,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let previous_plan = current_user.plan;
    let normalized_plan = format!("{:?}", payload.plan).to_lowercase();
    let price = checkout_price_eur_month(payload.plan);

    // Try real Stripe checkout if STRIPE_SECRET_KEY is configured
    if let Some(stripe) = StripeClient::from_env() {
        if payload.plan == SubscriptionPlan::Free {
            // Free plan: activate directly, no Stripe needed
            match repos::upsert_user_plan(&state, session.user_id, payload.plan).await {
                Ok(_) => {
                    let _ = repos::enforce_user_plan_limit(&state, session.user_id).await;
                    let _ = repos::record_subscription_history(&state, session.user_id, "plan_downgrade_free", payload.plan, Some(previous_plan), None, Some("stripe"), Some("active"), Some("Downgrade zu Free direkt aktiviert.")).await;
                    return Json(CheckoutResponse {
                        status: "ready".into(),
                        plan: normalized_plan,
                        mode: "direct_plan_change".into(),
                        message: "Free-Plan direkt aktiviert.".into(),
                        checkout_url: None,
                        plant_limit: payload.plan.plant_limit(),
                        price_eur_month: 0.0,
                        activated_immediately: true,
                    }).into_response();
                },
                Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
            }
        }

        let price_id = match stripe_price_id_for(payload.plan) {
            Some(id) => id,
            None => return api_error(StatusCode::BAD_REQUEST, format!("no Stripe price configured for plan {normalized_plan}")),
        };
        let email = repos::get_user_email(&state, session.user_id).await.ok().flatten().unwrap_or_default();
        if email.is_empty() {
            return api_error(StatusCode::BAD_REQUEST, "user email required for Stripe checkout");
        }

        let base_url = std::env::var("CHECKOUT_BASE_URL").unwrap_or_else(|_| "http://localhost:1420".to_string());
        let success_url = format!("{base_url}/checkout/success?plan={normalized_plan}&user_id={}", session.user_id);
        let cancel_url = format!("{base_url}/checkout/cancel");

        match stripe.create_checkout_session(&email, &price_id, &success_url, &cancel_url).await {
            Ok(url) => {
                let _ = repos::record_subscription_history(&state, session.user_id, "stripe_checkout_created", payload.plan, Some(previous_plan), None, Some("stripe"), Some("pending"), Some("Stripe Checkout Session erstellt.")).await;
                return Json(CheckoutResponse {
                    status: "ready".into(),
                    plan: normalized_plan,
                    mode: "stripe_checkout".into(),
                    message: "Stripe Checkout Session erstellt.".into(),
                    checkout_url: Some(url),
                    plant_limit: payload.plan.plant_limit(),
                    price_eur_month: price,
                    activated_immediately: false,
                }).into_response();
            },
            Err(err) => {
                eprintln!("[Stripe] checkout error: {err}");
                return api_error(StatusCode::BAD_GATEWAY, "checkout service temporarily unavailable");
            },
        }
    }

    // Fallback: stub/dev mode (no STRIPE_SECRET_KEY)
    let checkout_url = build_checkout_url(std::env::var("CHECKOUT_BASE_URL").ok().as_deref(), session.user_id, payload.plan);
    let mode = checkout_mode_for(payload.plan).to_string();
    let message = if payload.plan == SubscriptionPlan::Free {
        "Free-Plan wird direkt ohne externen Checkout gesetzt.".to_string()
    } else if checkout_url.is_some() {
        format!(
            "Checkout-Stub für {plan} vorbereitet. In diesem MVP wird der Plan zusätzlich direkt aktiviert.",
            plan = normalized_plan
        )
    } else {
        format!(
            "Checkout-Stub für {plan} vorbereitet. Sobald CHECKOUT_BASE_URL gesetzt ist, wird hier eine externe Checkout-URL zurückgegeben. In diesem MVP wird der Plan zusätzlich direkt aktiviert.",
            plan = normalized_plan
        )
    };
    let checkout_id = Some(format!("stub_{}", Uuid::new_v4()));

    let _ = repos::record_subscription_history(&state, session.user_id, "checkout_prepared", payload.plan, Some(previous_plan), checkout_id.as_deref(), Some("stub"), Some("pending"), Some(&message)).await;

    match repos::upsert_user_plan(&state, session.user_id, payload.plan).await {
        Ok(_) => {
            let _ = repos::enforce_user_plan_limit(&state, session.user_id).await;
            let _ = repos::record_subscription_history(&state, session.user_id, "plan_activated_mvp_direct", payload.plan, Some(previous_plan), checkout_id.as_deref(), Some("stub"), Some("active"), Some("Plan im MVP direkt aktiviert.")).await;
            Json(CheckoutResponse {
                status: "ready".into(),
                plan: normalized_plan,
                mode,
                message,
                checkout_url,
                plant_limit: payload.plan.plant_limit(),
                price_eur_month: price,
                activated_immediately: true,
            }).into_response()
        },
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn post_subscription_webhook(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    // If STRIPE_WEBHOOK_SECRET is set, verify signature and parse Stripe event
    if let Ok(webhook_secret) = std::env::var("STRIPE_WEBHOOK_SECRET") {
        let sig_header = match headers.get("stripe-signature").and_then(|v| v.to_str().ok()) {
            Some(sig) => sig.to_string(),
            None => return api_error(StatusCode::BAD_REQUEST, "missing Stripe-Signature header"),
        };

        if let Err(err) = StripeClient::verify_webhook_signature(&body, &sig_header, &webhook_secret) {
            return api_error(StatusCode::BAD_REQUEST, format!("webhook signature verification failed: {err}"));
        }

        let event: serde_json::Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(err) => return api_error(StatusCode::BAD_REQUEST, format!("invalid webhook JSON: {err}")),
        };

        let event_type = event["type"].as_str().unwrap_or("unknown").to_string();

        // Handle checkout.session.completed
        if event_type == "checkout.session.completed" {
            let session_obj = &event["data"]["object"];
            let customer_email = session_obj["customer_email"].as_str().unwrap_or("");
            let customer_id = session_obj["customer"].as_str().unwrap_or("");

            // Look up user by email
            let user = match repos::get_user_by_email(&state, customer_email).await {
                Ok(Some(u)) => u,
                Ok(None) => {
                    tracing::warn!("stripe webhook: no user found for email {}", pseudonymize_email(customer_email));
                    return Json(serde_json::json!({ "status": "ignored", "reason": "user not found" })).into_response();
                },
                Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
            };

            // Store stripe customer ID
            if !customer_id.is_empty() {
                let _ = repos::set_stripe_customer_id(&state, user.user_id, customer_id).await;
            }

            // Determine plan from line items price or metadata
            let plan = determine_plan_from_stripe_session(session_obj);
            let previous_plan = Some(user.plan);

            if let Err(err) = repos::upsert_user_plan(&state, user.user_id, plan).await {
                return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
            }
            let _ = repos::enforce_user_plan_limit(&state, user.user_id).await;
            let _ = repos::record_subscription_history(&state, user.user_id, "stripe_webhook_activated", plan, previous_plan, session_obj["id"].as_str(), Some("stripe"), Some("active"), Some("Plan via Stripe Webhook aktiviert.")).await;

            return Json(serde_json::json!({ "status": "processed", "event_type": event_type, "plan": format!("{:?}", plan).to_lowercase() })).into_response();
        }

        // Handle customer.subscription.deleted (cancellation)
        if event_type == "customer.subscription.deleted" {
            let sub_obj = &event["data"]["object"];
            let customer_id = sub_obj["customer"].as_str().unwrap_or("");
            // Look up user by stripe customer id — best effort
            if !customer_id.is_empty() {
                // Try to find user by stripe_customer_id
                let row: Option<(uuid::Uuid,)> = sqlx::query_as("SELECT user_id FROM users WHERE stripe_customer_id = $1")
                    .bind(customer_id)
                    .fetch_optional(&state.pg)
                    .await
                    .unwrap_or(None);
                if let Some((user_id,)) = row {
                    let _ = repos::upsert_user_plan(&state, user_id, SubscriptionPlan::Free).await;
                    let _ = repos::enforce_user_plan_limit(&state, user_id).await;
                    let _ = repos::record_subscription_history(&state, user_id, "stripe_subscription_cancelled", SubscriptionPlan::Free, None, None, Some("stripe"), Some("cancelled"), Some("Abo via Stripe Webhook gekuendigt.")).await;
                }
            }
            return Json(serde_json::json!({ "status": "processed", "event_type": event_type })).into_response();
        }

        // Unhandled event type
        return Json(serde_json::json!({ "status": "ignored", "event_type": event_type })).into_response();
    }

    // Fallback: stub/dev mode (no STRIPE_WEBHOOK_SECRET). Forbidden in release builds.
    if cfg!(not(debug_assertions)) {
        tracing::error!("stripe webhook received but STRIPE_WEBHOOK_SECRET is not configured");
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, "webhook not configured");
    }
    tracing::warn!("stripe webhook: STRIPE_WEBHOOK_SECRET not set — accepting unsigned dev payload. DO NOT use in production.");
    let payload: CheckoutWebhookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(err) => return api_error(StatusCode::BAD_REQUEST, format!("invalid payload: {err}")),
    };
    let normalized_plan = format!("{:?}", payload.plan).to_lowercase();
    let event_type = payload.event_type.clone().unwrap_or_else(|| "checkout.completed".to_string());
    let provider = payload.provider.clone().unwrap_or_else(|| "stub".to_string());
    let activation_allowed = checkout_status_allows_activation(&payload.checkout_status);
    let previous_plan = repos::get_user(&state, payload.user_id).await.ok().flatten().map(|u| u.plan);

    let _ = repos::record_subscription_history(&state, payload.user_id, "webhook_received", payload.plan, previous_plan, payload.checkout_id.as_deref(), Some(&provider), Some(&payload.checkout_status), Some(&format!("Webhook-Event {event_type} empfangen."))).await;

    if activation_allowed {
        if let Err(err) = repos::upsert_user_plan(&state, payload.user_id, payload.plan).await {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
        }
        let _ = repos::enforce_user_plan_limit(&state, payload.user_id).await;
        let _ = repos::record_subscription_history(&state, payload.user_id, "plan_activated_webhook", payload.plan, previous_plan, payload.checkout_id.as_deref(), Some(&provider), Some(&payload.checkout_status), Some("Plan nach erfolgreichem Webhook aktiviert.")).await;
    }

    Json(CheckoutWebhookResponse {
        status: if activation_allowed { "processed".into() } else { "ignored".into() },
        applied: activation_allowed,
        plan: normalized_plan.clone(),
        user_id: payload.user_id,
        checkout_status: payload.checkout_status.clone(),
        event_type,
        provider,
        checkout_id: payload.checkout_id.clone(),
        activated_immediately: activation_allowed,
        message: if activation_allowed {
            format!("Webhook für Plan {normalized_plan} verarbeitet. Der Nutzerplan wurde aktualisiert.")
        } else {
            format!("Webhook für Plan {normalized_plan} empfangen, aber wegen Checkout-Status {} noch nicht aktiviert.", payload.checkout_status)
        },
    }).into_response()
}

fn determine_plan_from_stripe_session(session: &serde_json::Value) -> SubscriptionPlan {
    // Check metadata first
    if let Some(plan_str) = session["metadata"]["plan"].as_str() {
        match plan_str {
            "basic" => return SubscriptionPlan::Basic,
            "pro" => return SubscriptionPlan::Pro,
            "csc" => return SubscriptionPlan::Csc,
            _ => {}
        }
    }
    // Try to match by price ID from env vars
    if let Some(price_id) = session["line_items"]["data"][0]["price"]["id"].as_str()
        .or_else(|| session["display_items"].as_array().and_then(|a| a.first()).and_then(|i| i["price"]["id"].as_str()))
    {
        if std::env::var("STRIPE_PRICE_BASIC").ok().as_deref() == Some(price_id) {
            return SubscriptionPlan::Basic;
        }
        if std::env::var("STRIPE_PRICE_PRO").ok().as_deref() == Some(price_id) {
            return SubscriptionPlan::Pro;
        }
        if std::env::var("STRIPE_PRICE_CSC").ok().as_deref() == Some(price_id) {
            return SubscriptionPlan::Csc;
        }
    }
    // Fallback to Basic
    SubscriptionPlan::Basic
}

// --- Phase 3g: Stripe Customer Portal ---

#[derive(Deserialize)]
pub struct PortalPayload {
    pub return_url: Option<String>,
}

pub async fn post_subscription_portal(State(state): State<AppState>, session: SessionUser, Json(payload): Json<PortalPayload>) -> impl IntoResponse {
    let stripe = match StripeClient::from_env() {
        Some(s) => s,
        None => return api_error(StatusCode::NOT_IMPLEMENTED, "Stripe is not configured (STRIPE_SECRET_KEY not set)"),
    };

    let customer_id = match repos::get_stripe_customer_id(&state, session.user_id).await {
        Ok(Some(id)) => id,
        Ok(None) => return api_error(StatusCode::BAD_REQUEST, "no Stripe customer linked to this account"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    // The client-supplied return_url is reflected into Stripe's redirect, so an
    // arbitrary value would turn this endpoint into an open redirect. Only
    // accept URLs under the configured checkout base.
    let base_url = std::env::var("CHECKOUT_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:1420".to_string());
    let return_url = match payload.return_url {
        Some(candidate) if candidate.starts_with(&base_url) => candidate,
        Some(rejected) => {
            eprintln!("[Stripe] rejected off-site return_url: {rejected}");
            base_url
        }
        None => base_url,
    };

    match stripe.create_portal_session(&customer_id, &return_url).await {
        Ok(url) => Json(serde_json::json!({ "portal_url": url })).into_response(),
        Err(err) => {
            eprintln!("[Stripe] portal error: {err}");
            api_error(StatusCode::BAD_GATEWAY, "subscription portal temporarily unavailable")
        },
    }
}

// --- Phase 2d: Settings / Account endpoints ---

#[derive(Deserialize)]
pub struct UpdateProfilePayload {
    pub locale: Option<String>,
    pub timezone: Option<String>,
}

pub async fn update_profile(
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<UpdateProfilePayload>,
) -> impl IntoResponse {
    if let Some(ref locale) = payload.locale {
        if locale.is_empty() || locale.len() > 20 {
            return api_error(StatusCode::BAD_REQUEST, "invalid locale");
        }
    }
    if let Some(ref tz) = payload.timezone {
        if tz.is_empty() || tz.len() > 60 {
            return api_error(StatusCode::BAD_REQUEST, "invalid timezone");
        }
    }
    match repos::update_user_profile(&state, session.user_id, payload.locale, payload.timezone).await {
        Ok(user) => Json(user).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct ChangePasswordPayload {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<ChangePasswordPayload>,
) -> impl IntoResponse {
    if payload.new_password.len() < 8 {
        return api_error(StatusCode::BAD_REQUEST, "new password must be at least 8 characters");
    }
    if payload.new_password.len() > 256 {
        return api_error(StatusCode::BAD_REQUEST, "new password too long");
    }
    let stored_hash = match repos::get_password_hash_by_user_id(&state, session.user_id).await {
        Ok(Some(h)) => h,
        Ok(None) => return api_error(StatusCode::BAD_REQUEST, "no password set for this account"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    if !crate::services::auth::verify_password(&payload.current_password, &stored_hash) {
        return api_error(StatusCode::UNAUTHORIZED, "current password is incorrect");
    }
    let new_hash = match crate::services::auth::hash_password(&payload.new_password) {
        Ok(h) => h,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    if let Err(err) = repos::change_user_password(&state, session.user_id, &new_hash).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }
    // Revoke all refresh tokens on password change
    let _ = repos::revoke_all_refresh_tokens(&state, session.user_id).await;
    Json(ApiMessage { message: "password changed".into() }).into_response()
}

pub async fn delete_account(
    State(state): State<AppState>,
    session: SessionUser,
) -> impl IntoResponse {
    if let Err(err) = repos::delete_user_cascade(&state, session.user_id).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }
    StatusCode::NO_CONTENT.into_response()
}

/// GET /me/export — server-side JSON export of all data for the current user
/// (Art. 15 / Art. 20 GDPR). Response is served with a Content-Disposition
/// header so browsers offer it as a download.
pub async fn export_me(
    State(state): State<AppState>,
    session: SessionUser,
) -> Response {
    let user_id = session.user_id;
    let user = match repos::get_user(&state, user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return api_error(StatusCode::NOT_FOUND, "user not found"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let plants = match repos::list_plants(&state, user_id).await {
        Ok(v) => v,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let tasks = match repos::list_tasks_for_user_since(&state, user_id, None).await {
        Ok(v) => v,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let logs = match repos::list_logs_for_user_since(&state, user_id, None).await {
        Ok(v) => v,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let locations = match repos::list_locations(&state, user_id).await {
        Ok(v) => v,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    // plant_seasons aggregated over all user plants
    let mut plant_seasons: Vec<serde_json::Value> = Vec::new();
    for p in &plants {
        match repos::list_seasons_for_plant(&state, p.plant_id).await {
            Ok(v) => {
                for s in v {
                    plant_seasons.push(serde_json::to_value(s).unwrap_or(serde_json::Value::Null));
                }
            }
            Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        }
    }
    let subscription_history = match repos::list_subscription_history(&state, user_id, 10_000).await {
        Ok(v) => v,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let subscription = match repos::subscription_status_summary(&state, user_id, user.plan).await {
        Ok(v) => serde_json::to_value(v).unwrap_or(serde_json::Value::Null),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let consents = repos::list_user_consents(&state, user_id).await.unwrap_or_default();

    let body = serde_json::json!({
        "schema_version": 1,
        "exported_at": Utc::now().to_rfc3339(),
        "user": user,
        "plants": plants,
        "tasks": tasks,
        "logs": logs,
        "locations": locations,
        "plant_seasons": plant_seasons,
        "subscriptions": [subscription],
        "subscription_history": subscription_history,
        "consents": consents,
    });

    let json = match serde_json::to_vec(&body) {
        Ok(v) => v,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let filename = format!("plants-calendar-export-{}.json", user_id);
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json; charset=utf-8"),
    );
    if let Ok(val) = axum::http::HeaderValue::from_str(
        &format!("attachment; filename=\"{filename}\"")
    ) {
        headers.insert(axum::http::header::CONTENT_DISPOSITION, val);
    }
    (StatusCode::OK, headers, json).into_response()
}

// TODO: Phase 2b - Email verification (requires email service like Resend/SendGrid)
// TODO: Phase 2c - Password reset (requires email service like Resend/SendGrid)

// ── Plant Season endpoints ──

pub async fn list_seasons_for_plant(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    if let Err(err) = repos::verify_plant_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    match repos::list_seasons_for_plant(&state, id).await {
        Ok(seasons) => Json(seasons).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct CreateSeasonPayload {
    pub year: i32,
    pub notes: Option<String>,
}

pub async fn create_season_for_plant(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<CreateSeasonPayload>,
) -> impl IntoResponse {
    if let Err(err) = repos::verify_plant_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    let now = Utc::now();
    let season = shared::PlantSeason {
        season_id: Uuid::new_v4(),
        plant_id: id,
        year: payload.year,
        started_at: Some(now),
        ended_at: None,
        notes: payload.notes,
        harvest_kg: None,
        harvest_count: None,
        pruning_done: false,
        created_at: now,
        updated_at: now,
    };
    match repos::insert_season(&state, &season).await {
        Ok(_) => (StatusCode::CREATED, Json(season)).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct UpdateSeasonPayload {
    pub notes: Option<String>,
    pub harvest_kg: Option<f32>,
    pub harvest_count: Option<i32>,
    pub pruning_done: Option<bool>,
}

pub async fn update_season(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser, Json(payload): Json<UpdateSeasonPayload>) -> impl IntoResponse {
    if let Err(err) = repos::verify_season_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    match repos::update_season(&state, id, payload.notes, payload.harvest_kg, payload.harvest_count, payload.pruning_done).await {
        Ok(season) => Json(season).into_response(),
        Err(err) if err.to_string().contains("not found") => api_error(StatusCode::NOT_FOUND, err),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn delete_season(Path(id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    if let Err(err) = repos::verify_season_ownership(&state, session.user_id, id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    match repos::delete_season(&state, id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn sync_push(
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<SyncPushRequest>,
) -> Response {
    // GDPR consent gate (defence-in-depth): require cloud_sync consent.
    if let Err(resp) = require_consent(&state, session.user_id, "cloud_sync").await {
        return resp;
    }
    // Phase 4a: Payload size guard
    let total_records = payload.envelope.plants.len()
        + payload.envelope.tasks.len()
        + payload.envelope.logs.len()
        + payload.deleted_records.len();
    if total_records > 500 {
        return api_error(StatusCode::PAYLOAD_TOO_LARGE, format!("sync payload too large: {total_records} records (max 500)"));
    }

    if let Some(guest_user_id) = payload.guest_user_id {
        if guest_user_id != session.user_id && !session.is_guest() {
            if let Err(err) = repos::merge_guest_data(&state, guest_user_id, session.user_id).await {
                return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
            }
        }
    }

    // Phase 4a: Wrap all upserts in a single Postgres transaction with LWW guards
    let mut tx = match state.pg.begin().await {
        Ok(tx) => tx,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    // Verify all plants in the envelope belong to the user
    for plant in &payload.envelope.plants {
        if plant.user_id != session.user_id {
            return api_error(StatusCode::FORBIDDEN, "cannot sync plants owned by another user");
        }
    }
    // Verify all tasks belong to user's plants
    for task in &payload.envelope.tasks {
        if let Err(_) = repos::verify_plant_ownership(&state, session.user_id, task.plant_id).await {
            return api_error(StatusCode::FORBIDDEN, "cannot sync tasks for unowned plants");
        }
    }
    for log in &payload.envelope.logs {
        if let Err(_) = repos::verify_plant_ownership(&state, session.user_id, log.plant_id).await {
            return api_error(StatusCode::FORBIDDEN, "cannot sync logs for unowned plants");
        }
    }

    for mut plant in payload.envelope.plants {
        plant.user_id = session.user_id;
        plant.normalize_in_place();
        if let Err(err) = repos::upsert_plant_tx(&mut tx, &plant).await {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
        }
    }
    for task in payload.envelope.tasks {
        if let Err(err) = repos::upsert_task_tx(&mut tx, &task).await {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
        }
    }
    for log in payload.envelope.logs {
        if let Err(err) = repos::upsert_log_tx(&mut tx, &log).await {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
        }
    }

    for deleted in payload.deleted_records {
        let kind_str = match deleted.entity_kind {
            shared::SyncEntityKind::Plant => "plant",
            shared::SyncEntityKind::Task => "task",
            shared::SyncEntityKind::Log => "log",
        };
        if let Err(err) = repos::record_delete_tx(&mut tx, session.user_id, deleted.record_id, kind_str).await {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
        }
    }

    if let Err(err) = tx.commit().await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }

    let _ = repos::enforce_user_plan_limit(&state, session.user_id).await;
    Json(serde_json::json!({ "status": "ok", "synced_at": Utc::now() })).into_response()
}

#[derive(Deserialize)]
pub struct SyncPullQuery {
    pub since: Option<chrono::DateTime<Utc>>,
}

pub async fn sync_pull(
    State(state): State<AppState>,
    session: SessionUser,
    Query(query): Query<SyncPullQuery>,
) -> Response {
    if let Err(resp) = require_consent(&state, session.user_id, "cloud_sync").await {
        return resp;
    }
    match repos::list_changes_since(&state, session.user_id, query.since).await {
        Ok((plants, tasks, logs)) => {
            let deleted_records = match repos::list_deleted_records_since(&state, session.user_id, query.since).await { Ok(v) => v, Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err), };
            Json(SyncPullResponse {
            cursor: SyncCursor {
                user_id: Some(session.user_id),
                device_id: None,
                last_synced_at: Some(Utc::now()),
            },
            envelope: SyncEnvelope { plants, tasks, logs },
            deleted_records,
        }).into_response() },
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

// ── GDPR consent endpoints (Phase C2) ──

pub async fn list_consents(State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    match repos::list_user_consents(&state, session.user_id).await {
        Ok(records) => Json(records).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct ConsentUpsertPayload {
    pub consent_type: String,
    pub granted: bool,
    pub policy_version: String,
}

pub async fn upsert_consent(
    State(state): State<AppState>,
    session: SessionUser,
    Json(payload): Json<ConsentUpsertPayload>,
) -> impl IntoResponse {
    const ALLOWED: &[&str] = &["cloud_sync", "weather", "devices", "billing_stripe"];
    if !ALLOWED.contains(&payload.consent_type.as_str()) {
        return api_error(StatusCode::BAD_REQUEST, "invalid consent_type");
    }
    if payload.policy_version.trim().is_empty() || payload.policy_version.len() > 32 {
        return api_error(StatusCode::BAD_REQUEST, "invalid policy_version");
    }
    match repos::upsert_user_consent(&state, session.user_id, &payload.consent_type, payload.granted, &payload.policy_version).await {
        Ok(record) => Json(record).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}
