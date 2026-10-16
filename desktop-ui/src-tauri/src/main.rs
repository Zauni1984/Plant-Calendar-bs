#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ble;
mod tuya_local;
mod local_db;
mod secrets;

use chrono::{Datelike, Duration, Utc};
use local_db::{ConsentRecord, DirtyBreakdown, LocalStore, ReminderCandidate, StoredSession};
use serde::{Deserialize, Serialize};
use shared::{AuthMode, ClimateConfig, Device, DeviceReading, DirtyRecord, LocalProfile, Location, LogEntry, LogType, Metrics, PhotoRef, Plant, PlantLifecycle, PlantPhase, PlantSeason, PlantStatus, SubscriptionPlan, SubscriptionStatusSummary, SyncPullResponse, SyncPushRequest, Task, TaskCategory, TaskStatus, UserProfile, WeatherRecord};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};
use uuid::Uuid;

use once_cell::sync::OnceCell;

static APP_DATA_DIR: OnceCell<PathBuf> = OnceCell::new();

fn app_data_dir() -> PathBuf {
    APP_DATA_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Round geographic coordinate to 2 decimal places (~1.1 km precision).
/// GDPR Art. 5(1)(c): data minimization for upstream weather requests.
fn round_coord(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else if ty.is_file() && !dest.exists() {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

struct AppLocalState {
    store: Mutex<LocalStore>,
}

#[derive(Debug, Deserialize)]
struct RegisterPayload {
    email: String,
    password: String,
    backend_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginPayload {
    email: String,
    password: String,
    backend_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthResponse {
    user_id: Uuid,
    auth_mode: AuthMode,
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct SessionView {
    session: StoredSession,
    me: UserProfile,
}

#[derive(Debug, Serialize)]
struct SyncStatusView {
    session_active: bool,
    user_id: Option<Uuid>,
    auth_mode: Option<AuthMode>,
    backend_url: Option<String>,
    dirty_count: usize,
    dirty_breakdown: DirtyBreakdown,
    last_sync_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct SyncRunResult {
    pushed_plants: usize,
    pushed_tasks: usize,
    pushed_logs: usize,
    pushed_deleted: usize,
    pulled_plants: usize,
    pulled_tasks: usize,
    pulled_logs: usize,
    pulled_deleted: usize,
}

#[derive(Debug, Deserialize)]
struct ReminderDispatchPayload {
    task_id: String,
    due_at: String,
}

#[derive(Debug, Serialize)]
struct ReminderCheckResult {
    count: usize,
    reminders: Vec<ReminderCandidate>,
}

#[derive(Debug, Serialize)]
struct CreatePlantPayload {
    name: String,
    color_tag: String,
    phase: PlantPhase,
    // Must be sent, otherwise the server creates the plant without them and the
    // response overwrites the local row — silently dropping the user's input.
    #[serde(skip_serializing_if = "Option::is_none")]
    location_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lifecycle: Option<PlantLifecycle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pot_size_liters: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    soil_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateTaskPayload {
    title: String,
    category: TaskCategory,
    due_at: chrono::DateTime<Utc>,
    repeat_interval_hours: Option<i32>,
    // Without this the online path dropped the series end date entirely, so the
    // backend applied its own default and the calendar expanded a wrong series.
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat_until: Option<chrono::DateTime<Utc>>,
    notification_enabled: bool,
}

#[derive(Debug, Serialize)]
struct UpdateTaskPayload {
    title: Option<String>,
    status: Option<TaskStatus>,
    due_at: Option<chrono::DateTime<Utc>>,
    repeat_interval_hours: Option<i32>,
    notification_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
struct CreateLogPayload {
    log_type: LogType,
    text: Option<String>,
    metrics: Option<Metrics>,
    action: Option<serde_json::Value>,
    photo_refs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct PlantCreateInput {
    name: String,
    color_tag: Option<String>,
    phase: Option<String>,
    phase_week: Option<i32>,
    location_id: Option<String>,
    lifecycle: Option<String>,
    plant_year: Option<i32>,
    catalog_id: Option<String>,
    pot_size_liters: Option<f32>,
    soil_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlantUpdateInput {
    plant_id: String,
    name: String,
    color_tag: String,
    phase: String,
    phase_week: Option<i32>,
    #[serde(default)]
    is_active: Option<bool>,
    #[serde(default)]
    archived: Option<bool>,
    status: Option<String>,
    location_id: Option<String>,
    lifecycle: Option<String>,
    plant_year: Option<i32>,
    pot_size_liters: Option<f32>,
    soil_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SeasonUpdateInput {
    season_id: String,
    notes: Option<String>,
    harvest_kg: Option<f32>,
    harvest_count: Option<i32>,
    pruning_done: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TaskCreateInput {
    plant_id: String,
    title: String,
    category: Option<String>,
    due_at: Option<String>,
    repeat_interval_hours: Option<i32>,
    repeat_until: Option<String>,
    notification_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TaskStatusInput {
    task_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct TaskUpdateInput {
    task_id: String,
    title: Option<String>,
    status: Option<String>,
    due_at: Option<String>,
    repeat_interval_hours: Option<i32>,
    notification_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct LogCreateInput {
    plant_id: String,
    log_type: Option<String>,
    created_at: Option<String>,
    text: Option<String>,
    ph: Option<f32>,
    ec: Option<f32>,
    temp_c: Option<f32>,
    rh: Option<f32>,
    harvest_kg: Option<f32>,
    harvest_count: Option<i32>,
    harvest_quality: Option<String>,
    water_liters: Option<f32>,
    water_method: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LogUpdateInput {
    log_id: String,
    log_type: Option<String>,
    text: Option<String>,
    ph: Option<f32>,
    ec: Option<f32>,
    temp_c: Option<f32>,
    rh: Option<f32>,
    harvest_kg: Option<f32>,
    harvest_count: Option<i32>,
    harvest_quality: Option<String>,
    water_liters: Option<f32>,
    water_method: Option<String>,
}

fn parse_plant_status(value: Option<&str>, fallback_archived: bool, fallback_active: bool) -> PlantStatus {
    PlantStatus::from_optional_storage_value(value, fallback_active, fallback_archived)
}

fn resolve_requested_plant_status(payload: &PlantUpdateInput, current_status: PlantStatus) -> PlantStatus {
    shared::resolve_requested_plant_status(
        current_status,
        payload
            .status
            .as_deref()
            .map(|status| PlantStatus::from_optional_storage_value(Some(status), false, false)),
        payload.archived,
        payload.is_active,
    )
}

fn outbound_requested_plant_status(payload: &PlantUpdateInput) -> Option<PlantStatus> {
    shared::outbound_requested_plant_status(
        payload
            .status
            .as_deref()
            .map(|status| PlantStatus::from_optional_storage_value(Some(status), false, false)),
        payload.archived,
        payload.is_active,
    )
}

fn parse_phase(value: Option<&str>) -> PlantPhase {
    match value.unwrap_or("veg").to_lowercase().as_str() {
        "seed" => PlantPhase::Seed,
        "flower" => PlantPhase::Flower,
        "harvest" => PlantPhase::Harvest,
        "dry" => PlantPhase::Dry,
        "cure" => PlantPhase::Cure,
        "custom" => PlantPhase::Custom,
        "dormant" => PlantPhase::Dormant,
        _ => PlantPhase::Veg,
    }
}

fn parse_lifecycle(value: Option<&str>) -> PlantLifecycle {
    match value.unwrap_or("annual").to_lowercase().as_str() {
        "perennial" => PlantLifecycle::Perennial,
        _ => PlantLifecycle::Annual,
    }
}

fn parse_task_category(value: Option<&str>) -> TaskCategory {
    match value.unwrap_or("water").to_lowercase().as_str() {
        "feed" => TaskCategory::Feed,
        "check" => TaskCategory::Check,
        "train" => TaskCategory::Train,
        "note" => TaskCategory::Note,
        _ => TaskCategory::Water,
    }
}


fn parse_due_at(value: Option<&str>, fallback: chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
    value
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok().map(|dt| dt.with_timezone(&Utc)))
        .unwrap_or(fallback)
}

fn parse_task_status(value: &str) -> TaskStatus {
    match value.to_lowercase().as_str() {
        "done" => TaskStatus::Done,
        "skipped" => TaskStatus::Skipped,
        _ => TaskStatus::Open,
    }
}

fn parse_log_type(value: Option<&str>) -> LogType {
    match value.unwrap_or("note").to_lowercase().as_str() {
        "measurement" => LogType::Measurement,
        "photo" => LogType::Photo,
        "action" => LogType::Action,
        "harvest" => LogType::Harvest,
        "watering" => LogType::Watering,
        _ => LogType::Note,
    }
}

fn current_user_id(store: &LocalStore) -> Uuid {
    store.load_session().ok().flatten().map(|s| s.user_id).unwrap_or(Uuid::nil())
}

async fn fetch_me_with_session(session: &StoredSession) -> Result<UserProfile, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/me", session.backend_url))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "fetch me failed".into()));
    }

    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_post_sync_push(session: &StoredSession, payload: &SyncPushRequest) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/sync/push", session.backend_url))
        .bearer_auth(&session.access_token)
        .json(payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "sync push failed".into()));
    }

    Ok(())
}

async fn authorized_get_sync_pull(session: &StoredSession, since: Option<chrono::DateTime<Utc>>) -> Result<SyncPullResponse, String> {
    let client = reqwest::Client::new();
    let mut request = client
        .get(format!("{}/sync/pull", session.backend_url))
        .bearer_auth(&session.access_token);

    if let Some(since) = since {
        request = request.query(&[("since", since.to_rfc3339())]);
    }

    let response = request.send().await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "sync pull failed".into()));
    }

    response.json().await.map_err(|e| e.to_string())
}


async fn authorized_get_plants(session: &StoredSession) -> Result<Vec<Plant>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/plants", session.backend_url))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "list plants failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_create_plant(session: &StoredSession, payload: &CreatePlantPayload) -> Result<Plant, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/plants", session.backend_url))
        .bearer_auth(&session.access_token)
        .json(payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "create plant failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_update_plant(session: &StoredSession, plant_id: Uuid, payload: &PlantUpdateInput) -> Result<Plant, String> {
    let client = reqwest::Client::new();
    let response = client
        .patch(format!("{}/plants/{}", session.backend_url, plant_id))
        .bearer_auth(&session.access_token)
        // Every field the user can edit must be sent. Anything omitted keeps the
        // server's old value, which then overwrites the local row on the
        // follow-up GET — silently reverting the edit.
        .json(&serde_json::json!({
            "name": payload.name.trim(),
            "color_tag": payload.color_tag,
            "phase": parse_phase(Some(&payload.phase)),
            "status": outbound_requested_plant_status(payload).map(|status| status.as_str().to_string()),
            "phase_week": payload.phase_week,
            "location_id": payload.location_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
            "lifecycle": payload.lifecycle.as_deref().map(|s| parse_lifecycle(Some(s))),
            "plant_year": payload.plant_year,
            "pot_size_liters": payload.pot_size_liters,
            "soil_type": payload.soil_type,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "update plant failed".into()));
    }

    let get_response = client
        .get(format!("{}/plants/{}", session.backend_url, plant_id))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !get_response.status().is_success() {
        return Err(get_response.text().await.unwrap_or_else(|_| "fetch updated plant failed".into()));
    }
    get_response.json().await.map_err(|e| e.to_string())
}

async fn authorized_delete_plant(session: &StoredSession, plant_id: Uuid) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .delete(format!("{}/plants/{}", session.backend_url, plant_id))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "delete plant failed".into()));
    }
    Ok(())
}

async fn authorized_get_tasks(session: &StoredSession, plant_id: Option<Uuid>) -> Result<Vec<Task>, String> {
    let client = reqwest::Client::new();
    let url = match plant_id {
        Some(id) => format!("{}/plants/{}/tasks", session.backend_url, id),
        None => return Err("plant id required for online tasks".into()),
    };
    let response = client.get(url).bearer_auth(&session.access_token).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "list tasks failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_create_task(session: &StoredSession, plant_id: Uuid, payload: &CreateTaskPayload) -> Result<Task, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/plants/{}/tasks", session.backend_url, plant_id))
        .bearer_auth(&session.access_token)
        .json(payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "create task failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_update_task(session: &StoredSession, task_id: Uuid, payload: &UpdateTaskPayload) -> Result<Task, String> {
    let client = reqwest::Client::new();
    let response = client
        .patch(format!("{}/tasks/{}", session.backend_url, task_id))
        .bearer_auth(&session.access_token)
        .json(payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "update task failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_delete_task(session: &StoredSession, task_id: Uuid) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .delete(format!("{}/tasks/{}", session.backend_url, task_id))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "delete task failed".into()));
    }
    Ok(())
}

async fn authorized_get_logs(session: &StoredSession, plant_id: Option<Uuid>) -> Result<Vec<LogEntry>, String> {
    let client = reqwest::Client::new();
    let url = match plant_id {
        Some(id) => format!("{}/plants/{}/logs", session.backend_url, id),
        None => return Err("plant id required for online logs".into()),
    };
    let response = client.get(url).bearer_auth(&session.access_token).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "list logs failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_create_log(session: &StoredSession, plant_id: Uuid, payload: &CreateLogPayload) -> Result<LogEntry, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/plants/{}/logs", session.backend_url, plant_id))
        .bearer_auth(&session.access_token)
        .json(payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "create log failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_update_log(session: &StoredSession, log_id: Uuid, payload: &CreateLogPayload) -> Result<LogEntry, String> {
    let client = reqwest::Client::new();
    let response = client
        .patch(format!("{}/logs/{}", session.backend_url, log_id))
        .bearer_auth(&session.access_token)
        .json(payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "update log failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

async fn authorized_delete_log(session: &StoredSession, log_id: Uuid) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .delete(format!("{}/logs/{}", session.backend_url, log_id))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "delete log failed".into()));
    }
    Ok(())
}

fn normalize_backend_url(value: Option<String>) -> Result<String, String> {
    let raw = value.unwrap_or_else(|| "http://127.0.0.1:8080".into());
    let trimmed = raw.trim().trim_end_matches('/').to_string();
    let parsed = url::Url::parse(&trimmed)
        .map_err(|e| format!("invalid backend url: {}", e))?;
    let scheme = parsed.scheme();
    let host = parsed.host_str().unwrap_or("");
    let is_local_host = matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]");
    if scheme == "https" {
        // https allowed for any host
    } else if scheme == "http" && is_local_host {
        // http allowed only for loopback
    } else {
        return Err(format!(
            "backend url must use https, or http with localhost/127.0.0.1/[::1]; got scheme='{}' host='{}'",
            scheme, host
        ));
    }
    Ok(trimmed)
}

