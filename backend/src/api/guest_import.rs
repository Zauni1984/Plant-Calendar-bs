//! Moving browser-guest data into an account.
//!
//! The web app can be used without an account; everything then lives in the
//! browser (IndexedDB). On signing in or registering, the browser sends that
//! data here once. Everything is written in one transaction: either the whole
//! garden arrives or nothing does, so a retry never leaves half a copy.
//!
//! Records get new server IDs (guest IDs are only unique within one browser)
//! and references are rewritten. Dates, statuses and history are kept.
//! Active plants beyond the account's plan limit are imported as "ended", so
//! nothing is lost; the user can reactivate them after upgrading.

use std::collections::HashMap;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::{
    LogEntry, LogType, Metrics, Plant, PlantLifecycle, PlantPhase, PlantStatus, Task, TaskCategory, TaskStatus,
};
use uuid::Uuid;

use super::{api_error, MAX_LOG_TEXT_LEN, MAX_REPEAT_INTERVAL_HOURS};
use crate::{
    db::{repos, AppState},
    services::auth::SessionUser,
};

/// A browser garden is small; this only stops absurd payloads.
const MAX_RECORDS: usize = 5000;

#[derive(Deserialize)]
pub struct ImportLocation {
    location_id: Uuid,
    name: String,
    #[serde(default)]
    location_type: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    latitude: Option<f64>,
    #[serde(default)]
    longitude: Option<f64>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct ImportPlant {
    plant_id: Uuid,
    name: String,
    color_tag: String,
    start_date: DateTime<Utc>,
    phase: PlantPhase,
    #[serde(default)]
    phase_week: Option<i32>,
    #[serde(default)]
    status: PlantStatus,
    #[serde(default)]
    location_id: Option<Uuid>,
    #[serde(default)]
    lifecycle: PlantLifecycle,
    #[serde(default)]
    plant_year: Option<i32>,
    #[serde(default)]
    catalog_id: Option<String>,
    #[serde(default)]
    pot_size_liters: Option<f32>,
    #[serde(default)]
    soil_type: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct ImportTask {
    plant_id: Uuid,
    title: String,
    category: TaskCategory,
    due_at: DateTime<Utc>,
    #[serde(default)]
    repeat_interval_hours: Option<i32>,
    #[serde(default)]
    repeat_until: Option<DateTime<Utc>>,
    notification_enabled: bool,
    status: TaskStatus,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct ImportLog {
    plant_id: Uuid,
    log_type: LogType,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    metrics: Option<Metrics>,
    created_at: DateTime<Utc>,
    #[serde(default)]
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct ImportSeason {
    plant_id: Uuid,
    year: i32,
    #[serde(default)]
    started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    ended_at: Option<DateTime<Utc>>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    harvest_kg: Option<f32>,
    #[serde(default)]
    harvest_count: Option<i32>,
    #[serde(default)]
    pruning_done: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct GuestImport {
    #[serde(default)]
    locations: Vec<ImportLocation>,
    #[serde(default)]
    plants: Vec<ImportPlant>,
    #[serde(default)]
    tasks: Vec<ImportTask>,
    #[serde(default)]
    logs: Vec<ImportLog>,
    #[serde(default)]
    seasons: Vec<ImportSeason>,
}

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct ImportReport {
    pub locations: usize,
    pub plants: usize,
    pub tasks: usize,
    pub logs: usize,
    pub seasons: usize,
    /// Active guest plants that did not fit the plan and arrived as "ended".
    pub plants_ended_for_limit: usize,
}

fn validate(body: &GuestImport) -> Result<(), String> {
    let total = body.locations.len() + body.plants.len() + body.tasks.len() + body.logs.len() + body.seasons.len();
    if total > MAX_RECORDS {
        return Err(format!("too many records ({total}, max {MAX_RECORDS})"));
    }
    let bad_len = |s: &str, max: usize| s.trim().is_empty() || s.chars().count() > max;
    if body.locations.iter().any(|l| bad_len(&l.name, 120)) {
        return Err("location name must be 1-120 characters".into());
    }
    for l in &body.locations {
        repos::Coords::parse(l.latitude, l.longitude).map_err(str::to_string)?;
    }
    if body.plants.iter().any(|p| bad_len(&p.name, 120) || bad_len(&p.color_tag, 30)) {
        return Err("plant name must be 1-120 characters, color tag 1-30".into());
    }
    for t in &body.tasks {
        if bad_len(&t.title, 120) {
            return Err("task title must be 1-120 characters".into());
        }
        if let Some(h) = t.repeat_interval_hours {
            if h <= 0 || h > MAX_REPEAT_INTERVAL_HOURS {
                return Err(format!("repeat_interval_hours must be 1-{MAX_REPEAT_INTERVAL_HOURS}"));
            }
        }
    }
    if body.logs.iter().any(|l| l.text.as_ref().is_some_and(|t| t.len() > MAX_LOG_TEXT_LEN)) {
        return Err(format!("log text must be at most {MAX_LOG_TEXT_LEN} characters"));
    }
    if body.seasons.iter().any(|s| s.notes.as_ref().is_some_and(|n| n.len() > MAX_LOG_TEXT_LEN)) {
        return Err("season notes too long".into());
    }
    Ok(())
}

/// The status each imported plant gets: active ones only while the plan has
/// room (counting the account's existing active plants), the rest "ended".
fn assign_statuses(requested: &[PlantStatus], existing_active: usize, limit: usize) -> (Vec<PlantStatus>, usize) {
    let mut active = existing_active;
    let mut demoted = 0;
    let statuses = requested
        .iter()
        .map(|status| match status {
            PlantStatus::Active if active < limit => {
                active += 1;
                PlantStatus::Active
            }
            PlantStatus::Active => {
                demoted += 1;
                PlantStatus::Ended
            }
            other => *other,
        })
        .collect();
    (statuses, demoted)
}

pub async fn import_guest_data(
    State(state): State<AppState>,
    session: SessionUser,
    Json(body): Json<GuestImport>,
) -> impl IntoResponse {
    if let Err(msg) = validate(&body) {
        return api_error(StatusCode::BAD_REQUEST, msg);
    }
    let user = match repos::get_or_create_user(&state, session.user_id, session.auth_mode).await {
        Ok(u) => u,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let existing = match repos::list_plants(&state, user.user_id).await {
        Ok(p) => p,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let existing_active = shared::active_plant_count(&existing);

    match write_import(&state, user.user_id, user.plan.plant_limit(), existing_active, body).await {
        Ok(report) => Json(report).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

async fn write_import(
    state: &AppState,
    user_id: Uuid,
    limit: usize,
    existing_active: usize,
    body: GuestImport,
) -> anyhow::Result<ImportReport> {
    let mut report = ImportReport::default();
    let mut tx = state.pg.begin().await?;
    let now = Utc::now();

    let mut location_ids: HashMap<Uuid, Uuid> = HashMap::new();
    for loc in body.locations {
        let coords = repos::Coords::parse(loc.latitude, loc.longitude).map_err(anyhow::Error::msg)?;
        let new_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO locations (location_id, user_id, name, location_type, description, latitude, longitude, created_at, updated_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(new_id)
        .bind(user_id)
        .bind(loc.name.trim())
        .bind(loc.location_type.unwrap_or_else(|| "custom".into()))
        .bind(loc.description)
        .bind(coords.map(|c| c.latitude))
        .bind(coords.map(|c| c.longitude))
        .bind(loc.created_at.unwrap_or(now))
        .bind(now)
        .execute(&mut *tx)
        .await?;
        location_ids.insert(loc.location_id, new_id);
        report.locations += 1;
    }

    let plants: Vec<ImportPlant> = body.plants.into_iter().filter(|p| p.status != PlantStatus::Deleted).collect();
    let (statuses, demoted) = assign_statuses(
        &plants.iter().map(|p| p.status).collect::<Vec<_>>(),
        existing_active,
        limit,
    );
    report.plants_ended_for_limit = demoted;
    let mut plant_ids: HashMap<Uuid, Uuid> = HashMap::new();
    for (p, status) in plants.into_iter().zip(statuses) {
        let new_id = Uuid::new_v4();
        let mut plant = Plant {
            plant_id: new_id,
            user_id,
            name: p.name.trim().to_string(),
            color_tag: p.color_tag,
            start_date: p.start_date,
            phase: p.phase,
            phase_week: p.phase_week,
            status,
            is_active: false,
            archived: false,
            location_id: p.location_id.and_then(|id| location_ids.get(&id).copied()),
            lifecycle: p.lifecycle,
            plant_year: p.plant_year,
            catalog_id: p.catalog_id,
            pot_size_liters: p.pot_size_liters,
            soil_type: p.soil_type,
            created_at: p.created_at,
            // "Now", so the imported plants count as the newest change.
            updated_at: now.max(p.updated_at),
        };
        plant.sync_legacy_flags();
        repos::upsert_plant_tx(&mut tx, &plant).await?;
        plant_ids.insert(p.plant_id, new_id);
        report.plants += 1;
    }

    for t in body.tasks {
        let Some(&plant_id) = plant_ids.get(&t.plant_id) else { continue };
        let task = Task {
            task_id: Uuid::new_v4(),
            plant_id,
            title: t.title.trim().to_string(),
            category: t.category,
            due_at: t.due_at,
            repeat_interval_hours: t.repeat_interval_hours,
            repeat_until: t.repeat_until,
            notification_enabled: t.notification_enabled,
            status: t.status,
            created_at: t.created_at,
            updated_at: now.max(t.updated_at),
        };
        repos::upsert_task_tx(&mut tx, &task).await?;
        report.tasks += 1;
    }

    for l in body.logs {
        let Some(&plant_id) = plant_ids.get(&l.plant_id) else { continue };
        let log = LogEntry {
            log_id: Uuid::new_v4(),
            plant_id,
            log_type: l.log_type,
            text: l.text,
            metrics: l.metrics,
            action: None,
            photo_refs: None,
            created_at: l.created_at,
            updated_at: Some(now.max(l.updated_at.unwrap_or(l.created_at))),
        };
        repos::upsert_log_tx(&mut tx, &log).await?;
        report.logs += 1;
    }

    for s in body.seasons {
        let Some(&plant_id) = plant_ids.get(&s.plant_id) else { continue };
        sqlx::query(
            "INSERT INTO plant_seasons (season_id, plant_id, year, started_at, ended_at, notes, harvest_kg, harvest_count, pruning_done, created_at, updated_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(Uuid::new_v4())
        .bind(plant_id)
        .bind(s.year)
        .bind(s.started_at)
        .bind(s.ended_at)
        .bind(s.notes)
        .bind(s.harvest_kg)
        .bind(s.harvest_count)
        .bind(s.pruning_done)
        .bind(s.created_at)
        .bind(now.max(s.updated_at))
        .execute(&mut *tx)
        .await?;
        report.seasons += 1;
    }

    tx.commit().await?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_plants_beyond_the_limit_arrive_ended() {
        use PlantStatus::*;
        let (s, demoted) = assign_statuses(&[Active, Ended, Active, Archived], 0, 1);
        assert_eq!(s, vec![Active, Ended, Ended, Archived]);
        assert_eq!(demoted, 1);
        // The account's own active plant already uses the only Free slot.
        let (s, demoted) = assign_statuses(&[Active], 1, 1);
        assert_eq!((s, demoted), (vec![Ended], 1));
        let (s, demoted) = assign_statuses(&[Active, Active], 0, usize::MAX);
        assert_eq!((s, demoted), (vec![Active, Active], 0));
    }

    #[test]
    fn payload_limits_are_enforced() {
        let json = |plants: &str| serde_json::from_str::<GuestImport>(&format!("{{\"plants\":[{plants}]}}")).unwrap();
        let plant = |name: &str| format!(
            "{{\"plant_id\":\"{}\",\"name\":\"{name}\",\"color_tag\":\"#0B3D2E\",\"start_date\":\"2026-01-01T00:00:00Z\",\
              \"phase\":\"veg\",\"created_at\":\"2026-01-01T00:00:00Z\",\"updated_at\":\"2026-01-01T00:00:00Z\"}}",
            Uuid::new_v4()
        );
        assert!(validate(&json(&plant("Basil"))).is_ok());
        assert!(validate(&json(&plant(" "))).is_err());
        assert!(validate(&json(&plant(&"x".repeat(121)))).is_err());
    }
}
