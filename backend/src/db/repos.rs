use chrono::{DateTime, Utc};
use shared::{
    ActionInfo, AuthMode, Location, LogEntry, LogType, Metrics, Plant, PlantLifecycle, PlantPhase,
    PlantSeason, PlantStatus, SubscriptionPlan, Task, TaskCategory, TaskStatus, UserProfile,
};
use uuid::Uuid;

use super::{models::*, AppState};

fn parse_auth_mode(value: &str) -> AuthMode {
    match value {
        "email" => AuthMode::Email,
        "google" => AuthMode::Google,
        _ => AuthMode::Anonymous,
    }
}

fn auth_mode_str(value: &AuthMode) -> &'static str {
    match value {
        AuthMode::Anonymous => "anonymous",
        AuthMode::Email => "email",
        AuthMode::Google => "google",
    }
}

fn parse_plan(value: &str) -> SubscriptionPlan {
    match value {
        "basic" => SubscriptionPlan::Basic,
        "pro" => SubscriptionPlan::Pro,
        "csc" => SubscriptionPlan::Csc,
        _ => SubscriptionPlan::Free,
    }
}

fn plan_str(value: SubscriptionPlan) -> &'static str {
    match value {
        SubscriptionPlan::Free => "free",
        SubscriptionPlan::Basic => "basic",
        SubscriptionPlan::Pro => "pro",
        SubscriptionPlan::Csc => "csc",
    }
}

fn parse_phase(value: &str) -> PlantPhase {
    match value {
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

fn phase_str(value: PlantPhase) -> &'static str {
    match value {
        PlantPhase::Seed => "seed",
        PlantPhase::Veg => "veg",
        PlantPhase::Flower => "flower",
        PlantPhase::Harvest => "harvest",
        PlantPhase::Dry => "dry",
        PlantPhase::Cure => "cure",
        PlantPhase::Custom => "custom",
        PlantPhase::Dormant => "dormant",
    }
}

fn parse_lifecycle(value: Option<&str>) -> PlantLifecycle {
    match value.unwrap_or("annual") {
        "perennial" => PlantLifecycle::Perennial,
        _ => PlantLifecycle::Annual,
    }
}

fn lifecycle_str(value: PlantLifecycle) -> &'static str {
    match value {
        PlantLifecycle::Annual => "annual",
        PlantLifecycle::Perennial => "perennial",
    }
}

fn parse_plant_status(value: &str, is_active: bool, archived: bool) -> PlantStatus {
    PlantStatus::from_storage_value(value, is_active, archived)
}

fn plant_status_str(value: PlantStatus) -> &'static str {
    value.as_str()
}

fn normalize_plant_status(current: &Plant, requested_status: Option<PlantStatus>) -> PlantStatus {
    shared::resolve_requested_plant_status(current.status, requested_status, None, None)
}

fn parse_category(value: &str) -> TaskCategory {
    match value {
        "feed" => TaskCategory::Feed,
        "check" => TaskCategory::Check,
        "train" => TaskCategory::Train,
        "note" => TaskCategory::Note,
        _ => TaskCategory::Water,
    }
}

fn category_str(value: TaskCategory) -> &'static str {
    match value {
        TaskCategory::Water => "water",
        TaskCategory::Feed => "feed",
        TaskCategory::Check => "check",
        TaskCategory::Train => "train",
        TaskCategory::Note => "note",
    }
}

fn parse_status(value: &str) -> TaskStatus {
    match value {
        "done" => TaskStatus::Done,
        "skipped" => TaskStatus::Skipped,
        _ => TaskStatus::Open,
    }
}

fn status_str(value: TaskStatus) -> &'static str {
    match value {
        TaskStatus::Open => "open",
        TaskStatus::Done => "done",
        TaskStatus::Skipped => "skipped",
    }
}

fn parse_log_type(value: &str) -> LogType {
    match value {
        "measurement" => LogType::Measurement,
        "photo" => LogType::Photo,
        "action" => LogType::Action,
        "harvest" => LogType::Harvest,
        "watering" => LogType::Watering,
        _ => LogType::Note,
    }
}

fn log_type_str(value: LogType) -> &'static str {
    match value {
        LogType::Note => "note",
        LogType::Measurement => "measurement",
        LogType::Photo => "photo",
        LogType::Action => "action",
        LogType::Harvest => "harvest",
        LogType::Watering => "watering",
    }
}