#[tauri::command]
fn get_plan_info() -> String {
    format!(
        "Free: {}, Basic: {}, Pro: {}, CSC: {}",
        SubscriptionPlan::Free.plant_limit(),
        SubscriptionPlan::Basic.plant_limit(),
        SubscriptionPlan::Pro.plant_limit(),
        if SubscriptionPlan::Csc.plant_limit() == usize::MAX { "∞".to_string() } else { SubscriptionPlan::Csc.plant_limit().to_string() }
    )
}

fn parse_subscription_plan(value: &str) -> SubscriptionPlan {
    match value.to_lowercase().as_str() {
        "basic" => SubscriptionPlan::Basic,
        "pro" => SubscriptionPlan::Pro,
        "csc" => SubscriptionPlan::Csc,
        _ => SubscriptionPlan::Free,
    }
}


fn local_subscription_summary(store: &LocalStore) -> Result<SubscriptionStatusSummary, String> {
    let user = store.get_or_create_guest_user().map_err(|e| e.to_string())?;
    let plants = store.list_plants().map_err(|e| e.to_string())?;
    Ok(shared::subscription_status_summary(&plants, user.plan))
}

#[tauri::command]
fn bootstrap_local_user(state: State<AppLocalState>) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let user = store.get_or_create_guest_user().map_err(|e| e.to_string())?;
    Ok(format!("{}:{}", user.locale, if user.adult_confirmed { "adult" } else { "pending" }))
}

#[tauri::command]
fn list_local_plants(state: State<AppLocalState>) -> Result<Vec<Plant>, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.list_plants().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_local_plant(state: State<AppLocalState>, payload: PlantCreateInput) -> Result<Plant, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let user = store.get_or_create_guest_user().map_err(|e| e.to_string())?;
    let plants = store.list_plants().map_err(|e| e.to_string())?;
    shared::validate_create_with_plants(&plants, user.plan).map_err(|e| e.to_string())?;
    let now = Utc::now();
    let lifecycle = parse_lifecycle(payload.lifecycle.as_deref());
    let plant = Plant {
        plant_id: Uuid::new_v4(),
        user_id: current_user_id(&store),
        name: payload.name.trim().to_string(),
        color_tag: payload.color_tag.unwrap_or_else(|| "#0B3D2E".into()),
        start_date: now,
        phase: parse_phase(payload.phase.as_deref()),
        phase_week: payload.phase_week,
        status: PlantStatus::Active,
        is_active: true,
        archived: false,
        location_id: payload.location_id.and_then(|s| Uuid::parse_str(&s).ok()),
        lifecycle,
        plant_year: payload.plant_year.or_else(|| if lifecycle == PlantLifecycle::Perennial { Some(now.year()) } else { None }),
        catalog_id: payload.catalog_id,
        pot_size_liters: payload.pot_size_liters,
        soil_type: payload.soil_type,
        created_at: now,
        updated_at: now,
    }
    .normalized();
    store.upsert_plant(&plant).map_err(|e| e.to_string())?;
    Ok(plant)
}

#[tauri::command]
fn update_local_plant(state: State<AppLocalState>, payload: PlantUpdateInput) -> Result<Plant, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let user = store.get_or_create_guest_user().map_err(|e| e.to_string())?;
    let plants = store.list_plants().map_err(|e| e.to_string())?;
    let mut plant = plants
        .iter()
        .find(|p| p.plant_id.to_string() == payload.plant_id)
        .cloned()
        .ok_or_else(|| "plant not found".to_string())?;

    let requested_status = resolve_requested_plant_status(&payload, plant.status);
    if shared::is_reactivation_transition(plant.status, requested_status) {
        shared::validate_reactivate_with_plants(&plants, plant.plant_id, user.plan).map_err(|e| e.to_string())?;
    }

    plant.name = payload.name.trim().to_string();
    plant.color_tag = payload.color_tag;
    plant.phase = parse_phase(Some(&payload.phase));
    plant.phase_week = payload.phase_week;
    plant.location_id = payload.location_id.as_deref().and_then(|s| if s.is_empty() { None } else { Uuid::parse_str(s).ok() });
    if let Some(ref lc) = payload.lifecycle {
        plant.lifecycle = parse_lifecycle(Some(lc));
    }
    if let Some(py) = payload.plant_year {
        plant.plant_year = Some(py);
    }
    if payload.pot_size_liters.is_some() {
        plant.pot_size_liters = payload.pot_size_liters;
    }
    if payload.soil_type.is_some() {
        plant.soil_type = payload.soil_type.clone();
    }
    plant.status = requested_status;
    plant.sync_legacy_flags();
    plant.updated_at = Utc::now();

    store.upsert_plant(&plant).map_err(|e| e.to_string())?;
    Ok(plant)
}

#[tauri::command]
fn delete_local_plant(state: State<AppLocalState>, plant_id: String) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let parsed = Uuid::parse_str(&plant_id).map_err(|e| e.to_string())?;
    store.delete_plant(parsed).map_err(|e| e.to_string())?;
    Ok("plant deleted".into())
}

#[tauri::command]
fn list_local_tasks(state: State<AppLocalState>, plant_id: Option<String>) -> Result<Vec<Task>, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let mut tasks = store.list_tasks().map_err(|e| e.to_string())?;
    if let Some(plant_id) = plant_id {
        tasks.retain(|t| t.plant_id.to_string() == plant_id);
    }
    Ok(tasks)
}

/// Resolve a task's series end date.
///
/// Accepts RFC3339 or plain `YYYY-MM-DD` (interpreted as end of that day).
/// When the caller supplies nothing but the task repeats, defaults to four
/// weeks out so a series always terminates.
fn resolve_repeat_until(
    raw: Option<&str>,
    due_at: chrono::DateTime<Utc>,
    repeat_interval_hours: Option<i32>,
) -> Option<chrono::DateTime<Utc>> {
    if let Some(until_str) = raw {
        return chrono::DateTime::parse_from_rfc3339(until_str)
            .ok()
            .map(|d| d.with_timezone(&Utc))
            .or_else(|| {
                chrono::NaiveDate::parse_from_str(until_str, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(23, 59, 59))
                    .map(|d| d.and_utc())
            });
    }
    if repeat_interval_hours.filter(|&h| h > 0).is_some() {
        return Some(due_at + Duration::weeks(4));
    }
    None
}

#[tauri::command]
fn create_local_task(state: State<AppLocalState>, payload: TaskCreateInput) -> Result<Task, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let plant_id = Uuid::parse_str(&payload.plant_id).map_err(|e| e.to_string())?;
    let now = Utc::now();
    let due_at = parse_due_at(payload.due_at.as_deref(), now + Duration::hours(24));
    let repeat_until = resolve_repeat_until(payload.repeat_until.as_deref(), due_at, payload.repeat_interval_hours);
    let task = Task {
        task_id: Uuid::new_v4(),
        plant_id,
        title: payload.title.trim().to_string(),
        category: parse_task_category(payload.category.as_deref()),
        due_at,
        repeat_interval_hours: payload.repeat_interval_hours,
        repeat_until,
        notification_enabled: payload.notification_enabled.unwrap_or(true),
        status: TaskStatus::Open,
        created_at: now,
        updated_at: now,
    };
    store.upsert_task(&task).map_err(|e| e.to_string())?;
    Ok(task)
}

fn next_recurring_due_after(
    due_at: chrono::DateTime<Utc>,
    repeat_interval_hours: Option<i32>,
    repeat_until: Option<chrono::DateTime<Utc>>,
    now: chrono::DateTime<Utc>,
) -> Option<chrono::DateTime<Utc>> {
    let hours = repeat_interval_hours.filter(|&h| h > 0)?;
    let interval = Duration::hours(hours as i64);
    let mut next = due_at;
    while next <= now {
        next = next + interval;
    }
    // Check if next occurrence is beyond the repeat-until date
    if let Some(until) = repeat_until {
        if next > until {
            return None; // Recurring task has expired
        }
    }
    Some(next)
}

#[tauri::command]
fn update_local_task_status(state: State<AppLocalState>, payload: TaskStatusInput) -> Result<Task, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let mut task = store
        .list_tasks()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|t| t.task_id.to_string() == payload.task_id)
        .ok_or_else(|| "task not found".to_string())?;
    let requested_status = parse_task_status(&payload.status);
    let now = Utc::now();
    task.status = requested_status;
    task.updated_at = now;
    if requested_status == TaskStatus::Done {
        if let Some(next_due) = next_recurring_due_after(task.due_at, task.repeat_interval_hours, task.repeat_until, now) {
            task.status = TaskStatus::Open;
            task.due_at = next_due;
            store.clear_notification_sent_for_task(task.task_id).map_err(|e| e.to_string())?;
        }
    }
    store.upsert_task(&task).map_err(|e| e.to_string())?;
    Ok(task)
}

#[tauri::command]
fn update_local_task(state: State<AppLocalState>, payload: TaskUpdateInput) -> Result<Task, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let task_id = Uuid::parse_str(&payload.task_id).map_err(|e| e.to_string())?;
    let mut task = store
        .list_tasks()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|t| t.task_id == task_id)
        .ok_or_else(|| "task not found".to_string())?;
    if let Some(title) = payload.title { task.title = title.trim().to_string(); }
    let requested_status = payload.status.as_deref().map(parse_task_status);
    if let Some(due_at) = payload.due_at.as_deref() { task.due_at = parse_due_at(Some(due_at), task.due_at); }
    if let Some(repeat) = payload.repeat_interval_hours { task.repeat_interval_hours = Some(repeat); }
    if let Some(enabled) = payload.notification_enabled { task.notification_enabled = enabled; }
    let now = Utc::now();
    if let Some(status) = requested_status {
        task.status = status;
        if status == TaskStatus::Done {
            if let Some(next_due) = next_recurring_due_after(task.due_at, task.repeat_interval_hours, task.repeat_until, now) {
                task.status = TaskStatus::Open;
                task.due_at = next_due;
                store.clear_notification_sent_for_task(task.task_id).map_err(|e| e.to_string())?;
            }
        }
    }
    task.updated_at = now;
    store.upsert_task(&task).map_err(|e| e.to_string())?;
    Ok(task)
}

#[tauri::command]
fn delete_local_task(state: State<AppLocalState>, task_id: String) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let parsed = Uuid::parse_str(&task_id).map_err(|e| e.to_string())?;
    store.delete_task(parsed).map_err(|e| e.to_string())?;
    Ok("task deleted".into())
}

#[tauri::command]
fn list_local_logs(state: State<AppLocalState>, plant_id: Option<String>) -> Result<Vec<LogEntry>, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let mut logs = store.list_logs().map_err(|e| e.to_string())?;
    if let Some(plant_id) = plant_id {
        logs.retain(|l| l.plant_id.to_string() == plant_id);
    }
    Ok(logs)
}

#[tauri::command]
fn create_local_log(state: State<AppLocalState>, payload: LogCreateInput) -> Result<LogEntry, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let plant_id = Uuid::parse_str(&payload.plant_id).map_err(|e| e.to_string())?;
    let has_metrics = payload.ph.is_some() || payload.ec.is_some() || payload.temp_c.is_some() || payload.rh.is_some()
        || payload.harvest_kg.is_some() || payload.harvest_count.is_some() || payload.harvest_quality.is_some()
        || payload.water_liters.is_some() || payload.water_method.is_some();
    let log = LogEntry {
        log_id: Uuid::new_v4(),
        plant_id,
        log_type: parse_log_type(payload.log_type.as_deref()),
        text: payload.text.filter(|t| !t.trim().is_empty()),
        metrics: if has_metrics {
            Some(Metrics {
                ph: payload.ph, ec: payload.ec, temp_c: payload.temp_c, rh: payload.rh, vpd: None,
                harvest_kg: payload.harvest_kg, harvest_count: payload.harvest_count, harvest_quality: payload.harvest_quality,
                water_liters: payload.water_liters, water_method: payload.water_method,
            })
        } else {
            None
        },
        action: None,
        photo_refs: None,
        created_at: parse_due_at(payload.created_at.as_deref(), Utc::now()),
        updated_at: Some(Utc::now()),
    };
    store.upsert_log(&log).map_err(|e| e.to_string())?;
    Ok(log)
}

#[tauri::command]
fn update_local_log(state: State<AppLocalState>, payload: LogUpdateInput) -> Result<LogEntry, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let log_id = Uuid::parse_str(&payload.log_id).map_err(|e| e.to_string())?;
    let mut log = store
        .list_logs()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|l| l.log_id == log_id)
        .ok_or_else(|| "log not found".to_string())?;
    log.log_type = parse_log_type(payload.log_type.as_deref());
    log.text = payload.text.filter(|t| !t.trim().is_empty());
    let has_metrics = payload.ph.is_some() || payload.ec.is_some() || payload.temp_c.is_some() || payload.rh.is_some()
        || payload.harvest_kg.is_some() || payload.harvest_count.is_some() || payload.harvest_quality.is_some()
        || payload.water_liters.is_some() || payload.water_method.is_some();
    log.metrics = if has_metrics { Some(Metrics {
        ph: payload.ph, ec: payload.ec, temp_c: payload.temp_c, rh: payload.rh, vpd: None,
        harvest_kg: payload.harvest_kg, harvest_count: payload.harvest_count, harvest_quality: payload.harvest_quality,
        water_liters: payload.water_liters, water_method: payload.water_method,
    }) } else { None };
    store.upsert_log(&log).map_err(|e| e.to_string())?;
    Ok(log)
}

#[tauri::command]
fn delete_local_log(state: State<AppLocalState>, log_id: String) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let parsed = Uuid::parse_str(&log_id).map_err(|e| e.to_string())?;
    store.delete_log(parsed).map_err(|e| e.to_string())?;
    Ok("log deleted".into())
}

#[tauri::command]
async fn list_current_plants(state: State<'_, AppLocalState>) -> Result<Vec<Plant>, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        let plants = authorized_get_plants(&session).await?;
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.replace_cached_plants(&plants).map_err(|e| e.to_string())?;
        Ok(plants)
    } else {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.list_plants().map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn create_plant_smart(state: State<'_, AppLocalState>, payload: PlantCreateInput) -> Result<Plant, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        let request = CreatePlantPayload {
            name: payload.name.trim().to_string(),
            color_tag: payload.color_tag.unwrap_or_else(|| "#0B3D2E".into()),
            phase: parse_phase(payload.phase.as_deref()),
            location_id: payload.location_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()),
            lifecycle: payload.lifecycle.as_deref().map(|s| parse_lifecycle(Some(s))),
            catalog_id: payload.catalog_id.clone(),
            pot_size_liters: payload.pot_size_liters,
            soil_type: payload.soil_type.clone(),
        };
        let plant = authorized_create_plant(&session, &request).await?;
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.cache_upsert_plant(&plant).map_err(|e| e.to_string())?;
        Ok(plant)
    } else {
        create_local_plant(state, payload)
    }
}

#[tauri::command]
async fn update_plant_smart(state: State<'_, AppLocalState>, payload: PlantUpdateInput) -> Result<Plant, String> {
    let plant_id = Uuid::parse_str(&payload.plant_id).map_err(|e| e.to_string())?;
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        let plant = authorized_update_plant(&session, plant_id, &payload).await?;
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.cache_upsert_plant(&plant).map_err(|e| e.to_string())?;
        Ok(plant)
    } else {
        update_local_plant(state, payload)
    }
}

#[tauri::command]
async fn delete_plant_smart(state: State<'_, AppLocalState>, plant_id: String) -> Result<String, String> {
    let parsed = Uuid::parse_str(&plant_id).map_err(|e| e.to_string())?;
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        authorized_delete_plant(&session, parsed).await?;
    }
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.cache_delete_plant(parsed).map_err(|e| e.to_string())?;
    Ok("plant deleted".into())
}

#[tauri::command]
async fn list_current_tasks(state: State<'_, AppLocalState>, plant_id: Option<String>) -> Result<Vec<Task>, String> {
    let parsed = plant_id.as_ref().and_then(|v| Uuid::parse_str(v).ok());
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        if let Some(pid) = parsed {
            let tasks = authorized_get_tasks(&session, Some(pid)).await?;
            let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
            store.replace_cached_tasks_for_plant(pid, &tasks).map_err(|e| e.to_string())?;
            Ok(tasks)
        } else {
            let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
            store.list_tasks().map_err(|e| e.to_string())
        }
    } else {
        list_local_tasks(state, plant_id)
    }
}

#[tauri::command]
async fn create_task_smart(state: State<'_, AppLocalState>, payload: TaskCreateInput) -> Result<Task, String> {
    let plant_id = Uuid::parse_str(&payload.plant_id).map_err(|e| e.to_string())?;
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        let due_at = parse_due_at(payload.due_at.as_deref(), Utc::now() + Duration::hours(24));
        let request = CreateTaskPayload {
            title: payload.title.trim().to_string(),
            category: parse_task_category(payload.category.as_deref()),
            due_at,
            repeat_interval_hours: payload.repeat_interval_hours,
            repeat_until: resolve_repeat_until(
                payload.repeat_until.as_deref(),
                due_at,
                payload.repeat_interval_hours,
            ),
            notification_enabled: payload.notification_enabled.unwrap_or(true),
        };
        let task = authorized_create_task(&session, plant_id, &request).await?;
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.cache_upsert_task(&task).map_err(|e| e.to_string())?;
        Ok(task)
    } else {
        create_local_task(state, payload)
    }
}

#[tauri::command]
async fn update_task_smart(state: State<'_, AppLocalState>, payload: TaskUpdateInput) -> Result<Task, String> {
    let task_id = Uuid::parse_str(&payload.task_id).map_err(|e| e.to_string())?;
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        let request = UpdateTaskPayload {
            title: payload.title.clone().map(|v| v.trim().to_string()),
            status: payload.status.as_deref().map(parse_task_status),
            due_at: payload.due_at.as_deref().map(|raw| parse_due_at(Some(raw), Utc::now() + Duration::hours(24))),
            repeat_interval_hours: payload.repeat_interval_hours,
            notification_enabled: payload.notification_enabled,
        };
        let task = authorized_update_task(&session, task_id, &request).await?;
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.cache_upsert_task(&task).map_err(|e| e.to_string())?;
        if payload.status.as_deref().map(parse_task_status) == Some(TaskStatus::Done) {
            store.clear_notification_sent_for_task(task.task_id).map_err(|e| e.to_string())?;
        }
        return Ok(task);
    }
    update_local_task(state, payload)
}

#[tauri::command]
async fn update_task_status_smart(state: State<'_, AppLocalState>, payload: TaskStatusInput) -> Result<Task, String> {
    update_task_smart(state, TaskUpdateInput {
        task_id: payload.task_id,
        title: None,
        status: Some(payload.status),
        due_at: None,
        repeat_interval_hours: None,
        notification_enabled: None,
    }).await
}

#[tauri::command]
async fn delete_task_smart(state: State<'_, AppLocalState>, task_id: String) -> Result<String, String> {
    let parsed = Uuid::parse_str(&task_id).map_err(|e| e.to_string())?;
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        authorized_delete_task(&session, parsed).await?;
    }
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.cache_delete_task(parsed).map_err(|e| e.to_string())?;
    Ok("task deleted".into())
}

#[tauri::command]
async fn list_current_logs(state: State<'_, AppLocalState>, plant_id: Option<String>) -> Result<Vec<LogEntry>, String> {
    let parsed = plant_id.as_ref().and_then(|v| Uuid::parse_str(v).ok());
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        if let Some(pid) = parsed {
            let logs = authorized_get_logs(&session, Some(pid)).await?;
            let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
            store.replace_cached_logs_for_plant(pid, &logs).map_err(|e| e.to_string())?;
            Ok(logs)
        } else {
            let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
            store.list_logs().map_err(|e| e.to_string())
        }
    } else {
        list_local_logs(state, plant_id)
    }
}

#[tauri::command]
async fn create_log_smart(state: State<'_, AppLocalState>, payload: LogCreateInput) -> Result<LogEntry, String> {
    let plant_id = Uuid::parse_str(&payload.plant_id).map_err(|e| e.to_string())?;
    let has_metrics = payload.ph.is_some() || payload.ec.is_some() || payload.temp_c.is_some() || payload.rh.is_some()
        || payload.harvest_kg.is_some() || payload.harvest_count.is_some() || payload.harvest_quality.is_some()
        || payload.water_liters.is_some() || payload.water_method.is_some();
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        let request = CreateLogPayload {
            log_type: parse_log_type(payload.log_type.as_deref()),
            text: payload.text.clone().filter(|t| !t.trim().is_empty()),
            metrics: if has_metrics { Some(Metrics {
                ph: payload.ph, ec: payload.ec, temp_c: payload.temp_c, rh: payload.rh, vpd: None,
                harvest_kg: payload.harvest_kg, harvest_count: payload.harvest_count,
                harvest_quality: payload.harvest_quality.clone(),
                water_liters: payload.water_liters, water_method: payload.water_method.clone(),
            }) } else { None },
            action: None,
            photo_refs: None,
        };
        let log = authorized_create_log(&session, plant_id, &request).await?;
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.cache_upsert_log(&log).map_err(|e| e.to_string())?;
        Ok(log)
    } else {
        create_local_log(state, payload)
    }
}

#[tauri::command]
async fn update_log_smart(state: State<'_, AppLocalState>, payload: LogUpdateInput) -> Result<LogEntry, String> {
    let log_id = Uuid::parse_str(&payload.log_id).map_err(|e| e.to_string())?;
    let has_metrics = payload.ph.is_some() || payload.ec.is_some() || payload.temp_c.is_some() || payload.rh.is_some()
        || payload.harvest_kg.is_some() || payload.harvest_count.is_some() || payload.harvest_quality.is_some()
        || payload.water_liters.is_some() || payload.water_method.is_some();
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        let request = CreateLogPayload {
            log_type: parse_log_type(payload.log_type.as_deref()),
            text: payload.text.clone().filter(|t| !t.trim().is_empty()),
            metrics: if has_metrics { Some(Metrics {
                ph: payload.ph, ec: payload.ec, temp_c: payload.temp_c, rh: payload.rh, vpd: None,
                harvest_kg: payload.harvest_kg, harvest_count: payload.harvest_count,
                harvest_quality: payload.harvest_quality.clone(),
                water_liters: payload.water_liters, water_method: payload.water_method.clone(),
            }) } else { None },
            action: None,
            photo_refs: None,
        };
        let log = authorized_update_log(&session, log_id, &request).await?;
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.cache_upsert_log(&log).map_err(|e| e.to_string())?;
        return Ok(log);
    }
    update_local_log(state, payload)
}

#[tauri::command]
async fn delete_log_smart(state: State<'_, AppLocalState>, log_id: String) -> Result<String, String> {
    let parsed = Uuid::parse_str(&log_id).map_err(|e| e.to_string())?;
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        authorized_delete_log(&session, parsed).await?;
    }
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.cache_delete_log(parsed).map_err(|e| e.to_string())?;
    Ok("log deleted".into())
}

// ── Local Profile Commands ──

#[tauri::command]
fn list_profiles(state: State<AppLocalState>) -> Result<Vec<LocalProfile>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.list_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_profile(state: State<AppLocalState>, name: String, role: Option<String>, color: Option<String>, avatar_emoji: Option<String>) -> Result<LocalProfile, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let now = Utc::now();
    let profiles = store.list_profiles().map_err(|e| e.to_string())?;
    let is_owner = profiles.is_empty(); // first profile is owner
    let profile = LocalProfile {
        profile_id: Uuid::new_v4(),
        name: name.trim().to_string(),
        role: if is_owner { "owner".into() } else { role.unwrap_or_else(|| "gardener".into()) },
        color: color.unwrap_or_else(|| "#6bffbc".into()),
        avatar_emoji: avatar_emoji.unwrap_or_else(|| "\u{1F9D1}\u{200D}\u{1F33E}".into()),
        is_owner,
        created_at: now,
        updated_at: now,
    };
    store.upsert_profile(&profile).map_err(|e| e.to_string())?;
    // Auto-activate the first profile
    if is_owner {
        store.set_active_profile(profile.profile_id).map_err(|e| e.to_string())?;
    }
    Ok(profile)
}

#[tauri::command]
fn update_profile_local(state: State<AppLocalState>, profile_id: String, name: Option<String>, role: Option<String>, color: Option<String>, avatar_emoji: Option<String>) -> Result<LocalProfile, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&profile_id).map_err(|e| e.to_string())?;
    let mut profile = store.list_profiles().map_err(|e| e.to_string())?
        .into_iter().find(|p| p.profile_id == pid)
        .ok_or_else(|| "profile not found".to_string())?;
    if let Some(n) = name { profile.name = n.trim().to_string(); }
    if let Some(r) = role { profile.role = r; }
    if let Some(c) = color { profile.color = c; }
    if let Some(a) = avatar_emoji { profile.avatar_emoji = a; }
    profile.updated_at = Utc::now();
    store.upsert_profile(&profile).map_err(|e| e.to_string())?;
    Ok(profile)
}