pub fn user_from_row(row: UserRow) -> UserProfile {
    UserProfile {
        user_id: row.user_id,
        auth_mode: parse_auth_mode(&row.auth_mode),
        adult_confirmed: row.adult_confirmed,
        locale: row.locale,
        timezone: row.timezone,
        plan: parse_plan(&row.plan),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn plant_from_row(row: PlantRow) -> Plant {
    Plant {
        plant_id: row.plant_id,
        user_id: row.user_id,
        name: row.name,
        color_tag: row.color_tag,
        start_date: row.start_date,
        phase: parse_phase(&row.phase),
        phase_week: row.phase_week,
        status: parse_plant_status(&row.plant_status, row.is_active, row.archived),
        is_active: row.is_active,
        archived: row.archived,
        location_id: row.location_id,
        lifecycle: parse_lifecycle(row.lifecycle.as_deref()),
        plant_year: row.plant_year,
        catalog_id: row.catalog_id,
        pot_size_liters: row.pot_size_liters,
        soil_type: row.soil_type,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
    .normalized()
}

pub fn season_from_row(row: PlantSeasonRow) -> PlantSeason {
    PlantSeason {
        season_id: row.season_id,
        plant_id: row.plant_id,
        year: row.year,
        started_at: row.started_at,
        ended_at: row.ended_at,
        notes: row.notes,
        harvest_kg: row.harvest_kg,
        harvest_count: row.harvest_count,
        pruning_done: row.pruning_done,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn location_from_row(row: LocationRow) -> Location {
    Location {
        location_id: row.location_id,
        user_id: row.user_id,
        name: row.name,
        location_type: row.location_type,
        description: row.description,
        latitude: row.latitude,
        longitude: row.longitude,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn task_from_row(row: TaskRow) -> Task {
    Task {
        task_id: row.task_id,
        plant_id: row.plant_id,
        title: row.title,
        category: parse_category(&row.category),
        due_at: row.due_at,
        repeat_interval_hours: row.repeat_interval_hours,
        repeat_until: row.repeat_until,
        notification_enabled: row.notification_enabled,
        status: parse_status(&row.status),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn log_from_row(row: LogRow) -> LogEntry {
    LogEntry {
        log_id: row.log_id,
        plant_id: row.plant_id,
        log_type: parse_log_type(&row.log_type),
        text: row.text,
        metrics: row.metrics_json.and_then(|v| serde_json::from_value::<Metrics>(v).ok()),
        action: row.action_json.and_then(|v| serde_json::from_value::<ActionInfo>(v).ok()),
        photo_refs: row.photo_refs_json.and_then(|v| serde_json::from_value::<Vec<String>>(v).ok()),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub async fn get_user(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<UserProfile>> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT user_id, auth_mode, adult_confirmed, email, password_hash, locale, timezone, plan, created_at, updated_at FROM users WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pg)
    .await?;
    Ok(row.map(user_from_row))
}

pub async fn get_or_create_user(state: &AppState, user_id: Uuid, auth_mode: AuthMode) -> anyhow::Result<UserProfile> {
    if let Some(user) = get_user(state, user_id).await? {
        return Ok(user);
    }
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO users (user_id, auth_mode, adult_confirmed, email, password_hash, locale, timezone, plan, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(user_id)
    .bind(auth_mode_str(&auth_mode))
    .bind(false)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind("de-DE")
    .bind("Europe/Berlin")
    .bind("free")
    .bind(now)
    .bind(now)
    .execute(&state.pg)
    .await?;

    Ok(UserProfile {
        user_id,
        auth_mode,
        adult_confirmed: false,
        locale: "de-DE".into(),
        timezone: "Europe/Berlin".into(),
        plan: SubscriptionPlan::Free,
        created_at: now,
        updated_at: now,
    })
}

pub async fn update_adult_confirmation(state: &AppState, user_id: Uuid, adult_confirmed: bool) -> anyhow::Result<UserProfile> {
    sqlx::query("UPDATE users SET adult_confirmed = $2, updated_at = $3 WHERE user_id = $1")
        .bind(user_id)
        .bind(adult_confirmed)
        .bind(Utc::now())
        .execute(&state.pg)
        .await?;
    get_user(state, user_id).await?.ok_or_else(|| anyhow::anyhow!("user not found"))
}

pub async fn upsert_user_plan(state: &AppState, user_id: Uuid, plan: SubscriptionPlan) -> anyhow::Result<()> {
    // Every plan change goes through here, so this is where the photo
    // retention clock is kept (see api::photos). In an UPDATE the right-hand
    // `plan` is the OLD value:
    //   paid -> free : start the clock
    //   free -> free : keep whatever is there
    //   -> any paid  : stop it
    sqlx::query(
        "UPDATE users SET \
            paid_plan_ended_at = CASE \
                WHEN $2 = 'free' AND plan <> 'free' THEN NOW() \
                WHEN $2 = 'free' THEN paid_plan_ended_at \
                ELSE NULL END, \
            plan = $2, updated_at = $3 \
         WHERE user_id = $1",
    )
        .bind(user_id)
        .bind(plan_str(plan))
        .bind(Utc::now())
        .execute(&state.pg)
        .await?;
    Ok(())
}

pub async fn active_plant_count(state: &AppState, user_id: Uuid) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM plants WHERE user_id = $1 AND plant_status = 'active'",
    )
    .bind(user_id)
    .fetch_one(&state.pg)
    .await?;
    Ok(count)
}


pub async fn subscription_status_summary(state: &AppState, user_id: Uuid, plan: SubscriptionPlan) -> anyhow::Result<shared::SubscriptionStatusSummary> {
    let plants = list_plants(state, user_id).await?;
    Ok(shared::subscription_status_summary(&plants, plan))
}

pub async fn list_plants(state: &AppState, user_id: Uuid) -> anyhow::Result<Vec<Plant>> {
    let rows = sqlx::query_as::<_, PlantRow>(
        "SELECT plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type, created_at, updated_at
         FROM plants WHERE user_id = $1 AND plant_status != 'deleted' ORDER BY updated_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.pg)
    .await?;
    Ok(rows.into_iter().map(plant_from_row).collect())
}

pub async fn get_plant(state: &AppState, user_id: Uuid, plant_id: Uuid) -> anyhow::Result<Option<Plant>> {
    let row = sqlx::query_as::<_, PlantRow>(
        "SELECT plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type, created_at, updated_at
         FROM plants WHERE user_id = $1 AND plant_id = $2 AND plant_status != 'deleted'",
    )
    .bind(user_id)
    .bind(plant_id)
    .fetch_optional(&state.pg)
    .await?;
    Ok(row.map(plant_from_row))
}

pub async fn insert_plant(state: &AppState, plant: &Plant) -> anyhow::Result<()> {
    let plant = plant.normalized_ref();
    sqlx::query(
        "INSERT INTO plants (plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, location_id, lifecycle, plant_year, catalog_id, created_at, updated_at, pot_size_liters, soil_type)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)",
    )
    .bind(plant.plant_id)
    .bind(plant.user_id)
    .bind(&plant.name)
    .bind(&plant.color_tag)
    .bind(plant.start_date)
    .bind(phase_str(plant.phase))
    .bind(plant.phase_week)
    .bind(plant_status_str(plant.status))
    .bind(plant.is_active_state())
    .bind(plant.is_archived_state())
    .bind(plant.location_id)
    .bind(lifecycle_str(plant.lifecycle))
    .bind(plant.plant_year)
    .bind(&plant.catalog_id)
    .bind(plant.created_at)
    .bind(plant.updated_at)
    .bind(plant.pot_size_liters)
    .bind(&plant.soil_type)
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn upsert_plant(state: &AppState, plant: &Plant) -> anyhow::Result<()> {
    let plant = plant.normalized_ref();
    sqlx::query(
        "INSERT INTO plants (plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, location_id, lifecycle, plant_year, catalog_id, created_at, updated_at, pot_size_liters, soil_type)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
         ON CONFLICT (plant_id) DO UPDATE SET
            user_id = EXCLUDED.user_id,
            name = EXCLUDED.name,
            color_tag = EXCLUDED.color_tag,
            start_date = EXCLUDED.start_date,
            phase = EXCLUDED.phase,
            phase_week = EXCLUDED.phase_week,
            plant_status = EXCLUDED.plant_status,
            is_active = EXCLUDED.is_active,
            archived = EXCLUDED.archived,
            location_id = EXCLUDED.location_id,
            lifecycle = EXCLUDED.lifecycle,
            plant_year = EXCLUDED.plant_year,
            catalog_id = EXCLUDED.catalog_id,
            pot_size_liters = EXCLUDED.pot_size_liters,
            soil_type = EXCLUDED.soil_type,
            updated_at = EXCLUDED.updated_at",
    )
    .bind(plant.plant_id)
    .bind(plant.user_id)
    .bind(&plant.name)
    .bind(&plant.color_tag)
    .bind(plant.start_date)
    .bind(phase_str(plant.phase))
    .bind(plant.phase_week)
    .bind(plant_status_str(plant.status))
    .bind(plant.is_active_state())
    .bind(plant.is_archived_state())
    .bind(plant.location_id)
    .bind(lifecycle_str(plant.lifecycle))
    .bind(plant.plant_year)
    .bind(&plant.catalog_id)
    .bind(plant.created_at)
    .bind(plant.updated_at)
    .bind(plant.pot_size_liters)
    .bind(&plant.soil_type)
    .execute(&state.pg)
    .await?;
    Ok(())
}

/// Optional plant attributes an update may carry. `None` leaves the stored
/// value untouched, so a client that omits a field never clears it.
#[derive(Debug, Default, Clone)]
pub struct PlantUpdateExtras {
    pub phase_week: Option<i32>,
    pub location_id: Option<Uuid>,
    pub lifecycle: Option<shared::PlantLifecycle>,
    pub plant_year: Option<i32>,
    pub pot_size_liters: Option<f32>,
    pub soil_type: Option<String>,
}

pub async fn update_plant(state: &AppState, user_id: Uuid, plant_id: Uuid, name: Option<String>, color_tag: Option<String>, phase: Option<PlantPhase>, status: Option<PlantStatus>, extras: PlantUpdateExtras) -> anyhow::Result<Plant> {
    let current = get_plant(state, user_id, plant_id).await?.ok_or_else(|| anyhow::anyhow!("plant not found"))?;
    let updated_status = normalize_plant_status(&current, status);
    let updated_name = name.unwrap_or(current.name);
    let updated_color = color_tag.unwrap_or(current.color_tag);
    let updated_phase = phase.unwrap_or(current.phase);
    let updated_archived = updated_status == PlantStatus::Archived;
    let updated_is_active = updated_status == PlantStatus::Active;
    let updated_phase_week = extras.phase_week.or(current.phase_week);
    let updated_location = extras.location_id.or(current.location_id);
    let updated_lifecycle = extras.lifecycle.unwrap_or(current.lifecycle);
    let updated_plant_year = extras.plant_year.or(current.plant_year);
    let updated_pot_size = extras.pot_size_liters.or(current.pot_size_liters);
    let updated_soil = extras.soil_type.or(current.soil_type);
    let now = Utc::now();
    sqlx::query(
        "UPDATE plants SET name = $3, color_tag = $4, phase = $5, plant_status = $6, is_active = $7, archived = $8,          phase_week = $9, location_id = $10, lifecycle = $11, plant_year = $12, pot_size_liters = $13, soil_type = $14,          updated_at = $15 WHERE user_id = $1 AND plant_id = $2",
    )
    .bind(user_id)
    .bind(plant_id)
    .bind(updated_name)
    .bind(updated_color)
    .bind(phase_str(updated_phase))
    .bind(plant_status_str(updated_status))
    .bind(updated_is_active)
    .bind(updated_archived)
    .bind(updated_phase_week)
    .bind(updated_location)
    .bind(lifecycle_str(updated_lifecycle))
    .bind(updated_plant_year)
    .bind(updated_pot_size)
    .bind(updated_soil)
    .bind(now)
    .execute(&state.pg)
    .await?;
    get_plant(state, user_id, plant_id).await?.ok_or_else(|| anyhow::anyhow!("plant not found"))
}

pub async fn delete_plant(state: &AppState, user_id: Uuid, plant_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE plants SET plant_status = 'deleted', is_active = FALSE, archived = FALSE, updated_at = $3 WHERE user_id = $1 AND plant_id = $2")
        .bind(user_id)
        .bind(plant_id)
        .bind(Utc::now())
        .execute(&state.pg)
        .await?;
    record_delete(state, user_id, plant_id, "plant").await?;
    Ok(())
}

pub async fn list_tasks_for_plant(state: &AppState, plant_id: Uuid) -> anyhow::Result<Vec<Task>> {
    let rows = sqlx::query_as::<_, TaskRow>(
        "SELECT task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at
         FROM tasks WHERE plant_id = $1 ORDER BY due_at ASC",
    )
    .bind(plant_id)
    .fetch_all(&state.pg)
    .await?;
    Ok(rows.into_iter().map(task_from_row).collect())
}

pub async fn list_tasks_for_user_since(state: &AppState, user_id: Uuid, since: Option<DateTime<Utc>>) -> anyhow::Result<Vec<Task>> {
    let rows = if let Some(since) = since {
        sqlx::query_as::<_, TaskRow>(
            "SELECT t.task_id, t.plant_id, t.title, t.category, t.due_at, t.repeat_interval_hours, t.repeat_until, t.notification_enabled, t.status, t.created_at, t.updated_at
             FROM tasks t INNER JOIN plants p ON p.plant_id = t.plant_id
             WHERE p.user_id = $1 AND t.updated_at > $2 ORDER BY t.updated_at DESC"
        ).bind(user_id).bind(since).fetch_all(&state.pg).await?
    } else {
        sqlx::query_as::<_, TaskRow>(
            "SELECT t.task_id, t.plant_id, t.title, t.category, t.due_at, t.repeat_interval_hours, t.repeat_until, t.notification_enabled, t.status, t.created_at, t.updated_at
             FROM tasks t INNER JOIN plants p ON p.plant_id = t.plant_id
             WHERE p.user_id = $1 ORDER BY t.updated_at DESC"
        ).bind(user_id).fetch_all(&state.pg).await?
    };
    Ok(rows.into_iter().map(task_from_row).collect())
}

pub async fn get_task(state: &AppState, task_id: Uuid) -> anyhow::Result<Option<Task>> {
    let row = sqlx::query_as::<_, TaskRow>(
        "SELECT task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at FROM tasks WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_optional(&state.pg)
    .await?;

    Ok(row.map(|row| Task {
        task_id: row.task_id,
        plant_id: row.plant_id,
        title: row.title,
        category: parse_category(&row.category),
        due_at: row.due_at,
        repeat_interval_hours: row.repeat_interval_hours,
        repeat_until: row.repeat_until,
        notification_enabled: row.notification_enabled,
        status: parse_status(&row.status),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

pub async fn insert_task(state: &AppState, task: &Task) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO tasks (task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
    )
    .bind(task.task_id)
    .bind(task.plant_id)
    .bind(&task.title)
    .bind(category_str(task.category))
    .bind(task.due_at)
    .bind(task.repeat_interval_hours)
    .bind(task.repeat_until)
    .bind(task.notification_enabled)
    .bind(status_str(task.status))
    .bind(task.created_at)
    .bind(task.updated_at)
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn upsert_task(state: &AppState, task: &Task) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO tasks (task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
         ON CONFLICT (task_id) DO UPDATE SET
            plant_id = EXCLUDED.plant_id,
            title = EXCLUDED.title,
            category = EXCLUDED.category,
            due_at = EXCLUDED.due_at,
            repeat_interval_hours = EXCLUDED.repeat_interval_hours,
            repeat_until = EXCLUDED.repeat_until,
            notification_enabled = EXCLUDED.notification_enabled,
            status = EXCLUDED.status,
            updated_at = EXCLUDED.updated_at"
    )
    .bind(task.task_id)
    .bind(task.plant_id)
    .bind(&task.title)
    .bind(category_str(task.category))
    .bind(task.due_at)
    .bind(task.repeat_interval_hours)
    .bind(task.repeat_until)
    .bind(task.notification_enabled)
    .bind(status_str(task.status))
    .bind(task.created_at)
    .bind(task.updated_at)
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn update_task(state: &AppState, task_id: Uuid, title: Option<String>, status: Option<TaskStatus>, due_at: Option<DateTime<Utc>>, repeat_interval_hours: Option<i32>, repeat_until: Option<DateTime<Utc>>, notification_enabled: Option<bool>) -> anyhow::Result<Task> {
    let row = sqlx::query_as::<_, TaskRow>(
        "SELECT task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at FROM tasks WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_one(&state.pg)
    .await?;
    let next_title = title.unwrap_or(row.title.clone());
    let next_status = status.unwrap_or(parse_status(&row.status));
    let next_due_at = due_at.unwrap_or(row.due_at);
    let next_repeat = repeat_interval_hours.or(row.repeat_interval_hours);
    let next_repeat_until = repeat_until.or(row.repeat_until);
    let next_notification = notification_enabled.unwrap_or(row.notification_enabled);
    let updated_at = Utc::now();
    sqlx::query("UPDATE tasks SET title = $2, status = $3, due_at = $4, repeat_interval_hours = $5, repeat_until = $6, notification_enabled = $7, updated_at = $8 WHERE task_id = $1")
        .bind(task_id)
        .bind(&next_title)
        .bind(status_str(next_status))
        .bind(next_due_at)
        .bind(next_repeat)
        .bind(next_repeat_until)
        .bind(next_notification)
        .bind(updated_at)
        .execute(&state.pg)
        .await?;
    Ok(Task {
        task_id: row.task_id,
        plant_id: row.plant_id,
        title: next_title,
        category: parse_category(&row.category),
        due_at: next_due_at,
        repeat_interval_hours: next_repeat,
        repeat_until: next_repeat_until,
        notification_enabled: next_notification,
        status: next_status,
        created_at: row.created_at,
        updated_at,
    })
}

pub async fn delete_task(state: &AppState, user_id: Uuid, task_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM tasks USING plants WHERE tasks.task_id = $1 AND tasks.plant_id = plants.plant_id AND plants.user_id = $2")
        .bind(task_id)
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    record_delete(state, user_id, task_id, "task").await?;
    Ok(())
}

pub async fn list_logs_for_plant(state: &AppState, plant_id: Uuid) -> anyhow::Result<Vec<LogEntry>> {
    let rows = sqlx::query_as::<_, LogRow>(
        "SELECT log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at
         FROM logs WHERE plant_id = $1 ORDER BY created_at DESC",
    )
    .bind(plant_id)
    .fetch_all(&state.pg)
    .await?;
    Ok(rows.into_iter().map(log_from_row).collect())
}

pub async fn list_logs_for_user_since(state: &AppState, user_id: Uuid, since: Option<DateTime<Utc>>) -> anyhow::Result<Vec<LogEntry>> {
    let rows = if let Some(since) = since {
        sqlx::query_as::<_, LogRow>(
            "SELECT l.log_id, l.plant_id, l.log_type, l.text, l.metrics_json, l.action_json, l.photo_refs_json, l.created_at, l.updated_at
             FROM logs l INNER JOIN plants p ON p.plant_id = l.plant_id
             WHERE p.user_id = $1 AND l.created_at > $2 ORDER BY l.created_at DESC"
        ).bind(user_id).bind(since).fetch_all(&state.pg).await?
    } else {
        sqlx::query_as::<_, LogRow>(
            "SELECT l.log_id, l.plant_id, l.log_type, l.text, l.metrics_json, l.action_json, l.photo_refs_json, l.created_at, l.updated_at
             FROM logs l INNER JOIN plants p ON p.plant_id = l.plant_id
             WHERE p.user_id = $1 ORDER BY l.created_at DESC"
        ).bind(user_id).fetch_all(&state.pg).await?
    };
    Ok(rows.into_iter().map(log_from_row).collect())
}

pub async fn insert_log(state: &AppState, log: &LogEntry) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO logs (log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(log.log_id)
    .bind(log.plant_id)
    .bind(log_type_str(log.log_type))
    .bind(&log.text)
    .bind(log.metrics.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.action.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.photo_refs.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.created_at)
    .bind(log.last_modified())
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn upsert_log(state: &AppState, log: &LogEntry) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO logs (log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         ON CONFLICT (log_id) DO UPDATE SET
            plant_id = EXCLUDED.plant_id,
            log_type = EXCLUDED.log_type,
            text = EXCLUDED.text,
            metrics_json = EXCLUDED.metrics_json,
            action_json = EXCLUDED.action_json,
            photo_refs_json = EXCLUDED.photo_refs_json,
            created_at = EXCLUDED.created_at,
            updated_at = EXCLUDED.updated_at"
    )
    .bind(log.log_id)
    .bind(log.plant_id)
    .bind(log_type_str(log.log_type))
    .bind(&log.text)
    .bind(log.metrics.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.action.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.photo_refs.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.created_at)
    .bind(log.last_modified())
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn update_log(state: &AppState, log_id: Uuid, log_type: Option<LogType>, text: Option<String>, metrics: Option<Metrics>, action: Option<serde_json::Value>, photo_refs: Option<Vec<String>>) -> anyhow::Result<LogEntry> {
    let row = sqlx::query_as::<_, LogRow>(
        "SELECT log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at FROM logs WHERE log_id = $1",
    )
    .bind(log_id)
    .fetch_one(&state.pg)
    .await?;
    let next_log_type = log_type.unwrap_or(parse_log_type(&row.log_type));
    let next_text = text.or(row.text.clone());
    let next_metrics = metrics.or_else(|| row.metrics_json.clone().and_then(|v| serde_json::from_value::<Metrics>(v).ok()));
    let next_action_json = action.or_else(|| row.action_json.clone());
    let next_photo_refs = photo_refs.or_else(|| row.photo_refs_json.clone().and_then(|v| serde_json::from_value::<Vec<String>>(v).ok()));

    let now = Utc::now();
    sqlx::query(
        "UPDATE logs SET log_type = $2, text = $3, metrics_json = $4, action_json = $5, photo_refs_json = $6, updated_at = $7 WHERE log_id = $1",
    )
    .bind(log_id)
    .bind(log_type_str(next_log_type))
    .bind(&next_text)
    .bind(next_metrics.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(next_action_json.clone())
    .bind(next_photo_refs.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(now)
    .execute(&state.pg)
    .await?;

    Ok(LogEntry {
        log_id: row.log_id,
        plant_id: row.plant_id,
        log_type: next_log_type,
        text: next_text,
        metrics: next_metrics,
        action: next_action_json.and_then(|v| serde_json::from_value(v).ok()),
        photo_refs: next_photo_refs,
        created_at: row.created_at,
        updated_at: Some(now),
    })
}

pub async fn delete_log(state: &AppState, user_id: Uuid, log_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM logs USING plants WHERE logs.log_id = $1 AND logs.plant_id = plants.plant_id AND plants.user_id = $2")
        .bind(log_id)
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    record_delete(state, user_id, log_id, "log").await?;
    Ok(())
}

pub async fn list_changes_since(state: &AppState, user_id: Uuid, since: Option<DateTime<Utc>>) -> anyhow::Result<(Vec<Plant>, Vec<Task>, Vec<LogEntry>)> {
    let plants = if let Some(since) = since {
        let rows = sqlx::query_as::<_, PlantRow>(
            "SELECT plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type, created_at, updated_at
             FROM plants WHERE user_id = $1 AND plant_status != 'deleted' AND updated_at > $2 ORDER BY updated_at DESC",
        ).bind(user_id).bind(since).fetch_all(&state.pg).await?;
        rows.into_iter().map(plant_from_row).collect()
    } else {
        list_plants(state, user_id).await?
    };
    let tasks = list_tasks_for_user_since(state, user_id, since).await?;
    let logs = list_logs_for_user_since(state, user_id, since).await?;
    Ok((plants, tasks, logs))
}

pub async fn merge_guest_data(state: &AppState, guest_user_id: Uuid, account_user_id: Uuid) -> anyhow::Result<()> {
    if guest_user_id == account_user_id {
        return Ok(());
    }

    // SECURITY: the caller supplies guest_user_id, so it must never be trusted.
    // Only an account that is still genuinely anonymous may be absorbed —
    // otherwise any authenticated user could pass a stranger's UUID and both
    // steal their plants and cascade-delete their account.
    let source_auth_mode: Option<String> =
        sqlx::query_scalar("SELECT auth_mode FROM users WHERE user_id = $1")
            .bind(guest_user_id)
            .fetch_optional(&state.pg)
            .await?;
    match source_auth_mode.as_deref() {
        Some("anonymous") => {}
        // Unknown user or a real (email/google) account: refuse silently-safely.
        _ => anyhow::bail!("merge source is not an anonymous guest account"),
    }

    let mut tx = state.pg.begin().await?;
    sqlx::query("UPDATE plants SET user_id = $2, updated_at = $3 WHERE user_id = $1")
        .bind(guest_user_id)
        .bind(account_user_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM users WHERE user_id = $1 AND auth_mode = 'anonymous'")
        .bind(guest_user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}


pub async fn record_delete(state: &AppState, user_id: Uuid, record_id: Uuid, entity_kind: &str) -> anyhow::Result<()> {
    sqlx::query(
        // Conflict target includes user_id, and user_id is never rewritten:
        // a tombstone belongs to exactly one user.
        "INSERT INTO deleted_records (record_id, user_id, entity_kind, operation, changed_at) VALUES ($1,$2,$3,'delete',$4)
         ON CONFLICT (user_id, record_id, entity_kind) DO UPDATE SET changed_at = EXCLUDED.changed_at"
    )
    .bind(record_id)
    .bind(user_id)
    .bind(entity_kind)
    .bind(Utc::now())
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn list_deleted_records_since(state: &AppState, user_id: Uuid, since: Option<DateTime<Utc>>) -> anyhow::Result<Vec<shared::DirtyRecord>> {
    let rows = if let Some(since) = since {
        sqlx::query_as::<_, DeletedRow>(
            "SELECT record_id, user_id, entity_kind, operation, changed_at FROM deleted_records WHERE user_id = $1 AND changed_at >= $2 ORDER BY changed_at ASC"
        ).bind(user_id).bind(since).fetch_all(&state.pg).await?
    } else {
        sqlx::query_as::<_, DeletedRow>(
            "SELECT record_id, user_id, entity_kind, operation, changed_at FROM deleted_records WHERE user_id = $1 ORDER BY changed_at ASC"
        ).bind(user_id).fetch_all(&state.pg).await?
    };
    Ok(rows.into_iter().map(|row| shared::DirtyRecord {
        record_id: row.record_id,
        entity_kind: match row.entity_kind.as_str() {
            "task" => shared::SyncEntityKind::Task,
            "log" => shared::SyncEntityKind::Log,
            _ => shared::SyncEntityKind::Plant,
        },
        operation: shared::SyncOperation::Delete,
        changed_at: row.changed_at,
    }).collect())
}

pub async fn enforce_user_plan_limit(state: &AppState, user_id: Uuid) -> anyhow::Result<()> {
    let user = get_user(state, user_id).await?.ok_or_else(|| anyhow::anyhow!("user not found"))?;
    let plants = list_plants(state, user_id).await?;
    let adjusted = crate::services::plants::apply_downgrade(plants, user.plan);
    let mut tx = state.pg.begin().await?;
    for plant in adjusted {
        let normalized = plant.normalized();
        sqlx::query("UPDATE plants SET plant_status = $2, is_active = $3, archived = $4, updated_at = $5 WHERE plant_id = $1")
            .bind(normalized.plant_id)
            .bind(plant_status_str(normalized.status))
            .bind(normalized.is_active)
            .bind(normalized.archived)
            .bind(Utc::now())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}


pub async fn record_subscription_history(
    state: &AppState,
    user_id: Uuid,
    event_kind: &str,
    plan: SubscriptionPlan,
    previous_plan: Option<SubscriptionPlan>,
    checkout_id: Option<&str>,
    provider: Option<&str>,
    checkout_status: Option<&str>,
    message: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO subscription_history (event_id, user_id, event_kind, plan, previous_plan, checkout_id, provider, checkout_status, message, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(event_kind)
    .bind(plan_str(plan))
    .bind(previous_plan.map(plan_str))
    .bind(checkout_id)
    .bind(provider)
    .bind(checkout_status)
    .bind(message)
    .bind(Utc::now())
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn list_subscription_history(state: &AppState, user_id: Uuid, limit: i64) -> anyhow::Result<Vec<SubscriptionHistoryRow>> {
    let rows = sqlx::query_as::<_, SubscriptionHistoryRow>(
        "SELECT event_id, user_id, event_kind, plan, previous_plan, checkout_id, provider, checkout_status, message, created_at
         FROM subscription_history WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2"
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(&state.pg)
    .await?;
    Ok(rows)
}

pub async fn get_user_by_email(state: &AppState, email: &str) -> anyhow::Result<Option<UserProfile>> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT user_id, auth_mode, adult_confirmed, email, password_hash, locale, timezone, plan, created_at, updated_at FROM users WHERE email = $1"
    )
    .bind(email)
    .fetch_optional(&state.pg)
    .await?;
    Ok(row.map(user_from_row))
}

// --- Refresh token repo functions ---

pub async fn store_refresh_token(state: &AppState, token_hash: &str, user_id: Uuid, expires_at: DateTime<Utc>) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO refresh_tokens (token_hash, user_id, expires_at) VALUES ($1, $2, $3)"
    )
    .bind(token_hash)
    .bind(user_id)
    .bind(expires_at)
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn find_refresh_token(state: &AppState, token_hash: &str) -> anyhow::Result<Option<super::models::RefreshTokenRow>> {
    let row = sqlx::query_as::<_, super::models::RefreshTokenRow>(
        "SELECT token_hash, user_id, expires_at, revoked_at, created_at FROM refresh_tokens WHERE token_hash = $1"
    )
    .bind(token_hash)
    .fetch_optional(&state.pg)
    .await?;
    Ok(row)
}

pub async fn revoke_refresh_token(state: &AppState, token_hash: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE refresh_tokens SET revoked_at = $2 WHERE token_hash = $1")
        .bind(token_hash)
        .bind(Utc::now())
        .execute(&state.pg)
        .await?;
    Ok(())
}

/// Revoke a refresh token that must belong to `user_id`.
/// Used by /auth/logout so a caller cannot revoke a stranger's token by hash.
pub async fn revoke_refresh_token_for_user(
    state: &AppState,
    token_hash: &str,
    user_id: Uuid,
) -> anyhow::Result<u64> {
    let result = sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = $3 WHERE token_hash = $1 AND user_id = $2",
    )
    .bind(token_hash)
    .bind(user_id)
    .bind(Utc::now())
    .execute(&state.pg)
    .await?;
    Ok(result.rows_affected())
}

pub async fn revoke_all_refresh_tokens(state: &AppState, user_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE refresh_tokens SET revoked_at = $2 WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .bind(Utc::now())
        .execute(&state.pg)
        .await?;
    Ok(())
}

// --- Settings / account repo functions ---

pub async fn update_user_profile(state: &AppState, user_id: Uuid, locale: Option<String>, timezone: Option<String>) -> anyhow::Result<UserProfile> {
    let current = get_user(state, user_id).await?.ok_or_else(|| anyhow::anyhow!("user not found"))?;
    let next_locale = locale.unwrap_or(current.locale);
    let next_timezone = timezone.unwrap_or(current.timezone);
    sqlx::query("UPDATE users SET locale = $2, timezone = $3, updated_at = $4 WHERE user_id = $1")
        .bind(user_id)
        .bind(&next_locale)
        .bind(&next_timezone)
        .bind(Utc::now())
        .execute(&state.pg)
        .await?;
    get_user(state, user_id).await?.ok_or_else(|| anyhow::anyhow!("user not found"))
}

pub async fn change_user_password(state: &AppState, user_id: Uuid, new_hash: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET password_hash = $2, updated_at = $3 WHERE user_id = $1")
        .bind(user_id)
        .bind(new_hash)
        .bind(Utc::now())
        .execute(&state.pg)
        .await?;
    Ok(())
}

// ── GDPR consent persistence (Phase C2) ──

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsentRecord {
    pub consent_id: Uuid,
    pub user_id: Uuid,
    pub consent_type: String,
    pub granted: bool,
    pub policy_version: String,
    pub granted_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

pub async fn list_user_consents(state: &AppState, user_id: Uuid) -> anyhow::Result<Vec<ConsentRecord>> {
    let rows: Vec<(Uuid, Uuid, String, bool, String, DateTime<Utc>, Option<DateTime<Utc>>, DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT consent_id, user_id, consent_type, granted, policy_version, granted_at, revoked_at, updated_at \
             FROM user_consents WHERE user_id = $1 ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .fetch_all(&state.pg)
        .await?;
    Ok(rows.into_iter().map(|(consent_id, user_id, consent_type, granted, policy_version, granted_at, revoked_at, updated_at)| ConsentRecord {
        consent_id, user_id, consent_type, granted, policy_version, granted_at, revoked_at, updated_at,
    }).collect())
}

pub async fn upsert_user_consent(state: &AppState, user_id: Uuid, consent_type: &str, granted: bool, policy_version: &str) -> anyhow::Result<ConsentRecord> {
    let now = Utc::now();
    // Try existing
    let existing: Option<(Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT consent_id, granted_at FROM user_consents WHERE user_id = $1 AND consent_type = $2 AND policy_version = $3"
    )
    .bind(user_id)
    .bind(consent_type)
    .bind(policy_version)
    .fetch_optional(&state.pg)
    .await?;
    let (consent_id, granted_at) = match existing {
        Some((id, prev)) => (id, prev),
        None => (Uuid::new_v4(), now),
    };
    let revoked_at = if granted { None } else { Some(now) };
    sqlx::query(
        "INSERT INTO user_consents (consent_id, user_id, consent_type, granted, policy_version, granted_at, revoked_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8) \
         ON CONFLICT (user_id, consent_type, policy_version) DO UPDATE SET \
           granted = EXCLUDED.granted, revoked_at = EXCLUDED.revoked_at, updated_at = EXCLUDED.updated_at",
    )
    .bind(consent_id)
    .bind(user_id)
    .bind(consent_type)
    .bind(granted)
    .bind(policy_version)
    .bind(granted_at)
    .bind(revoked_at)
    .bind(now)
    .execute(&state.pg)
    .await?;
    Ok(ConsentRecord {
        consent_id, user_id, consent_type: consent_type.to_string(), granted,
        policy_version: policy_version.to_string(), granted_at, revoked_at, updated_at: now,
    })
}

pub async fn delete_user_cascade(state: &AppState, user_id: Uuid) -> anyhow::Result<()> {
    // If the user has a Stripe customer, delete it first (Art. 17 GDPR –
    // complete erasure also at the processor). Failures are logged but do
    // not block local deletion.
    let stripe_customer: Option<(Option<String>,)> =
        sqlx::query_as("SELECT stripe_customer_id FROM users WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&state.pg)
            .await?;
    if let Some((Some(customer_id),)) = stripe_customer {
        if !customer_id.is_empty() {
            if let Some(stripe) = crate::services::stripe::StripeClient::from_env() {
                if let Err(err) = stripe.delete_customer(&customer_id).await {
                    tracing::warn!(
                        user_id = %user_id,
                        "stripe delete_customer failed during account erasure: {err}"
                    );
                }
            } else {
                tracing::warn!(
                    user_id = %user_id,
                    "STRIPE_SECRET_KEY not set; skipping Stripe customer deletion"
                );
            }
        }
    }

    // Delete plant_seasons (migration 012 already has ON DELETE CASCADE on
    // plants, but belt-and-suspenders – explicit delete before plants).
    sqlx::query("DELETE FROM plant_seasons WHERE plant_id IN (SELECT plant_id FROM plants WHERE user_id = $1)")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    // Delete logs for user's plants
    sqlx::query("DELETE FROM logs WHERE plant_id IN (SELECT plant_id FROM plants WHERE user_id = $1)")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    // Delete tasks for user's plants
    sqlx::query("DELETE FROM tasks WHERE plant_id IN (SELECT plant_id FROM plants WHERE user_id = $1)")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    // Delete plants
    sqlx::query("DELETE FROM plants WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    // Delete user-owned locations
    sqlx::query("DELETE FROM locations WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    // Delete refresh tokens
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    // Delete deleted_records
    sqlx::query("DELETE FROM deleted_records WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    // Delete subscription history
    sqlx::query("DELETE FROM subscription_history WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    // Delete consent records (Art. 17 GDPR). CASCADE on migration 021 also covers
    // this, but an explicit DELETE keeps erasure deterministic even on older DBs.
    sqlx::query("DELETE FROM user_consents WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pg)
        .await
        .ok();
    // Delete user
    sqlx::query("DELETE FROM users WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    Ok(())
}


// ── PlantSeason CRUD ──

pub async fn list_seasons_for_plant(state: &AppState, plant_id: Uuid) -> anyhow::Result<Vec<PlantSeason>> {
    let rows = sqlx::query_as::<_, PlantSeasonRow>(
        "SELECT season_id, plant_id, year, started_at, ended_at, notes, harvest_kg, harvest_count, pruning_done, created_at, updated_at
         FROM plant_seasons WHERE plant_id = $1 ORDER BY year DESC"
    )
    .bind(plant_id)
    .fetch_all(&state.pg)
    .await?;
    Ok(rows.into_iter().map(season_from_row).collect())
}

/// All seasons across a user's plants in one query.
/// /me/export previously issued one query per plant, so a large account turned
/// a single export into hundreds of round-trips.
pub async fn list_seasons_for_user(state: &AppState, user_id: Uuid) -> anyhow::Result<Vec<PlantSeason>> {
    let rows = sqlx::query_as::<_, PlantSeasonRow>(
        "SELECT ps.season_id, ps.plant_id, ps.year, ps.started_at, ps.ended_at, ps.notes,                 ps.harvest_kg, ps.harvest_count, ps.pruning_done, ps.created_at, ps.updated_at          FROM plant_seasons ps INNER JOIN plants p ON p.plant_id = ps.plant_id          WHERE p.user_id = $1 ORDER BY ps.year DESC"
    )
    .bind(user_id)
    .fetch_all(&state.pg)
    .await?;
    Ok(rows.into_iter().map(season_from_row).collect())
}

pub async fn get_season(state: &AppState, season_id: Uuid) -> anyhow::Result<Option<PlantSeason>> {
    let row = sqlx::query_as::<_, PlantSeasonRow>(
        "SELECT season_id, plant_id, year, started_at, ended_at, notes, harvest_kg, harvest_count, pruning_done, created_at, updated_at
         FROM plant_seasons WHERE season_id = $1"
    )
    .bind(season_id)
    .fetch_optional(&state.pg)
    .await?;
    Ok(row.map(season_from_row))
}

pub async fn insert_season(state: &AppState, season: &PlantSeason) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO plant_seasons (season_id, plant_id, year, started_at, ended_at, notes, harvest_kg, harvest_count, pruning_done, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"
    )
    .bind(season.season_id)
    .bind(season.plant_id)
    .bind(season.year)
    .bind(season.started_at)
    .bind(season.ended_at)
    .bind(&season.notes)
    .bind(season.harvest_kg)
    .bind(season.harvest_count)
    .bind(season.pruning_done)
    .bind(season.created_at)
    .bind(season.updated_at)
    .execute(&state.pg)
    .await?;
    Ok(())
}

pub async fn update_season(state: &AppState, season_id: Uuid, notes: Option<String>, harvest_kg: Option<f32>, harvest_count: Option<i32>, pruning_done: Option<bool>) -> anyhow::Result<PlantSeason> {
    let current = get_season(state, season_id).await?.ok_or_else(|| anyhow::anyhow!("season not found"))?;
    let next_notes = notes.or(current.notes);
    let next_kg = harvest_kg.or(current.harvest_kg);
    let next_count = harvest_count.or(current.harvest_count);
    let next_pruning = pruning_done.unwrap_or(current.pruning_done);
    let now = Utc::now();
    sqlx::query(
        "UPDATE plant_seasons SET notes = $2, harvest_kg = $3, harvest_count = $4, pruning_done = $5, updated_at = $6 WHERE season_id = $1"
    )
    .bind(season_id)
    .bind(&next_notes)
    .bind(next_kg)
    .bind(next_count)
    .bind(next_pruning)
    .bind(now)
    .execute(&state.pg)
    .await?;
    get_season(state, season_id).await?.ok_or_else(|| anyhow::anyhow!("season not found"))
}

pub async fn delete_season(state: &AppState, season_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM plant_seasons WHERE season_id = $1")
        .bind(season_id)
        .execute(&state.pg)
        .await?;
    Ok(())
}

pub async fn verify_season_ownership(state: &AppState, user_id: Uuid, season_id: Uuid) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM plant_seasons ps INNER JOIN plants p ON p.plant_id = ps.plant_id WHERE ps.season_id = $1 AND p.user_id = $2"
    )
    .bind(season_id)
    .bind(user_id)
    .fetch_one(&state.pg)
    .await?;
    if count == 0 {
        anyhow::bail!("season not found or not owned by user");
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_plant(status: PlantStatus) -> Plant {
        let now = Utc::now();
        Plant {
            plant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "RepoSample".into(),
            color_tag: "#0B3D2E".into(),
            start_date: now,
            phase: PlantPhase::Veg,
            phase_week: Some(1),
            status,
            is_active: false,
            archived: false,
            location_id: None,
            lifecycle: PlantLifecycle::default(),
            plant_year: None,
            catalog_id: None,
            pot_size_liters: None,
            soil_type: None,
            created_at: now,
            updated_at: now,
        }
        .normalized()
    }

    #[test]
    fn normalize_plant_status_keeps_current_status_when_nothing_requested() {
        let current = sample_plant(PlantStatus::Ended);
        assert_eq!(normalize_plant_status(&current, None), PlantStatus::Ended);
    }

    #[test]
    fn normalize_plant_status_respects_explicit_requested_status() {
        let current = sample_plant(PlantStatus::Archived);
        assert_eq!(normalize_plant_status(&current, Some(PlantStatus::Active)), PlantStatus::Active);
    }

    #[test]
    fn plant_status_parser_prefers_explicit_storage_value() {
        assert_eq!(parse_plant_status("deleted", true, true), PlantStatus::Deleted);
        assert_eq!(parse_plant_status("ended", false, false), PlantStatus::Ended);
    }
}

pub async fn create_user_with_credentials(state: &AppState, user_id: Uuid, email: String, password_hash: String, auth_mode: AuthMode) -> anyhow::Result<UserProfile> {
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO users (user_id, auth_mode, adult_confirmed, email, password_hash, locale, timezone, plan, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"
    )
    .bind(user_id)
    .bind(auth_mode_str(&auth_mode))
    .bind(false)
    .bind(email)
    .bind(password_hash)
    .bind("de-DE")
    .bind("Europe/Berlin")
    .bind("free")
    .bind(now)
    .bind(now)
    .execute(&state.pg)
    .await?;
    Ok(UserProfile {
        user_id,
        auth_mode,
        adult_confirmed: false,
        locale: "de-DE".into(),
        timezone: "Europe/Berlin".into(),
        plan: SubscriptionPlan::Free,
        created_at: now,
        updated_at: now,
    })
}

pub async fn get_password_hash_by_user_id(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<String>> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT password_hash FROM users WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.pg)
        .await?;
    Ok(row.and_then(|r| r.0))
}

// --- Transaction-based upserts with LWW timestamp guard (Phase 4a) ---

pub async fn upsert_plant_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, plant: &Plant) -> anyhow::Result<()> {
    let plant = plant.normalized_ref();
    sqlx::query(
        "INSERT INTO plants (plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, location_id, lifecycle, plant_year, catalog_id, created_at, updated_at, pot_size_liters, soil_type)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
         ON CONFLICT (plant_id) DO UPDATE SET
            user_id = EXCLUDED.user_id,
            name = EXCLUDED.name,
            color_tag = EXCLUDED.color_tag,
            start_date = EXCLUDED.start_date,
            phase = EXCLUDED.phase,
            phase_week = EXCLUDED.phase_week,
            plant_status = EXCLUDED.plant_status,
            is_active = EXCLUDED.is_active,
            archived = EXCLUDED.archived,
            location_id = EXCLUDED.location_id,
            lifecycle = EXCLUDED.lifecycle,
            plant_year = EXCLUDED.plant_year,
            catalog_id = EXCLUDED.catalog_id,
            pot_size_liters = EXCLUDED.pot_size_liters,
            soil_type = EXCLUDED.soil_type,
            updated_at = EXCLUDED.updated_at
         WHERE plants.updated_at < EXCLUDED.updated_at",
    )
    .bind(plant.plant_id)
    .bind(plant.user_id)
    .bind(&plant.name)
    .bind(&plant.color_tag)
    .bind(plant.start_date)
    .bind(phase_str(plant.phase))
    .bind(plant.phase_week)
    .bind(plant_status_str(plant.status))
    .bind(plant.is_active_state())
    .bind(plant.is_archived_state())
    .bind(plant.location_id)
    .bind(lifecycle_str(plant.lifecycle))
    .bind(plant.plant_year)
    .bind(&plant.catalog_id)
    .bind(plant.created_at)
    .bind(plant.updated_at)
    .bind(plant.pot_size_liters)
    .bind(&plant.soil_type)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn upsert_task_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, task: &Task) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO tasks (task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
         ON CONFLICT (task_id) DO UPDATE SET
            plant_id = EXCLUDED.plant_id,
            title = EXCLUDED.title,
            category = EXCLUDED.category,
            due_at = EXCLUDED.due_at,
            repeat_interval_hours = EXCLUDED.repeat_interval_hours,
            repeat_until = EXCLUDED.repeat_until,
            notification_enabled = EXCLUDED.notification_enabled,
            status = EXCLUDED.status,
            updated_at = EXCLUDED.updated_at
         WHERE tasks.updated_at < EXCLUDED.updated_at"
    )
    .bind(task.task_id)
    .bind(task.plant_id)
    .bind(&task.title)
    .bind(category_str(task.category))
    .bind(task.due_at)
    .bind(task.repeat_interval_hours)
    .bind(task.repeat_until)
    .bind(task.notification_enabled)
    .bind(status_str(task.status))
    .bind(task.created_at)
    .bind(task.updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn upsert_log_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, log: &LogEntry) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO logs (log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         ON CONFLICT (log_id) DO UPDATE SET
            plant_id = EXCLUDED.plant_id,
            log_type = EXCLUDED.log_type,
            text = EXCLUDED.text,
            metrics_json = EXCLUDED.metrics_json,
            action_json = EXCLUDED.action_json,
            photo_refs_json = EXCLUDED.photo_refs_json,
            created_at = EXCLUDED.created_at,
            updated_at = EXCLUDED.updated_at
         WHERE COALESCE(logs.updated_at, logs.created_at) < COALESCE(EXCLUDED.updated_at, EXCLUDED.created_at)"
    )
    .bind(log.log_id)
    .bind(log.plant_id)
    .bind(log_type_str(log.log_type))
    .bind(&log.text)
    .bind(log.metrics.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.action.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.photo_refs.as_ref().map(|m| serde_json::to_value(m).unwrap_or_default()))
    .bind(log.created_at)
    .bind(log.last_modified())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn record_delete_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, user_id: Uuid, record_id: Uuid, entity_kind: &str) -> anyhow::Result<()> {
    sqlx::query(
        // Conflict target includes user_id, and user_id is never rewritten:
        // a tombstone belongs to exactly one user.
        "INSERT INTO deleted_records (record_id, user_id, entity_kind, operation, changed_at) VALUES ($1,$2,$3,'delete',$4)
         ON CONFLICT (user_id, record_id, entity_kind) DO UPDATE SET changed_at = EXCLUDED.changed_at"
    )
    .bind(record_id)
    .bind(user_id)
    .bind(entity_kind)
    .bind(Utc::now())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn get_user_email(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<String>> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT email FROM users WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.pg)
        .await?;
    Ok(row.and_then(|r| r.0))
}

pub async fn set_stripe_customer_id(state: &AppState, user_id: Uuid, customer_id: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET stripe_customer_id = $2, updated_at = $3 WHERE user_id = $1")
        .bind(user_id)
        .bind(customer_id)
        .bind(Utc::now())
        .execute(&state.pg)
        .await?;
    Ok(())
}

pub async fn get_stripe_customer_id(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<String>> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT stripe_customer_id FROM users WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.pg)
        .await?;
    Ok(row.and_then(|r| r.0))
}

/// All plant ids owned by a user. Used by /sync/push so ownership for a whole
/// envelope costs one query instead of one per task and log.
pub async fn list_plant_ids(state: &AppState, user_id: Uuid) -> anyhow::Result<Vec<Uuid>> {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT plant_id FROM plants WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&state.pg)
        .await?;
    Ok(ids)
}

pub async fn verify_plant_ownership(state: &AppState, user_id: Uuid, plant_id: Uuid) -> Result<(), String> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM plants WHERE plant_id = $1 AND user_id = $2",
    )
    .bind(plant_id)
    .bind(user_id)
    .fetch_one(&state.pg)
    .await
    .map_err(|e| e.to_string())?;
    if count == 0 {
        return Err("forbidden: not your plant".into());
    }
    Ok(())
}

pub async fn verify_task_ownership(state: &AppState, user_id: Uuid, task_id: Uuid) -> Result<(), String> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks t INNER JOIN plants p ON t.plant_id = p.plant_id \
         WHERE t.task_id = $1 AND p.user_id = $2",
    )
    .bind(task_id)
    .bind(user_id)
    .fetch_one(&state.pg)
    .await
    .map_err(|e| e.to_string())?;
    if count == 0 {
        return Err("forbidden: not your task".into());
    }
    Ok(())
}

pub async fn verify_log_ownership(state: &AppState, user_id: Uuid, log_id: Uuid) -> Result<(), String> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs l INNER JOIN plants p ON l.plant_id = p.plant_id \
         WHERE l.log_id = $1 AND p.user_id = $2",
    )
    .bind(log_id)
    .bind(user_id)
    .fetch_one(&state.pg)
    .await
    .map_err(|e| e.to_string())?;
    if count == 0 {
        return Err("forbidden: not your log".into());
    }
    Ok(())
}

// ── Location CRUD ─��

pub async fn list_locations(state: &AppState, user_id: Uuid) -> anyhow::Result<Vec<Location>> {
    let rows = sqlx::query_as::<_, LocationRow>(
        "SELECT location_id, user_id, name, location_type, description, latitude, longitude, created_at, updated_at FROM locations WHERE user_id = $1 ORDER BY name ASC",
    )
    .bind(user_id)
    .fetch_all(&state.pg)
    .await?;
    Ok(rows.into_iter().map(location_from_row).collect())
}

/// Round a coordinate to two decimals (~1 km). The coordinates of a location
/// are often a home address; weather lookups need no more precision than this.
pub fn round_coord(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// A validated, rounded coordinate pair. Both or neither must be present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coords {
    pub latitude: f64,
    pub longitude: f64,
}

impl Coords {
    pub fn parse(latitude: Option<f64>, longitude: Option<f64>) -> Result<Option<Coords>, &'static str> {
        match (latitude, longitude) {
            (None, None) => Ok(None),
            (Some(lat), Some(lon)) => {
                if !lat.is_finite() || !lon.is_finite()
                    || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon)
                {
                    return Err("latitude must be -90..90 and longitude -180..180");
                }
                Ok(Some(Coords { latitude: round_coord(lat), longitude: round_coord(lon) }))
            }
            _ => Err("latitude and longitude must be given together"),
        }
    }
}

pub async fn create_location(state: &AppState, user_id: Uuid, name: String, location_type: String, description: Option<String>, coords: Option<Coords>) -> anyhow::Result<Location> {
    let now = Utc::now();
    let loc = Location {
        location_id: Uuid::new_v4(),
        user_id,
        name,
        location_type,
        description,
        latitude: coords.map(|c| c.latitude),
        longitude: coords.map(|c| c.longitude),
        created_at: now,
        updated_at: now,
    };
    sqlx::query(
        "INSERT INTO locations (location_id, user_id, name, location_type, description, latitude, longitude, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(loc.location_id)
    .bind(loc.user_id)
    .bind(&loc.name)
    .bind(&loc.location_type)
    .bind(&loc.description)
    .bind(loc.latitude)
    .bind(loc.longitude)
    .bind(loc.created_at)
    .bind(loc.updated_at)
    .execute(&state.pg)
    .await?;
    Ok(loc)
}

pub async fn update_location(state: &AppState, user_id: Uuid, location_id: Uuid, name: Option<String>, location_type: Option<String>, description: Option<String>, coords: Option<Coords>) -> anyhow::Result<Location> {
    let rows = sqlx::query_as::<_, LocationRow>(
        "SELECT location_id, user_id, name, location_type, description, latitude, longitude, created_at, updated_at FROM locations WHERE location_id = $1 AND user_id = $2",
    )
    .bind(location_id)
    .bind(user_id)
    .fetch_optional(&state.pg)
    .await?;
    let current = rows.map(location_from_row).ok_or_else(|| anyhow::anyhow!("location not found"))?;
    let next_name = name.unwrap_or(current.name);
    let next_type = location_type.unwrap_or(current.location_type);
    let next_desc = description.or(current.description);
    // Coordinates are replaced as a pair or kept; never half-updated.
    let next_lat = coords.map(|c| c.latitude).or(current.latitude);
    let next_lon = coords.map(|c| c.longitude).or(current.longitude);
    let now = Utc::now();
    sqlx::query("UPDATE locations SET name = $3, location_type = $4, description = $5, latitude = $6, longitude = $7, updated_at = $8 WHERE location_id = $1 AND user_id = $2")
        .bind(location_id)
        .bind(user_id)
        .bind(&next_name)
        .bind(&next_type)
        .bind(&next_desc)
        .bind(next_lat)
        .bind(next_lon)
        .bind(now)
        .execute(&state.pg)
        .await?;
    Ok(Location {
        location_id,
        user_id,
        name: next_name,
        location_type: next_type,
        description: next_desc,
        latitude: next_lat,
        longitude: next_lon,
        created_at: current.created_at,
        updated_at: now,
    })
}

pub async fn delete_location(state: &AppState, user_id: Uuid, location_id: Uuid) -> anyhow::Result<()> {
    // Unlink plants first
    sqlx::query("UPDATE plants SET location_id = NULL WHERE location_id = $1 AND user_id = $2")
        .bind(location_id)
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    sqlx::query("DELETE FROM locations WHERE location_id = $1 AND user_id = $2")
        .bind(location_id)
        .bind(user_id)
        .execute(&state.pg)
        .await?;
    Ok(())
}

// --- Retention (GDPR Art. 5(1)(e): storage limitation) ---

/// Number of rows removed by one retention pass, per table.
#[derive(Debug, Default, Clone, Copy)]
pub struct PruneReport {
    pub refresh_tokens: u64,
    pub deleted_records: u64,
    pub subscription_history: u64,
    pub weather_history: u64,
    pub push_reminders_sent: u64,
}

/// Delete operational rows that are no longer needed.
///
/// None of these tables had any retention, so they grew without bound for the
/// life of the deployment:
/// - `refresh_tokens`: expired or revoked tokens serve no purpose once past the
///   grace period, and they are credentials.
/// - `deleted_records`: tombstones only need to outlive the sync window; a
///   client that has not synced in a year performs a full resync anyway.
/// - `subscription_history`: keep the most recent entries per user for support,
///   drop the rest.
pub async fn prune_operational_data(
    state: &AppState,
    token_grace_days: i64,
    tombstone_days: i64,
    history_keep_per_user: i64,
    weather_days: i64,
) -> anyhow::Result<PruneReport> {
    let now = Utc::now();

    let tokens = sqlx::query(
        "DELETE FROM refresh_tokens \
         WHERE (expires_at < $1) OR (revoked_at IS NOT NULL AND revoked_at < $1)",
    )
    .bind(now - chrono::Duration::days(token_grace_days))
    .execute(&state.pg)
    .await?
    .rows_affected();

    let tombstones = sqlx::query("DELETE FROM deleted_records WHERE changed_at < $1")
        .bind(now - chrono::Duration::days(tombstone_days))
        .execute(&state.pg)
        .await?
        .rows_affected();

    let history = sqlx::query(
        "DELETE FROM subscription_history \
         WHERE event_id IN ( \
             SELECT event_id FROM ( \
                 SELECT event_id, row_number() OVER (PARTITION BY user_id ORDER BY created_at DESC) AS rn \
                 FROM subscription_history \
             ) ranked WHERE ranked.rn > $1 \
         )",
    )
    .bind(history_keep_per_user)
    .execute(&state.pg)
    .await?
    .rows_affected();

    // Upper bound regardless of what a user set in their own retention
    // setting; matches the period stated in the privacy policy.
    let weather = sqlx::query("DELETE FROM weather_history WHERE recorded_at < $1")
        .bind(now - chrono::Duration::days(weather_days))
        .execute(&state.pg)
        .await?
        .rows_affected();

    // Markers only need to outlive the 24 h window in which a task can still
    // be pushed; 60 days leaves room for clock and outage oddities.
    let push_sent = sqlx::query("DELETE FROM push_reminders_sent WHERE sent_at < $1")
        .bind(now - chrono::Duration::days(60))
        .execute(&state.pg)
        .await?
        .rows_affected();

    Ok(PruneReport {
        refresh_tokens: tokens,
        deleted_records: tombstones,
        subscription_history: history,
        weather_history: weather,
        push_reminders_sent: push_sent,
    })
}


#[cfg(test)]
mod coords_tests {
    use super::{round_coord, Coords};

    #[test]
    fn rounds_to_about_one_kilometre() {
        assert_eq!(round_coord(52.520008), 52.52);
        assert_eq!(round_coord(13.404954), 13.4);
        assert_eq!(round_coord(-33.86785), -33.87);
    }

    #[test]
    fn parse_accepts_pairs_and_rounds() {
        let c = Coords::parse(Some(48.137154), Some(11.576124)).unwrap().unwrap();
        assert_eq!(c, Coords { latitude: 48.14, longitude: 11.58 });
        assert_eq!(Coords::parse(None, None).unwrap(), None);
    }

    #[test]
    fn parse_rejects_half_pairs_and_out_of_range() {
        assert!(Coords::parse(Some(1.0), None).is_err());
        assert!(Coords::parse(None, Some(1.0)).is_err());
        assert!(Coords::parse(Some(91.0), Some(0.0)).is_err());
        assert!(Coords::parse(Some(0.0), Some(-180.5)).is_err());
        assert!(Coords::parse(Some(f64::NAN), Some(0.0)).is_err());
    }
}