#[tauri::command]
fn delete_profile_local(state: State<AppLocalState>, profile_id: String) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&profile_id).map_err(|e| e.to_string())?;
    // Prevent deleting the owner profile
    let profile = store.list_profiles().map_err(|e| e.to_string())?
        .into_iter().find(|p| p.profile_id == pid)
        .ok_or_else(|| "profile not found".to_string())?;
    if profile.is_owner {
        return Err("cannot delete the owner profile".to_string());
    }
    store.delete_profile(pid).map_err(|e| e.to_string())
}

#[tauri::command]
fn switch_profile(state: State<AppLocalState>, profile_id: String) -> Result<LocalProfile, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&profile_id).map_err(|e| e.to_string())?;
    let profile = store.list_profiles().map_err(|e| e.to_string())?
        .into_iter().find(|p| p.profile_id == pid)
        .ok_or_else(|| "profile not found".to_string())?;
    store.set_active_profile(pid).map_err(|e| e.to_string())?;
    Ok(profile)
}

#[tauri::command]
fn get_active_profile(state: State<AppLocalState>) -> Result<Option<LocalProfile>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.get_active_profile().map_err(|e| e.to_string())
}

#[tauri::command]
fn assign_plant_to_profile(state: State<AppLocalState>, plant_id: String, profile_id: Option<String>) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&plant_id).map_err(|e| e.to_string())?;
    let prof_id = match profile_id {
        Some(ref s) if !s.is_empty() => Some(Uuid::parse_str(s).map_err(|e| e.to_string())?),
        _ => None,
    };
    store.assign_plant_to_profile(pid, prof_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_locations(state: State<AppLocalState>) -> Result<Vec<Location>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.list_locations().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_location(state: State<AppLocalState>, name: String, location_type: Option<String>, description: Option<String>, latitude: Option<f64>, longitude: Option<f64>) -> Result<Location, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let user = store.get_or_create_guest_user().map_err(|e| e.to_string())?;
    let now = Utc::now();
    let loc = Location {
        location_id: Uuid::new_v4(),
        user_id: user.user_id,
        name: name.trim().to_string(),
        location_type: location_type.unwrap_or_else(|| "custom".into()),
        description,
        latitude,
        longitude,
        created_at: now,
        updated_at: now,
    };
    store.upsert_location(&loc).map_err(|e| e.to_string())?;
    Ok(loc)
}

#[tauri::command]
fn update_location(state: State<AppLocalState>, location_id: String, name: String, location_type: Option<String>, description: Option<String>, latitude: Option<f64>, longitude: Option<f64>) -> Result<Location, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let lid = Uuid::parse_str(&location_id).map_err(|e| e.to_string())?;
    let mut loc = store.list_locations().map_err(|e| e.to_string())?
        .into_iter().find(|l| l.location_id == lid)
        .ok_or_else(|| "location not found".to_string())?;
    loc.name = name.trim().to_string();
    if let Some(lt) = location_type { loc.location_type = lt; }
    if let Some(desc) = description { loc.description = Some(desc); }
    if latitude.is_some() || longitude.is_some() {
        loc.latitude = latitude;
        loc.longitude = longitude;
    }
    loc.updated_at = Utc::now();
    store.upsert_location(&loc).map_err(|e| e.to_string())?;
    Ok(loc)
}

#[tauri::command]
fn delete_location(state: State<AppLocalState>, location_id: String) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let lid = Uuid::parse_str(&location_id).map_err(|e| e.to_string())?;
    store.delete_location(lid).map_err(|e| e.to_string())?;
    Ok("location deleted".into())
}

#[tauri::command]
fn dirty_sync_count(state: State<AppLocalState>) -> Result<usize, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    Ok(store.list_dirty().map_err(|e| e.to_string())?.len())
}

#[tauri::command]
fn export_sync_payload(state: State<AppLocalState>, user_id: Option<String>) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let auth_user_id = user_id.and_then(|v| Uuid::parse_str(&v).ok());
    let payload = store.export_sync_payload(auth_user_id, Some("desktop-dev".into())).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_last_sync_cursor(state: State<AppLocalState>) -> Result<serde_json::Value, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let cursor = store.load_last_sync_cursor().map_err(|e| e.to_string())?;
    serde_json::to_value(cursor).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_sync_status(state: State<AppLocalState>) -> Result<SyncStatusView, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let session = store.load_session().map_err(|e| e.to_string())?;
    let dirty_breakdown = store.dirty_breakdown().map_err(|e| e.to_string())?;
    let dirty_count = dirty_breakdown.plants + dirty_breakdown.tasks + dirty_breakdown.logs + dirty_breakdown.deletes;
    let last_sync_at = store.load_last_sync_cursor().map_err(|e| e.to_string())?.and_then(|c| c.last_synced_at);

    Ok(SyncStatusView {
        session_active: session.is_some(),
        user_id: session.as_ref().map(|s| s.user_id),
        auth_mode: session.as_ref().map(|s| s.auth_mode.clone()),
        backend_url: session.as_ref().map(|s| s.backend_url.clone()),
        dirty_count,
        dirty_breakdown,
        last_sync_at,
    })
}

#[tauri::command]
fn get_dirty_records(state: State<AppLocalState>) -> Result<Vec<DirtyRecord>, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.list_dirty_details().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stored_session(state: State<AppLocalState>) -> Result<Option<StoredSession>, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.load_session().map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_stored_session(state: State<AppLocalState>) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.clear_session().map_err(|e| e.to_string())?;
    Ok("session cleared".into())
}

#[tauri::command]
async fn auth_register(state: State<'_, AppLocalState>, payload: RegisterPayload) -> Result<SessionView, String> {
    let backend_url = normalize_backend_url(payload.backend_url)?;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{backend_url}/auth/register"))
        .json(&serde_json::json!({ "email": payload.email, "password": payload.password }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "register failed".into()));
    }

    let auth: AuthResponse = response.json().await.map_err(|e| e.to_string())?;
    finalize_auth(state, auth, backend_url)
}

#[tauri::command]
async fn auth_login(state: State<'_, AppLocalState>, payload: LoginPayload) -> Result<SessionView, String> {
    let backend_url = normalize_backend_url(payload.backend_url)?;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{backend_url}/auth/login"))
        .json(&serde_json::json!({ "email": payload.email, "password": payload.password }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "login failed".into()));
    }

    let auth: AuthResponse = response.json().await.map_err(|e| e.to_string())?;
    finalize_auth(state, auth, backend_url)
}

#[tauri::command]
async fn auth_fetch_me(state: State<'_, AppLocalState>) -> Result<UserProfile, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?
    };
    fetch_me_with_session(&session).await
}

#[tauri::command]
async fn subscription_status_smart(state: State<'_, AppLocalState>) -> Result<SubscriptionStatusSummary, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session {
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/subscription/status", session.backend_url))
            .bearer_auth(&session.access_token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(response.text().await.unwrap_or_else(|_| "subscription status failed".into()));
        }

        response.json().await.map_err(|e| e.to_string())
    } else {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        local_subscription_summary(&store)
    }
}

#[tauri::command]
async fn subscription_checkout_smart(state: State<'_, AppLocalState>, plan: String) -> Result<serde_json::Value, String> {
    let parsed_plan = parse_subscription_plan(&plan);
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?
    };

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/subscription/checkout", session.backend_url))
        .bearer_auth(&session.access_token)
        .json(&serde_json::json!({ "plan": format!("{:?}", parsed_plan).to_lowercase() }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "subscription checkout failed".into()));
    }

    response.json().await.map_err(|e| e.to_string())
}


#[tauri::command]
async fn subscription_webhook_stub_smart(state: State<'_, AppLocalState>, plan: String, checkout_status: Option<String>) -> Result<serde_json::Value, String> {
    let parsed_plan = parse_subscription_plan(&plan);
    let status_value = checkout_status.unwrap_or_else(|| "paid".to_string());
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?
    };

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/subscription/webhook", session.backend_url))
        .json(&serde_json::json!({
            "user_id": session.user_id,
            "plan": format!("{:?}", parsed_plan).to_lowercase(),
            "checkout_status": status_value,
            "checkout_id": format!("stub-{}-{}", format!("{:?}", parsed_plan).to_lowercase(), session.user_id),
            "event_type": "checkout.completed",
            "provider": "desktop_stub"
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "subscription webhook stub failed".into()));
    }

    response.json().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn subscription_history_smart(state: State<'_, AppLocalState>) -> Result<serde_json::Value, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?
    };

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/subscription/history", session.backend_url))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "subscription history failed".into()));
    }

    response.json().await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn sync_push_now(state: State<'_, AppLocalState>) -> Result<SyncRunResult, String> {
    let (session, payload, pushed) = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        let session = store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?;
        let payload = store.export_sync_payload(Some(session.user_id), Some("desktop-dev".into())).map_err(|e| e.to_string())?;
        let pushed = SyncRunResult {
            pushed_plants: payload.envelope.plants.len(),
            pushed_tasks: payload.envelope.tasks.len(),
            pushed_logs: payload.envelope.logs.len(),
            pushed_deleted: payload.deleted_records.len(),
            pulled_plants: 0,
            pulled_tasks: 0,
            pulled_logs: 0,
            pulled_deleted: 0,
        };
        (session, payload, pushed)
    };

    authorized_post_sync_push(&session, &payload).await?;

    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.clear_confirmed_upserts(&payload.envelope).map_err(|e| e.to_string())?;
    store.clear_confirmed_deleted(&payload.deleted_records).map_err(|e| e.to_string())?;
    Ok(pushed)
}

#[tauri::command]
async fn sync_pull_now(state: State<'_, AppLocalState>) -> Result<SyncRunResult, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?
    };

    let since = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_last_sync_cursor().map_err(|e| e.to_string())?.and_then(|c| c.last_synced_at)
    };

    let pulled = authorized_get_sync_pull(&session, since).await?;
    let result = SyncRunResult {
        pushed_plants: 0,
        pushed_tasks: 0,
        pushed_logs: 0,
        pushed_deleted: 0,
        pulled_plants: pulled.envelope.plants.len(),
        pulled_tasks: pulled.envelope.tasks.len(),
        pulled_logs: pulled.envelope.logs.len(),
        pulled_deleted: pulled.deleted_records.len(),
    };

    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.apply_sync_pull(&pulled.envelope, &pulled.deleted_records, Some(&pulled.cursor)).map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
async fn sync_now(state: State<'_, AppLocalState>) -> Result<SyncRunResult, String> {
    let (session, first_payload, mut pushed_plants, mut pushed_tasks, mut pushed_logs, mut pushed_deleted) = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        let session = store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?;
        let first_payload = store.export_sync_payload(Some(session.user_id), Some("desktop-dev".into())).map_err(|e| e.to_string())?;
        let pushed_plants = first_payload.envelope.plants.len();
        let pushed_tasks = first_payload.envelope.tasks.len();
        let pushed_logs = first_payload.envelope.logs.len();
        let pushed_deleted = first_payload.deleted_records.len();
        (session, first_payload, pushed_plants, pushed_tasks, pushed_logs, pushed_deleted)
    };

    authorized_post_sync_push(&session, &first_payload).await?;

    let since = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_last_sync_cursor().map_err(|e| e.to_string())?.and_then(|c| c.last_synced_at)
    };

    let pulled = authorized_get_sync_pull(&session, since).await?;
    let pulled_plants = pulled.envelope.plants.len();
    let pulled_tasks = pulled.envelope.tasks.len();
    let pulled_logs = pulled.envelope.logs.len();
    let pulled_deleted = pulled.deleted_records.len();

    {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.clear_confirmed_upserts(&first_payload.envelope).map_err(|e| e.to_string())?;
        store.clear_confirmed_deleted(&first_payload.deleted_records).map_err(|e| e.to_string())?;
        store.apply_sync_pull(&pulled.envelope, &pulled.deleted_records, Some(&pulled.cursor)).map_err(|e| e.to_string())?;
    }

    let second_payload = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.export_sync_payload(Some(session.user_id), Some("desktop-dev".into())).map_err(|e| e.to_string())?
    };

    if !second_payload.envelope.plants.is_empty() || !second_payload.envelope.tasks.is_empty() || !second_payload.envelope.logs.is_empty() || !second_payload.deleted_records.is_empty() {
        authorized_post_sync_push(&session, &second_payload).await?;
        {
            let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
            store.clear_confirmed_upserts(&second_payload.envelope).map_err(|e| e.to_string())?;
            store.clear_confirmed_deleted(&second_payload.deleted_records).map_err(|e| e.to_string())?;
        }

        pushed_plants += second_payload.envelope.plants.len();
        pushed_tasks += second_payload.envelope.tasks.len();
        pushed_logs += second_payload.envelope.logs.len();
        pushed_deleted += second_payload.deleted_records.len();

        let final_since = {
            let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
            store.load_last_sync_cursor().map_err(|e| e.to_string())?.and_then(|c| c.last_synced_at)
        };

        let final_pull = authorized_get_sync_pull(&session, final_since).await?;
        let final_pulled_plants = final_pull.envelope.plants.len();
        let final_pulled_tasks = final_pull.envelope.tasks.len();
        let final_pulled_logs = final_pull.envelope.logs.len();
        let final_pulled_deleted = final_pull.deleted_records.len();

        {
            let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
            store.apply_sync_pull(&final_pull.envelope, &final_pull.deleted_records, Some(&final_pull.cursor)).map_err(|e| e.to_string())?;
        }

        return Ok(SyncRunResult {
            pushed_plants,
            pushed_tasks,
            pushed_logs,
            pushed_deleted,
            pulled_plants: pulled_plants + final_pulled_plants,
            pulled_tasks: pulled_tasks + final_pulled_tasks,
            pulled_logs: pulled_logs + final_pulled_logs,
            pulled_deleted: pulled_deleted + final_pulled_deleted,
        });
    }

    Ok(SyncRunResult { pushed_plants, pushed_tasks, pushed_logs, pushed_deleted, pulled_plants, pulled_tasks, pulled_logs, pulled_deleted })
}

#[tauri::command]
fn list_due_notification_candidates(state: State<AppLocalState>, within_minutes: Option<i64>) -> Result<ReminderCheckResult, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let reminders = store.list_due_notification_candidates(within_minutes.unwrap_or(30)).map_err(|e| e.to_string())?;
    Ok(ReminderCheckResult { count: reminders.len(), reminders })
}

#[tauri::command]
fn mark_notification_sent(state: State<AppLocalState>, payload: ReminderDispatchPayload) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let task_id = Uuid::parse_str(&payload.task_id).map_err(|e| e.to_string())?;
    let due_at = chrono::DateTime::parse_from_rfc3339(&payload.due_at).map_err(|e| e.to_string())?.with_timezone(&Utc);
    store.mark_notification_sent(task_id, due_at).map_err(|e| e.to_string())?;
    Ok("notification marked sent".into())
}

fn finalize_auth(state: State<'_, AppLocalState>, auth: AuthResponse, backend_url: String) -> Result<SessionView, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let auth_mode = auth.auth_mode;
    store.link_authenticated_user(auth.user_id, auth_mode.clone()).map_err(|e| e.to_string())?;
    let refresh_token = auth.refresh_token.unwrap_or_default();
    let session = store.save_session(auth.user_id, auth_mode.clone(), auth.access_token, refresh_token, backend_url).map_err(|e| e.to_string())?;
    let me = store.get_or_create_guest_user().map_err(|e| e.to_string())?;
    Ok(SessionView { session, me: UserProfile { user_id: auth.user_id, auth_mode, ..me } })
}

#[derive(Debug, Deserialize)]
struct UpdateProfileInput {
    locale: Option<String>,
    timezone: Option<String>,
}

#[tauri::command]
async fn update_profile_smart(state: State<'_, AppLocalState>, payload: UpdateProfileInput) -> Result<UserProfile, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?
    };
    let client = reqwest::Client::new();
    let response = client
        .patch(format!("{}/me", session.backend_url))
        .bearer_auth(&session.access_token)
        .json(&serde_json::json!({
            "locale": payload.locale,
            "timezone": payload.timezone,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "update profile failed".into()));
    }
    response.json().await.map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
struct ChangePasswordInput {
    current_password: String,
    new_password: String,
}

#[tauri::command]
async fn change_password_smart(state: State<'_, AppLocalState>, payload: ChangePasswordInput) -> Result<String, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?
    };
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/me/change-password", session.backend_url))
        .bearer_auth(&session.access_token)
        .json(&serde_json::json!({
            "current_password": payload.current_password,
            "new_password": payload.new_password,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "change password failed".into()));
    }
    Ok("password changed".into())
}

#[tauri::command]
async fn delete_account_smart(state: State<'_, AppLocalState>) -> Result<String, String> {
    let session = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?.ok_or_else(|| "no stored session".to_string())?
    };
    let client = reqwest::Client::new();
    let response = client
        .delete(format!("{}/me", session.backend_url))
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(response.text().await.unwrap_or_else(|_| "delete account failed".into()));
    }
    // Server-side erasure succeeded → wipe local data (Art. 17 GDPR).
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    store.wipe_all().map_err(|e| e.to_string())?;
    // Remove photos directory contents on disk.
    let photos_root = app_data_dir().join("photos");
    if photos_root.exists() {
        if let Err(e) = std::fs::remove_dir_all(&photos_root) {
            eprintln!("warn: failed to remove photos dir {:?}: {}", photos_root, e);
        }
    }
    store.clear_session().map_err(|e| e.to_string())?;
    Ok("account deleted".into())
}

use tauri_plugin_notification::NotificationExt;

#[tauri::command]
fn check_and_send_notifications(app: tauri::AppHandle, state: State<AppLocalState>) -> Result<usize, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let candidates = store.list_due_notification_candidates(30).map_err(|e| e.to_string())?;
    let mut sent = 0;
    for candidate in &candidates {
        let due_label = candidate.due_at.format("%d.%m.%Y %H:%M").to_string();
        let body = if candidate.overdue {
            format!("Dein Task \"{}\" ist ueberfaellig. Faellig seit {}.", candidate.title, due_label)
        } else {
            format!("Dein Task \"{}\" wird faellig am {}.", candidate.title, due_label)
        };
        let _ = app.notification()
            .builder()
            .title("Plants Calendar")
            .body(&body)
            .show();
        store.mark_notification_sent(candidate.task_id, candidate.due_at).map_err(|e| e.to_string())?;
        sent += 1;
    }
    Ok(sent)
}

// ── BLE / Device commands ────────────────────────────────────────

#[tauri::command]
async fn ble_scan() -> Result<Vec<ble::BleDeviceInfo>, String> {
    let mgr = ble::BleManager::new().await.map_err(|e| e.to_string())?;
    mgr.scan_ggs_devices(5).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn ble_debug_read(state: State<'_, AppLocalState>, device_id: String) -> Result<serde_json::Value, String> {
    let ble_address = {
        let store = state.store.lock().map_err(|_| "lock".to_string())?;
        let did = Uuid::parse_str(&device_id).map_err(|e| e.to_string())?;
        let device = store.get_device(did).map_err(|e| e.to_string())?
            .ok_or_else(|| "device not found".to_string())?;
        device.ble_address.clone()
    };

    let mgr = ble::BleManager::new().await.map_err(|e| e.to_string())?;
    let raw = mgr.read_raw_data(&ble_address).await.map_err(|e| e.to_string())?;

    let hex: String = raw.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
    let printable: String = raw.iter()
        .map(|&b| if (0x20..=0x7E).contains(&b) { b as char } else { '.' })
        .collect();

    // Write full dump to logfile on Desktop (easy to find)
    let log_path = {
        let base = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        let mut dir = std::path::PathBuf::from(base);
        dir.push("Desktop");
        dir.push("ble_logs");
        dir
    };
    let mut log_error: Option<String> = None;
    if let Err(e) = std::fs::create_dir_all(&log_path) {
        log_error = Some(format!("create_dir_all failed: {}", e));
    }
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let log_file = log_path.join(format!("ble_dump_{}_{}.log", device_id, ts));
    let log_content = format!(
        "BLE Dump\nDevice: {}\nAddress: {}\nRaw bytes: {}\nTimestamp: {}\n\n=== HEX ===\n{}\n\n=== ASCII ===\n{}\n",
        device_id,
        ble_address,
        raw.len(),
        chrono::Utc::now().to_rfc3339(),
        hex,
        printable
    );
    if log_error.is_none() {
        if let Err(e) = std::fs::write(&log_file, &log_content) {
            log_error = Some(format!("write failed: {}", e));
        }
    }

    Ok(serde_json::json!({
        "raw_bytes": raw.len(),
        "hex_preview": hex,
        "ascii_preview": printable,
        "log_path": log_file.display().to_string(),
        "log_error": log_error,
    }))
}

#[tauri::command]
async fn lc_read_status(state: State<'_, AppLocalState>, device_id: String) -> Result<serde_json::Value, String> {
    let ble_address = {
        let store = state.store.lock().map_err(|_| "lock".to_string())?;
        let did = Uuid::parse_str(&device_id).map_err(|e| e.to_string())?;
        let device = store.get_device(did).map_err(|e| e.to_string())?
            .ok_or_else(|| "device not found".to_string())?;
        device.ble_address.clone()
    };

    let mgr = ble::BleManager::new().await.map_err(|e| e.to_string())?;
    let raw = mgr.read_raw_data(&ble_address).await.map_err(|e| e.to_string())?;

    let frames = shared::lc_controller::parse_stream(&raw);
    let status = shared::lc_controller::build_status(&frames);

    Ok(serde_json::json!({
        "raw_bytes": raw.len(),
        "frame_count": frames.len(),
        "methods": frames.iter().map(|f| f.method.clone()).collect::<Vec<_>>(),
        "status": status,
    }))
}

#[tauri::command]
async fn ble_read_telemetry(state: State<'_, AppLocalState>, device_id: String) -> Result<shared::GgsTelemetry, String> {
    // Get device BLE address from DB
    let ble_address = {
        let store = state.store.lock().map_err(|_| "lock".to_string())?;
        let did = Uuid::parse_str(&device_id).map_err(|e| e.to_string())?;
        let device = store.get_device(did).map_err(|e| e.to_string())?
            .ok_or_else(|| "device not found".to_string())?;
        device.ble_address.clone()
    };

    // Read telemetry via BLE
    let mgr = ble::BleManager::new().await.map_err(|e| e.to_string())?;
    let telemetry = mgr.read_telemetry(&ble_address).await.map_err(|e| e.to_string())?;

    // Store as reading
    let now = Utc::now();
    let reading = DeviceReading {
        reading_id: Uuid::new_v4(),
        device_id: Uuid::parse_str(&device_id).unwrap_or(Uuid::nil()),
        plant_id: {
            let store = state.store.lock().map_err(|_| "lock".to_string())?;
            let did = Uuid::parse_str(&device_id).unwrap_or(Uuid::nil());
            store.get_device(did).ok().flatten().and_then(|d| d.linked_plant_id)
        },
        temp: telemetry.temp,
        humidity: telemetry.humidity,
        vpd: telemetry.vpd,
        fan_level: Some(telemetry.fan_level as i32),
        light_level: Some(telemetry.light_level as i32),
        created_at: now,
    };

    {
        let store = state.store.lock().map_err(|_| "lock".to_string())?;
        store.insert_reading(&reading).map_err(|e| e.to_string())?;
        // Update device last_seen
        let did = Uuid::parse_str(&device_id).unwrap_or(Uuid::nil());
        if let Ok(Some(mut device)) = store.get_device(did) {
            device.last_seen = now;
            device.updated_at = now;
            // Heartbeat only — a failure must not break the telemetry read,
            // but it should not vanish silently either.
            if let Err(e) = store.upsert_device(&device) {
                eprintln!("[devices] could not persist last_seen for {did}: {e}");
            }
        }
    }

    Ok(telemetry)
}

/// Query a Tuya device on the local network for sensor data.
/// Requires device_id, local_key, and IP address stored on the device record.
#[tauri::command]
async fn tuya_read_status(state: State<'_, AppLocalState>, device_id: String) -> Result<serde_json::Value, String> {
    let mut device = {
        let store = state.store.lock().map_err(|_| "lock".to_string())?;
        let did = Uuid::parse_str(&device_id).map_err(|e| e.to_string())?;
        store.get_device(did).map_err(|e| e.to_string())?
            .ok_or_else(|| "device not found".to_string())?
    };

    // GDPR Art. 32: legacy rows stored the Tuya local_key in plaintext.
    // First time we see one, migrate it into the OS keychain and rewrite the
    // row to a `keyring:<device_id>` pointer.
    if let Some(stripped) = split_tuya_suffix(&device.ble_address).0.strip_prefix("tuya:") {
        if let Some((_ip, key)) = stripped.split_once(':') {
            if !key.starts_with("keyring:") {
                let mut migrated = device.clone();
                // Best-effort here: if the keychain is unavailable we keep using
                // the in-memory key for this read rather than failing the call,
                // but we never rewrite the row to claim it was migrated.
                match redact_tuya_secret_for_storage(&mut migrated) {
                    Ok(()) if migrated.ble_address != device.ble_address => {
                        let store = state.store.lock().map_err(|_| "lock".to_string())?;
                        let _ = store.upsert_device(&migrated);
                        device = migrated;
                    }
                    Ok(()) => {}
                    Err(e) => eprintln!("[tuya] keychain migration skipped: {e}"),
                }
            }
        }
    }

    // Device ble_address field is repurposed to store "tuya:IP:LOCAL_KEY" for Tuya devices
    let tuya_config = parse_tuya_config(&device.ble_address)
        .ok_or_else(|| "Device not configured for Tuya local. Expected ble_address format: tuya:<IP>:<LOCAL_KEY>".to_string())?;

    let config = tuya_local::TuyaDeviceConfig {
        device_id: tuya_device_id(&device),
        local_key: tuya_config.1.clone(),
        ip_address: tuya_config.0.clone(),
        port: 6668,
    };

    let status = tuya_local::query_device_status(&config).await.map_err(|e| e.to_string())?;

    // Also store a reading from the data
    let telemetry = tuya_local::parse_dps_to_telemetry(&status);
    let now = Utc::now();
    let reading = DeviceReading {
        reading_id: Uuid::new_v4(),
        device_id: Uuid::parse_str(&device_id).unwrap_or(Uuid::nil()),
        plant_id: device.linked_plant_id,
        temp: telemetry.temp,
        humidity: telemetry.humidity,
        vpd: telemetry.vpd,
        fan_level: Some(telemetry.fan_level as i32),
        light_level: Some(telemetry.light_level as i32),
        created_at: now,
    };
    {
        let store = state.store.lock().map_err(|_| "lock".to_string())?;
        if let Err(e) = store.insert_reading(&reading) {
            eprintln!("[devices] could not store sensor reading: {e}");
        }
    }

    // Return full status + parsed telemetry
    Ok(serde_json::json!({
        "raw_dps": status,
        "telemetry": telemetry,
    }))
}

/// Parse "tuya:192.168.1.100:<key-or-keyring-pointer>" format.
///
/// GDPR Art. 32: when the second component is `keyring:<device_id>` we look
/// up the actual local_key from the OS keychain rather than holding it in the
/// SQLite row.
/// Split a Tuya `ble_address` into its config part and the optional
/// `#<tuya-device-id>` suffix.
///
/// Older rows have no suffix — they kept the Tuya device id in `Device.name`,
/// which meant the name the user typed was never stored.
fn split_tuya_suffix(address: &str) -> (&str, Option<&str>) {
    match address.split_once('#') {
        Some((cfg, id)) if !id.is_empty() => (cfg, Some(id)),
        _ => (address, None),
    }
}

/// The Tuya device id for a device, preferring the address suffix and falling
/// back to the legacy `name` field.
fn tuya_device_id(device: &shared::Device) -> String {
    split_tuya_suffix(&device.ble_address)
        .1
        .map(|s| s.to_string())
        .unwrap_or_else(|| device.name.clone())
}

fn parse_tuya_config(address: &str) -> Option<(String, String)> {
    let (address, _) = split_tuya_suffix(address);
    let stripped = address.strip_prefix("tuya:")?;
    let parts: Vec<&str> = stripped.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    let ip = parts[0].to_string();
    let key_or_pointer = parts[1];
    if let Some(device_id) = key_or_pointer.strip_prefix("keyring:") {
        let secret_name = format!("tuya_{device_id}");
        match secrets::get(&secret_name) {
            Ok(Some(real)) => Some((ip, real)),
            _ => None,
        }
    } else {
        Some((ip, key_or_pointer.to_string()))
    }
}

/// Prepare a Device's `ble_address` for storage. If it carries a plaintext
/// Tuya local_key, move the key into the OS keychain (GDPR Art. 32) and
/// rewrite the column to the `keyring:<device_id>` pointer.
/// Move a Tuya `local_key` out of the DB row and into the OS keychain.
/// Returns Err if the keychain write fails — the caller must NOT persist the
/// device in that case, otherwise the secret silently stays in plaintext.
fn redact_tuya_secret_for_storage(device: &mut shared::Device) -> Result<(), String> {
    let (config_part, suffix) = split_tuya_suffix(&device.ble_address);
    let suffix = suffix.map(|s| s.to_string());
    let Some(stripped) = config_part.strip_prefix("tuya:") else { return Ok(()); };
    let parts: Vec<&str> = stripped.splitn(2, ':').collect();
    if parts.len() != 2 { return Ok(()); }
    let ip = parts[0].to_string();
    let key = parts[1];
    if key.starts_with("keyring:") { return Ok(()); } // already redacted
    let secret_name = format!("tuya_{}", device.device_id);
    secrets::set(&secret_name, key).map_err(|e| {
        format!("could not store Tuya key in the OS keychain: {e}")
    })?;
    device.ble_address = match suffix {
        Some(id) => format!("tuya:{}:keyring:{}#{}", ip, device.device_id, id),
        None => format!("tuya:{}:keyring:{}", ip, device.device_id),
    };
    Ok(())
}

#[tauri::command]
fn list_devices(state: State<AppLocalState>) -> Result<Vec<Device>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.list_devices().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_device(state: State<AppLocalState>, mut device: Device) -> Result<(), String> {
    // GDPR Art. 32: never persist Tuya local_key in plaintext.
    // Refuse the save outright if the keychain is unavailable.
    redact_tuya_secret_for_storage(&mut device)?;
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.upsert_device(&device).map_err(|e| e.to_string())
}

#[tauri::command]
fn link_device(state: State<AppLocalState>, device_id: String, plant_id: Option<String>) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let did = Uuid::parse_str(&device_id).map_err(|e| e.to_string())?;
    let pid = plant_id.and_then(|p| Uuid::parse_str(&p).ok());
    store.link_device_to_plant(did, pid).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_device(state: State<AppLocalState>, device_id: String) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let did = Uuid::parse_str(&device_id).map_err(|e| e.to_string())?;
    // GDPR Art. 32: also wipe the keychain entry, if any.
    let _ = secrets::delete(&format!("tuya_{}", did));
    store.delete_device(did).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_device_readings(state: State<AppLocalState>, device_id: Option<String>, plant_id: Option<String>, limit: Option<usize>) -> Result<Vec<DeviceReading>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let lim = limit.unwrap_or(100);
    if let Some(did) = device_id {
        let id = Uuid::parse_str(&did).map_err(|e| e.to_string())?;
        store.list_readings_for_device(id, lim).map_err(|e| e.to_string())
    } else if let Some(pid) = plant_id {
        let id = Uuid::parse_str(&pid).map_err(|e| e.to_string())?;
        store.list_readings_for_plant(id, lim).map_err(|e| e.to_string())
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
fn save_device_reading(state: State<AppLocalState>, reading: DeviceReading) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.insert_reading(&reading).map_err(|e| e.to_string())
}

#[tauri::command]
fn prune_old_readings(state: State<AppLocalState>, days: Option<i64>) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.prune_old_readings(days.unwrap_or(90)).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_auto_log_from_reading(state: State<AppLocalState>, device_id: String) -> Result<Option<LogEntry>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let did = Uuid::parse_str(&device_id).map_err(|e| e.to_string())?;
    let device = store.get_device(did).map_err(|e| e.to_string())?
        .ok_or("device not found")?;
    let plant_id = device.linked_plant_id.ok_or("device not linked to plant")?;

    let readings = store.list_readings_for_device(did, 1).map_err(|e| e.to_string())?;
    let reading = readings.first().ok_or("no readings available")?;

    let now = chrono::Utc::now();
    let log = LogEntry {
        log_id: Uuid::new_v4(),
        plant_id,
        log_type: LogType::Measurement,
        text: Some(format!("Auto: {:.1}\u{00b0}C \u{00b7} {:.0}% RH \u{00b7} VPD {:.2}",
            reading.temp.unwrap_or(0.0), reading.humidity.unwrap_or(0.0), reading.vpd.unwrap_or(0.0))),
        metrics: Some(Metrics {
            ph: None, ec: None,
            temp_c: reading.temp,
            rh: reading.humidity,
            vpd: reading.vpd,
            harvest_kg: None, harvest_count: None, harvest_quality: None,
            water_liters: None, water_method: None,
        }),
        action: None,
        photo_refs: None,
        created_at: now,
        updated_at: Some(now),
    };

    store.upsert_log(&log).map_err(|e| e.to_string())?;
    Ok(Some(log))
}

// ── Plant Season commands ──

#[tauri::command]
fn list_plant_seasons(state: State<AppLocalState>, plant_id: String) -> Result<Vec<PlantSeason>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&plant_id).map_err(|e| e.to_string())?;
    store.list_seasons_for_plant(pid).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_plant_season(state: State<AppLocalState>, plant_id: String, year: i32, notes: Option<String>) -> Result<PlantSeason, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&plant_id).map_err(|e| e.to_string())?;
    let now = Utc::now();
    let season = PlantSeason {
        season_id: Uuid::new_v4(),
        plant_id: pid,
        year,
        started_at: Some(now),
        ended_at: None,
        notes,
        harvest_kg: None,
        harvest_count: None,
        pruning_done: false,
        created_at: now,
        updated_at: now,
    };
    store.upsert_season(&season).map_err(|e| e.to_string())?;
    Ok(season)
}

#[tauri::command]
fn update_plant_season(state: State<AppLocalState>, payload: SeasonUpdateInput) -> Result<PlantSeason, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let sid = Uuid::parse_str(&payload.season_id).map_err(|e| e.to_string())?;
    let seasons_all: Vec<PlantSeason> = {
        // We need to find the season. Scan all plants' seasons.
        // Since we don't have a direct get_season_by_id, search via all plants.
        let plants = store.list_plants().map_err(|e| e.to_string())?;
        let mut found = Vec::new();
        for plant in &plants {
            let ss = store.list_seasons_for_plant(plant.plant_id).map_err(|e| e.to_string())?;
            for s in ss {
                if s.season_id == sid {
                    found.push(s);
                }
            }
        }
        found
    };
    let mut season = seasons_all.into_iter().next().ok_or_else(|| "season not found".to_string())?;
    if let Some(notes) = payload.notes {
        season.notes = Some(notes);
    }
    if let Some(kg) = payload.harvest_kg {
        season.harvest_kg = Some(kg);
    }
    if let Some(count) = payload.harvest_count {
        season.harvest_count = Some(count);
    }
    if let Some(pruning) = payload.pruning_done {
        season.pruning_done = pruning;
    }
    season.updated_at = Utc::now();
    store.upsert_season(&season).map_err(|e| e.to_string())?;
    Ok(season)
}

#[tauri::command]
fn delete_plant_season(state: State<AppLocalState>, season_id: String) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let sid = Uuid::parse_str(&season_id).map_err(|e| e.to_string())?;
    store.delete_season(sid).map_err(|e| e.to_string())?;
    Ok("season deleted".into())
}

#[tauri::command]
fn start_new_season(state: State<AppLocalState>, plant_id: String) -> Result<PlantSeason, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&plant_id).map_err(|e| e.to_string())?;
    let now = Utc::now();
    let current_year = now.year();

    // End any existing season for the previous year
    let existing = store.list_seasons_for_plant(pid).map_err(|e| e.to_string())?;
    for mut s in existing {
        if s.ended_at.is_none() {
            s.ended_at = Some(now);
            s.updated_at = now;
            store.upsert_season(&s).map_err(|e| e.to_string())?;
        }
    }

    // Create new season for current year
    let season = PlantSeason {
        season_id: Uuid::new_v4(),
        plant_id: pid,
        year: current_year,
        started_at: Some(now),
        ended_at: None,
        notes: None,
        harvest_kg: None,
        harvest_count: None,
        pruning_done: false,
        created_at: now,
        updated_at: now,
    };
    store.upsert_season(&season).map_err(|e| e.to_string())?;

    // Reset plant phase to seed/veg for the new season and update plant_year
    let mut plant = store.list_plants().map_err(|e| e.to_string())?
        .into_iter().find(|p| p.plant_id == pid)
        .ok_or_else(|| "plant not found".to_string())?;
    plant.phase = PlantPhase::Veg;
    plant.phase_week = Some(1);
    plant.plant_year = Some(current_year);
    plant.updated_at = now;
    store.upsert_plant(&plant).map_err(|e| e.to_string())?;

    Ok(season)
}

// ── Photo commands (local-only) ─────────────────────────────────

#[tauri::command]
async fn add_photo(
    state: State<'_, AppLocalState>,
    plant_id: String,
    log_id: Option<String>,
    season_id: Option<String>,
    source_path: String,
    caption: Option<String>,
) -> Result<PhotoRef, String> {
    let pid = Uuid::parse_str(&plant_id).map_err(|e| e.to_string())?;

    // Create photos directory under app data
    let app_dir = app_data_dir();
    let photos_dir = app_dir.join("photos").join(&plant_id);
    std::fs::create_dir_all(&photos_dir).map_err(|e| e.to_string())?;

    // Store a WebP scaled to fit 1000x1000 instead of the original: same
    // pipeline as the server (shared::images), so a phone photo of several MB
    // becomes a few hundred KB and loses its EXIF/GPS metadata on the way.
    // Decoding and encoding take a moment for a large photo; off the UI thread.
    let processed = tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(&source_path).map_err(|e| format!("Failed to read photo: {e}"))?;
        shared::images::to_webp(&bytes).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    let now = Utc::now();
    let filename = format!(
        "{}_{}.webp",
        now.format("%Y%m%d_%H%M%S"),
        &Uuid::new_v4().to_string()[..8],
    );
    let dest_path = photos_dir.join(&filename);
    std::fs::write(&dest_path, &processed.webp).map_err(|e| format!("Failed to save photo: {e}"))?;

    let relative_path = format!("photos/{}/{}", plant_id, filename);

    let photo = PhotoRef {
        photo_id: Uuid::new_v4(),
        plant_id: pid,
        log_id: log_id.and_then(|s| Uuid::parse_str(&s).ok()),
        season_id: season_id.and_then(|s| Uuid::parse_str(&s).ok()),
        file_path: relative_path,
        caption,
        taken_at: now,
        created_at: now,
    };

    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.insert_photo(&photo).map_err(|e| e.to_string())?;
    Ok(photo)
}

#[tauri::command]
fn list_plant_photos(
    state: State<AppLocalState>,
    plant_id: String,
) -> Result<Vec<PhotoRef>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&plant_id).map_err(|e| e.to_string())?;
    store
        .list_photos_for_plant(pid)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_log_photos(
    state: State<AppLocalState>,
    log_id: String,
) -> Result<Vec<PhotoRef>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let lid = Uuid::parse_str(&log_id).map_err(|e| e.to_string())?;
    store.list_photos_for_log(lid).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_photo(
    state: State<AppLocalState>,
    photo_id: String,
) -> Result<String, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let pid = Uuid::parse_str(&photo_id).map_err(|e| e.to_string())?;
    let file_path = store.delete_photo(pid).map_err(|e| e.to_string())?;
    // Delete the actual file
    let app_dir = app_data_dir();
    let full_path = app_dir.join(&file_path);
    std::fs::remove_file(&full_path).ok(); // ignore if file already gone
    Ok("photo deleted".into())
}

#[tauri::command]
fn get_photo_path(photo_path: String) -> Result<String, String> {
    let app_dir = app_data_dir();
    let full_path = app_dir.join(&photo_path);
    Ok(full_path.to_string_lossy().to_string())
}

#[tauri::command]
fn get_plant_catalog() -> Vec<shared::CatalogEntry> {
    shared::plant_catalog().clone()
}

#[tauri::command]
fn get_companion_info(plant_a: String, plant_b: String) -> String {
    match shared::companion_status(&plant_a, &plant_b) {
        shared::CompanionResult::Good => "good".into(),
        shared::CompanionResult::Bad => "bad".into(),
        shared::CompanionResult::Neutral => "neutral".into(),
    }
}

#[tauri::command]
fn get_climate_config(state: State<AppLocalState>) -> Result<Option<ClimateConfig>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.get_climate_config().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_climate_config(state: State<AppLocalState>, config: ClimateConfig) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    store.save_climate_config(&config).map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_weather(lat: f64, lon: f64) -> Result<serde_json::Value, String> {
    // Data minimization: round to ~1 km precision before sending to Open-Meteo.
    let lat = round_coord(lat);
    let lon = round_coord(lon);
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,precipitation,wind_speed_10m&daily=temperature_2m_max,temperature_2m_min,precipitation_sum&timezone=auto&forecast_days=7",
        lat, lon
    );
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_city(query: String) -> Result<serde_json::Value, String> {
    let encoded = urlencoding::encode(&query);
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=5&language=de",
        encoded
    );
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_and_log_weather(state: State<'_, AppLocalState>, location_id: Option<String>, lat: f64, lon: f64) -> Result<serde_json::Value, String> {
    // Data minimization (GDPR Art. 5(1)(c)): round to ~1 km precision before
    // calling Open-Meteo and persisting in weather_history. The user-entered
    // lat/lon stays precise on the location record itself.
    let lat = round_coord(lat);
    let lon = round_coord(lon);
    // Expanded Open-Meteo query: adds apparent temp, UV, cloud cover, pressure, dew point,
    // visibility, wind gusts/direction, soil temp & moisture, day/night flag.
    // Hourly covers the next 7 days for richer charts.
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}\
         &current=temperature_2m,relative_humidity_2m,apparent_temperature,precipitation,rain,showers,snowfall,\
weather_code,cloud_cover,pressure_msl,surface_pressure,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day,uv_index\
         &hourly=temperature_2m,relative_humidity_2m,apparent_temperature,dew_point_2m,precipitation_probability,\
precipitation,rain,showers,snowfall,weather_code,pressure_msl,cloud_cover,visibility,wind_speed_10m,wind_gusts_10m,\
wind_direction_10m,uv_index,soil_temperature_0cm,soil_temperature_6cm,soil_moisture_0_to_1cm,soil_moisture_1_to_3cm,\
soil_moisture_3_to_9cm\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,apparent_temperature_max,apparent_temperature_min,\
sunrise,sunset,daylight_duration,sunshine_duration,uv_index_max,precipitation_sum,rain_sum,showers_sum,snowfall_sum,\
precipitation_hours,precipitation_probability_max,wind_speed_10m_max,wind_gusts_10m_max,wind_direction_10m_dominant,\
shortwave_radiation_sum,et0_fao_evapotranspiration\
         &timezone=auto&forecast_days=7",
        lat, lon
    );
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    // Store current reading in weather_history with extended metrics.
    let current = &data["current"];
    // Derive soil temp/moisture from hourly index 0 when available.
    let hourly = &data["hourly"];
    let soil_temp_c = hourly["soil_temperature_0cm"].get(0).and_then(|v| v.as_f64()).map(|v| v as f32);
    let soil_moisture = hourly["soil_moisture_0_to_1cm"].get(0).and_then(|v| v.as_f64()).map(|v| v as f32);
    let visibility_km = hourly["visibility"].get(0).and_then(|v| v.as_f64()).map(|v| (v / 1000.0) as f32);
    let dew_point_c = hourly["dew_point_2m"].get(0).and_then(|v| v.as_f64()).map(|v| v as f32);

    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let record = WeatherRecord {
        id: Uuid::new_v4(),
        location_id: location_id.as_ref().and_then(|s| Uuid::parse_str(s).ok()),
        latitude: lat,
        longitude: lon,
        temp_c: current["temperature_2m"].as_f64().map(|v| v as f32),
        humidity: current["relative_humidity_2m"].as_f64().map(|v| v as f32),
        wind_kmh: current["wind_speed_10m"].as_f64().map(|v| v as f32),
        precipitation_mm: current["precipitation"].as_f64().map(|v| v as f32),
        weather_code: current["weather_code"].as_i64().map(|v| v as i32),
        recorded_at: Utc::now(),
        apparent_temp_c: current["apparent_temperature"].as_f64().map(|v| v as f32),
        uv_index: current["uv_index"].as_f64().map(|v| v as f32),
        cloud_cover: current["cloud_cover"].as_f64().map(|v| v as f32),
        pressure_hpa: current["pressure_msl"].as_f64()
            .or_else(|| current["surface_pressure"].as_f64())
            .map(|v| v as f32),
        dew_point_c,
        visibility_km,
        wind_gusts_kmh: current["wind_gusts_10m"].as_f64().map(|v| v as f32),
        wind_direction_deg: current["wind_direction_10m"].as_f64().map(|v| v as f32),
        soil_temp_c,
        soil_moisture,
        is_day: current["is_day"].as_i64().map(|v| v != 0),
    };
    // The dashboard's history comes from this table, so a write failure would
    // leave silent gaps. Keep serving the live data, but make the loss visible.
    if let Err(e) = store.insert_weather_record(&record) {
        eprintln!("[weather] could not store weather history record: {e}");
    }

    Ok(data)
}

#[tauri::command]
fn list_weather_history(
    state: State<AppLocalState>,
    location_id: Option<String>,
    hours: Option<i64>,
    days: Option<i64>,
) -> Result<Vec<WeatherRecord>, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let lid = location_id.and_then(|s| Uuid::parse_str(&s).ok());
    // Accept either `hours` or `days` (days preferred for long ranges). Default 24h.
    let h = match (hours, days) {
        (Some(h), _) => h,
        (None, Some(d)) => d.saturating_mul(24),
        (None, None) => 24,
    };
    store.list_weather_history(lid, h).map_err(|e| e.to_string())
}

#[tauri::command]
fn prune_weather_history_cmd(state: State<AppLocalState>, days: Option<i64>) -> Result<usize, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    // Default retention: 365 days. Matches the privacy policy; users may
    // pass a different value explicitly.
    store.prune_weather_history(days.unwrap_or(365)).map_err(|e| e.to_string())
}

// ── CSV Import / Export ──

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            fields.push(current.clone());
            current.clear();
        } else {
            current.push(c);
        }
    }
    fields.push(current);
    fields
}

fn csv_escape(val: &str) -> String {
    if val.contains(',') || val.contains('"') || val.contains('\n') {
        format!("\"{}\"", val.replace('"', "\"\""))
    } else {
        val.to_string()
    }
}

#[tauri::command]
fn export_all_csv(state: State<AppLocalState>) -> Result<serde_json::Value, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let plants = store.list_plants().map_err(|e| e.to_string())?;
    let tasks = store.list_tasks().map_err(|e| e.to_string())?;
    let logs = store.list_logs().map_err(|e| e.to_string())?;
    let locations = store.list_locations().map_err(|e| e.to_string())?;

    // Plants CSV
    let mut plants_csv = String::from("Name,Phase,Woche,Status,Lebenszyklus,Farbe,Standort,Katalog-ID,Startdatum,Erstellt,Aktualisiert\n");
    for p in &plants {
        let loc_name = p.location_id
            .and_then(|lid| locations.iter().find(|l| l.location_id == lid))
            .map(|l| l.name.clone())
            .unwrap_or_default();
        plants_csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&p.name),
            csv_escape(&format!("{:?}", p.phase).to_lowercase()),
            p.phase_week.unwrap_or(0),
            csv_escape(&format!("{:?}", p.status).to_lowercase()),
            csv_escape(&format!("{:?}", p.lifecycle).to_lowercase()),
            csv_escape(&p.color_tag),
            csv_escape(&loc_name),
            csv_escape(p.catalog_id.as_deref().unwrap_or("")),
            p.start_date.format("%Y-%m-%d"),
            p.created_at.format("%Y-%m-%d %H:%M"),
            p.updated_at.format("%Y-%m-%d %H:%M"),
        ));
    }

    // Tasks CSV
    let mut tasks_csv = String::from("Pflanze,Titel,Kategorie,Status,Faellig,Wiederholung_Std,Wiederholen_bis,Erinnerung,Erstellt\n");
    for t in &tasks {
        let plant_name = plants.iter().find(|p| p.plant_id == t.plant_id).map(|p| p.name.clone()).unwrap_or_default();
        tasks_csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&plant_name),
            csv_escape(&t.title),
            csv_escape(&format!("{:?}", t.category).to_lowercase()),
            csv_escape(&format!("{:?}", t.status).to_lowercase()),
            t.due_at.format("%Y-%m-%d %H:%M"),
            t.repeat_interval_hours.map(|h| h.to_string()).unwrap_or_default(),
            t.repeat_until.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
            t.notification_enabled,
            t.created_at.format("%Y-%m-%d %H:%M"),
        ));
    }

    // Logs CSV
    let mut logs_csv = String::from("Pflanze,Typ,Text,pH,EC,Temp_C,RH,VPD,Ernte_kg,Ernte_Stueck,Wasser_Liter,Datum\n");
    for l in &logs {
        let plant_name = plants.iter().find(|p| p.plant_id == l.plant_id).map(|p| p.name.clone()).unwrap_or_default();
        let m = l.metrics.as_ref();
        logs_csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&plant_name),
            csv_escape(&format!("{:?}", l.log_type).to_lowercase()),
            csv_escape(l.text.as_deref().unwrap_or("")),
            m.and_then(|m| m.ph).map(|v| v.to_string()).unwrap_or_default(),
            m.and_then(|m| m.ec).map(|v| v.to_string()).unwrap_or_default(),
            m.and_then(|m| m.temp_c).map(|v| v.to_string()).unwrap_or_default(),
            m.and_then(|m| m.rh).map(|v| v.to_string()).unwrap_or_default(),
            m.and_then(|m| m.vpd).map(|v| v.to_string()).unwrap_or_default(),
            m.and_then(|m| m.harvest_kg).map(|v| v.to_string()).unwrap_or_default(),
            m.and_then(|m| m.harvest_count).map(|v| v.to_string()).unwrap_or_default(),
            m.and_then(|m| m.water_liters).map(|v| v.to_string()).unwrap_or_default(),
            l.created_at.format("%Y-%m-%d %H:%M"),
        ));
    }

    Ok(serde_json::json!({
        "plants": plants_csv,
        "tasks": tasks_csv,
        "logs": logs_csv,
    }))
}

#[tauri::command]
fn import_plants_csv(state: State<AppLocalState>, csv_content: String) -> Result<usize, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    let user = store.get_or_create_guest_user().map_err(|e| e.to_string())?;
    let now = Utc::now();
    let mut imported = 0;

    let lines: Vec<&str> = csv_content.lines().collect();
    if lines.is_empty() {
        return Ok(0);
    }

    // Parse header to detect column positions
    let header_fields: Vec<String> = parse_csv_line(lines[0])
        .iter()
        .map(|s| s.trim().to_lowercase())
        .collect();

    // Map column names to indices — support both German and English headers
    let col = |names: &[&str]| -> Option<usize> {
        for name in names {
            if let Some(idx) = header_fields.iter().position(|h| h == *name) {
                return Some(idx);
            }
        }
        None
    };

    let name_col = col(&["name", "pflanzenname", "plant name", "plant", "pflanze", "sorte", "variety"]);
    let phase_col = col(&["phase", "stadium", "stage", "growth stage"]);
    let week_col = col(&["woche", "week", "phase_week"]);
    let lifecycle_col = col(&["lebenszyklus", "lifecycle", "type", "typ"]);
    let color_col = col(&["farbe", "color", "color_tag", "colour"]);
    let catalog_col = col(&["katalog-id", "catalog_id", "katalog", "catalog", "sorte_id"]);
    let start_col = col(&["startdatum", "start_date", "start", "planted", "planted_date", "pflanzung"]);

    // Name column is required
    let name_idx = name_col.ok_or_else(|| "CSV hat keine erkennbare Name-Spalte (erwartet: Name, Plant, Pflanze, ...)".to_string())?;

    for line in lines.iter().skip(1) {
        let fields = parse_csv_line(line);
        if fields.len() <= name_idx {
            continue;
        }
        let name = fields[name_idx].trim().to_string();
        if name.is_empty() {
            continue;
        }

        let phase_str = phase_col.and_then(|i| fields.get(i)).map(|s| s.trim());
        let week_str = week_col.and_then(|i| fields.get(i)).map(|s| s.trim());
        let lifecycle_str = lifecycle_col.and_then(|i| fields.get(i)).map(|s| s.trim());
        let color_str = color_col.and_then(|i| fields.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let catalog_str = catalog_col.and_then(|i| fields.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let start_str = start_col.and_then(|i| fields.get(i)).map(|s| s.trim());

        let start_date = start_str
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            .unwrap_or(now);

        let plant = Plant {
            plant_id: Uuid::new_v4(),
            user_id: user.user_id,
            name,
            color_tag: color_str.unwrap_or_else(|| "#0B3D2E".into()),
            start_date,
            phase: parse_phase(phase_str),
            phase_week: week_str.and_then(|s| s.parse().ok()),
            status: PlantStatus::Active,
            is_active: true,
            archived: false,
            lifecycle: parse_lifecycle(lifecycle_str),
            plant_year: None,
            catalog_id: catalog_str,
            pot_size_liters: None,
            soil_type: None,
            location_id: None,
            created_at: now,
            updated_at: now,
        };
        store.upsert_plant(&plant).map_err(|e| e.to_string())?;
        imported += 1;
    }
    Ok(imported)
}

// ── GDPR consent commands (Phase C2) ──
//
// Consent types are literal strings used across frontend, backend, and local DB.
// Keep in sync with `CONSENT_TYPES` in desktop-ui/src/main.js.
const CONSENT_TYPES: &[&str] = &["cloud_sync", "weather", "devices", "billing_stripe"];
/// Current privacy-policy version. Bump when the policy text materially changes
/// (the previous consent rows remain as a historical record).
const CONSENT_POLICY_VERSION: &str = "1.0";

fn consent_current_user_id(state: &AppLocalState) -> Result<Uuid, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    if let Some(session) = store.load_session().map_err(|e| e.to_string())? {
        return Ok(session.user_id);
    }
    // Guest → nil UUID row created by bootstrap.
    Ok(Uuid::nil())
}

#[tauri::command]
async fn list_consents_smart(state: State<'_, AppLocalState>) -> Result<Vec<serde_json::Value>, String> {
    // Try authenticated backend first when logged in.
    let session_opt = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session_opt {
        let client = reqwest::Client::new();
        match client
            .get(format!("{}/me/consents", session.backend_url))
            .bearer_auth(&session.access_token)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                if let Ok(v) = response.json::<Vec<serde_json::Value>>().await {
                    return Ok(v);
                }
            }
            _ => {}
        }
        // Fall through to local on any network/auth error.
    }
    let uid = consent_current_user_id(&state)?;
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let records: Vec<ConsentRecord> = store.list_consents(uid).map_err(|e| e.to_string())?;
    Ok(records.into_iter().map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null)).collect())
}

#[tauri::command]
async fn set_consent_smart(state: State<'_, AppLocalState>, consent_type: String, granted: bool) -> Result<serde_json::Value, String> {
    if !CONSENT_TYPES.contains(&consent_type.as_str()) {
        return Err(format!("invalid consent_type: {consent_type}"));
    }
    // Always persist locally (offline-first).
    let uid = consent_current_user_id(&state)?;
    let local_record = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store
            .upsert_consent(uid, &consent_type, granted, CONSENT_POLICY_VERSION)
            .map_err(|e| e.to_string())?
    };
    // If logged in, mirror to backend (best-effort).
    let session_opt = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    if let Some(session) = session_opt {
        let client = reqwest::Client::new();
        if let Ok(response) = client
            .post(format!("{}/me/consents", session.backend_url))
            .bearer_auth(&session.access_token)
            .json(&serde_json::json!({
                "consent_type": consent_type,
                "granted": granted,
                "policy_version": CONSENT_POLICY_VERSION,
            }))
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(v) = response.json::<serde_json::Value>().await {
                    return Ok(v);
                }
            }
        }
    }
    Ok(serde_json::to_value(local_record).unwrap_or(serde_json::Value::Null))
}

/// Open a path (markdown doc, URL) in the host OS default handler.
/// Used for privacy-policy and imprint links.
#[tauri::command]
fn open_external_path(path: String) -> Result<(), String> {
    // Only ever open something that actually exists as a file on disk.
    let candidate = std::path::Path::new(&path);
    if !candidate.is_file() {
        return Err(format!("not a file: {path}"));
    }

    // NOTE: deliberately not `cmd /C start`. cmd.exe re-parses its command line,
    // so shell metacharacters in the path (&, |, ^, ...) would survive Rust's
    // argument escaping and execute. explorer.exe takes the path verbatim.
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer.exe").arg(candidate).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(candidate).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(candidate).spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// Return the absolute path to a bundled/adjacent docs markdown file so the
/// frontend can hand it to `open_external_path`. Tries several candidate roots
/// (current exe dir, cwd, a `docs/` sibling of the app data dir) and returns
/// the first hit.
#[tauri::command]
fn resolve_doc_path(filename: String) -> Result<String, String> {
    // The result is handed straight to open_external_path, so the name must be
    // a plain file name — no traversal, no absolute paths, no separators.
    let is_safe = !filename.is_empty()
        && filename.len() <= 128
        && !filename.contains("..")
        && !filename.contains('/')
        && !filename.contains('\\')
        && !filename.contains(':')
        && filename
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !is_safe {
        return Err(format!("invalid doc name: {filename}"));
    }

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("docs").join(&filename));
            candidates.push(dir.join(&filename));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("docs").join(&filename));
        candidates.push(cwd.join("..").join("docs").join(&filename));
        candidates.push(cwd.join("..").join("..").join("docs").join(&filename));
    }
    for c in candidates {
        if c.exists() {
            return Ok(c.to_string_lossy().into_owned());
        }
    }
    Err(format!("doc not found: {filename}"))
}

/// Server-side JSON export via `GET /me/export` for logged-in users, or a
/// local-only compile for guests. Writes the JSON to the user-chosen path.
#[tauri::command]
async fn export_account_json(state: State<'_, AppLocalState>, path: String) -> Result<String, String> {
    let session_opt = {
        let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
        store.load_session().map_err(|e| e.to_string())?
    };
    let body: serde_json::Value = if let Some(session) = session_opt {
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/me/export", session.backend_url))
            .bearer_auth(&session.access_token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            // Fall back to local export on backend failure.
            build_local_export(&state)?
        } else {
            response.json().await.map_err(|e| e.to_string())?
        }
    } else {
        build_local_export(&state)?
    };
    let bytes = serde_json::to_vec_pretty(&body).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

fn build_local_export(state: &AppLocalState) -> Result<serde_json::Value, String> {
    let store = state.store.lock().map_err(|_| "failed to lock store".to_string())?;
    let plants = store.list_plants().map_err(|e| e.to_string())?;
    let tasks = store.list_tasks().map_err(|e| e.to_string())?;
    let logs = store.list_logs().map_err(|e| e.to_string())?;
    let locations = store.list_locations().map_err(|e| e.to_string())?;
    // Must use the real session user, not Uuid::nil() — otherwise a logged-in
    // user's export silently omits all their consent records (Art. 15/20 GDPR).
    let consents: Vec<ConsentRecord> =
        store.list_consents(current_user_id(&store)).unwrap_or_default();
    Ok(serde_json::json!({
        "schema_version": 1,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "user": serde_json::Value::Null,
        "plants": plants,
        "tasks": tasks,
        "logs": logs,
        "locations": locations,
        "plant_seasons": Vec::<serde_json::Value>::new(),
        "subscriptions": Vec::<serde_json::Value>::new(),
        "subscription_history": Vec::<serde_json::Value>::new(),
        "consents": consents,
    }))
}

#[tauri::command]
fn write_export_file(path: String, content: String) -> Result<(), String> {
    // Write with BOM for Excel compatibility
    let bom = "\u{FEFF}";
    let with_bom = format!("{}{}", bom, content);
    std::fs::write(&path, with_bom.as_bytes()).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_import_file(path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    // Strip BOM if present
    let content = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&bytes[3..]).to_string()
    } else {
        String::from_utf8_lossy(&bytes).to_string()
    };
    Ok(content)
}

fn main() {
    // Retention is driven by the frontend: it runs a sweep shortly after startup
    // using the user's configured retention window (Settings -> data retention),
    // covering both weather_history and device_readings.

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let base = app
                .handle()
                .path()
                .app_local_data_dir()
                .expect("failed to resolve app_local_data_dir");
            std::fs::create_dir_all(&base).ok();
            std::fs::create_dir_all(base.join("photos")).ok();

            let db_path = base.join("plants_calendar_local.sqlite");

            // One-time migration: if the user has a DB in the legacy current_dir()
            // location and none in the new app_local_data_dir, copy it over.
            if !db_path.exists() {
                if let Ok(cwd) = std::env::current_dir() {
                    let legacy = cwd.join("plants_calendar_local.sqlite");
                    if legacy.exists() {
                        let _ = std::fs::copy(&legacy, &db_path);
                        // Also migrate photos dir if present
                        let legacy_photos = cwd.join("photos");
                        let new_photos = base.join("photos");
                        if legacy_photos.is_dir() && !new_photos.join(".migrated").exists() {
                            let _ = copy_dir_all(&legacy_photos, &new_photos);
                            let _ = std::fs::write(new_photos.join(".migrated"), b"1");
                        }
                    }
                }
            }

            let _ = APP_DATA_DIR.set(base.clone());

            // A panic here closes the window instantly with no visible reason.
            // Surface the actual cause (most often: the SQLCipher key is gone
            // from the OS keychain) so the user can act on it.
            let store = match LocalStore::open(&db_path) {
                Ok(store) => store,
                Err(err) => {
                    let msg = format!(
                        "The local database could not be opened.\n\n{err}\n\nLocation:\n{}",
                        db_path.display()
                    );
                    eprintln!("[startup] {msg}");
                    use tauri::async_runtime::block_on;
                    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                    let _ = block_on(async {
                        app.dialog()
                            .message(&msg)
                            .kind(MessageDialogKind::Error)
                            .title("Plants Calendar — database error")
                            .blocking_show()
                    });
                    return Err(err.into());
                }
            };
            app.manage(AppLocalState { store: Mutex::new(store) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_plan_info,
            bootstrap_local_user,
            list_local_plants,
            create_local_plant,
            update_local_plant,
            delete_local_plant,
            list_local_tasks,
            create_local_task,
            update_local_task_status,
            update_local_task,
            delete_local_task,
            list_local_logs,
            create_local_log,
            update_local_log,
            delete_local_log,
            list_current_plants,
            create_plant_smart,
            update_plant_smart,
            delete_plant_smart,
            list_current_tasks,
            create_task_smart,
            update_task_smart,
            update_task_status_smart,
            delete_task_smart,
            list_current_logs,
            create_log_smart,
            update_log_smart,
            delete_log_smart,
            dirty_sync_count,
            export_sync_payload,
            get_last_sync_cursor,
            get_sync_status,
            get_dirty_records,
            list_due_notification_candidates,
            mark_notification_sent,
            check_and_send_notifications,
            get_stored_session,
            clear_stored_session,
            auth_register,
            auth_login,
            auth_fetch_me,
            subscription_status_smart,
            subscription_checkout_smart,
            subscription_webhook_stub_smart,
            subscription_history_smart,
            sync_push_now,
            sync_pull_now,
            sync_now,
            update_profile_smart,
            change_password_smart,
            delete_account_smart,
            ble_scan,
            ble_read_telemetry,
            ble_debug_read,
            lc_read_status,
            tuya_read_status,
            list_devices,
            save_device,
            link_device,
            remove_device,
            list_device_readings,
            save_device_reading,
            prune_old_readings,
            create_auto_log_from_reading,
            list_profiles,
            create_profile,
            update_profile_local,
            delete_profile_local,
            switch_profile,
            get_active_profile,
            assign_plant_to_profile,
            list_locations,
            create_location,
            update_location,
            delete_location,
            list_plant_seasons,
            create_plant_season,
            update_plant_season,
            delete_plant_season,
            start_new_season,
            add_photo,
            list_plant_photos,
            list_log_photos,
            delete_photo,
            get_photo_path,
            get_plant_catalog,
            get_companion_info,
            get_climate_config,
            save_climate_config,
            fetch_weather,
            search_city,
            fetch_and_log_weather,
            list_weather_history,
            prune_weather_history_cmd,
            export_all_csv,
            import_plants_csv,
            write_export_file,
            read_import_file,
            list_consents_smart,
            set_consent_smart,
            open_external_path,
            resolve_doc_path,
            export_account_json,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
