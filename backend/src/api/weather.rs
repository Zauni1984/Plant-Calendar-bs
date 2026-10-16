//! Climate settings and weather history per account.
//!
//! The desktop keeps both in its local database. In the web app they live on
//! the server so they follow the user across devices. The browser fetches the
//! forecast from Open-Meteo itself and posts the current reading here; the
//! server never calls Open-Meteo on a user's behalf.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared::{ClimateConfig, WeatherRecord};
use uuid::Uuid;

use super::api_error;
use crate::{
    db::{repos::Coords, AppState},
    services::auth::SessionUser,
};

/// The longest history a single request may ask for (10 years).
const MAX_HISTORY_HOURS: i64 = 24 * 3650;
/// Readings for the same place closer together than this are dropped, so two
/// open tabs (or a phone and a laptop) do not each write a row every hour.
const MIN_MINUTES_BETWEEN_READINGS: i64 = 10;

// --- climate settings --------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ClimateRow {
    latitude: f64,
    longitude: f64,
    city_name: Option<String>,
    last_spring_frost: Option<String>,
    first_fall_frost: Option<String>,
    timezone: Option<String>,
}

impl From<ClimateRow> for ClimateConfig {
    fn from(r: ClimateRow) -> Self {
        ClimateConfig {
            latitude: r.latitude,
            longitude: r.longitude,
            city_name: r.city_name,
            last_spring_frost: r.last_spring_frost,
            first_fall_frost: r.first_fall_frost,
            timezone: r.timezone,
        }
    }
}

/// "MM-DD" with a plausible month and day. The frontend writes exactly this
/// shape; anything else would break the frost-date maths downstream.
fn is_month_day(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 5 || b[2] != b'-' {
        return false;
    }
    let (Ok(m), Ok(d)) = (s[..2].parse::<u8>(), s[3..].parse::<u8>()) else { return false };
    (1..=12).contains(&m) && (1..=31).contains(&d)
}

fn bounded(value: Option<String>, max: usize, field: &str) -> Result<Option<String>, String> {
    match value {
        Some(v) if v.chars().count() > max => Err(format!("{field} must be at most {max} characters")),
        Some(v) if v.trim().is_empty() => Ok(None),
        other => Ok(other),
    }
}

pub async fn get_climate(State(state): State<AppState>, session: SessionUser) -> impl IntoResponse {
    let row = sqlx::query_as::<_, ClimateRow>(
        "SELECT latitude, longitude, city_name, last_spring_frost, first_fall_frost, timezone \
         FROM user_climate_config WHERE user_id = $1",
    )
    .bind(session.user_id)
    .fetch_optional(&state.pg)
    .await;
    match row {
        Ok(row) => Json(row.map(ClimateConfig::from)).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn put_climate(
    State(state): State<AppState>,
    session: SessionUser,
    Json(config): Json<ClimateConfig>,
) -> impl IntoResponse {
    let coords = match Coords::parse(Some(config.latitude), Some(config.longitude)) {
        Ok(Some(c)) => c,
        Ok(None) => unreachable!("both coordinates were provided"),
        Err(msg) => return api_error(StatusCode::BAD_REQUEST, msg),
    };
    for (value, field) in [(&config.last_spring_frost, "last_spring_frost"), (&config.first_fall_frost, "first_fall_frost")] {
        if let Some(v) = value {
            if !is_month_day(v) {
                return api_error(StatusCode::BAD_REQUEST, format!("{field} must be MM-DD"));
            }
        }
    }
    let city_name = match bounded(config.city_name, 120, "city_name") {
        Ok(v) => v,
        Err(msg) => return api_error(StatusCode::BAD_REQUEST, msg),
    };
    let timezone = match bounded(config.timezone, 64, "timezone") {
        Ok(v) => v,
        Err(msg) => return api_error(StatusCode::BAD_REQUEST, msg),
    };

    let result = sqlx::query(
        "INSERT INTO user_climate_config \
            (user_id, latitude, longitude, city_name, last_spring_frost, first_fall_frost, timezone, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,NOW()) \
         ON CONFLICT (user_id) DO UPDATE SET \
            latitude = EXCLUDED.latitude, longitude = EXCLUDED.longitude, city_name = EXCLUDED.city_name, \
            last_spring_frost = EXCLUDED.last_spring_frost, first_fall_frost = EXCLUDED.first_fall_frost, \
            timezone = EXCLUDED.timezone, updated_at = NOW()",
    )
    .bind(session.user_id)
    .bind(coords.latitude)
    .bind(coords.longitude)
    .bind(&city_name)
    .bind(&config.last_spring_frost)
    .bind(&config.first_fall_frost)
    .bind(&timezone)
    .execute(&state.pg)
    .await;

    match result {
        Ok(_) => Json(ClimateConfig {
            latitude: coords.latitude,
            longitude: coords.longitude,
            city_name,
            last_spring_frost: config.last_spring_frost,
            first_fall_frost: config.first_fall_frost,
            timezone,
        })
        .into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

// --- weather history ---------------------------------------------------------

#[derive(sqlx::FromRow)]
struct WeatherRow {
    id: Uuid,
    location_id: Option<Uuid>,
    latitude: f64,
    longitude: f64,
    temp_c: Option<f32>,
    humidity: Option<f32>,
    wind_kmh: Option<f32>,
    precipitation_mm: Option<f32>,
    weather_code: Option<i32>,
    recorded_at: DateTime<Utc>,
    apparent_temp_c: Option<f32>,
    uv_index: Option<f32>,
    cloud_cover: Option<f32>,
    pressure_hpa: Option<f32>,
    dew_point_c: Option<f32>,
    visibility_km: Option<f32>,
    wind_gusts_kmh: Option<f32>,
    wind_direction_deg: Option<f32>,
    soil_temp_c: Option<f32>,
    soil_moisture: Option<f32>,
    is_day: Option<bool>,
}

impl From<WeatherRow> for WeatherRecord {
    fn from(r: WeatherRow) -> Self {
        WeatherRecord {
            id: r.id,
            location_id: r.location_id,
            latitude: r.latitude,
            longitude: r.longitude,
            temp_c: r.temp_c,
            humidity: r.humidity,
            wind_kmh: r.wind_kmh,
            precipitation_mm: r.precipitation_mm,
            weather_code: r.weather_code,
            recorded_at: r.recorded_at,
            apparent_temp_c: r.apparent_temp_c,
            uv_index: r.uv_index,
            cloud_cover: r.cloud_cover,
            pressure_hpa: r.pressure_hpa,
            dew_point_c: r.dew_point_c,
            visibility_km: r.visibility_km,
            wind_gusts_kmh: r.wind_gusts_kmh,
            wind_direction_deg: r.wind_direction_deg,
            soil_temp_c: r.soil_temp_c,
            soil_moisture: r.soil_moisture,
            is_day: r.is_day,
        }
    }
}

const WEATHER_COLUMNS: &str = "id, location_id, latitude, longitude, temp_c, humidity, wind_kmh, \
    precipitation_mm, weather_code, recorded_at, apparent_temp_c, uv_index, cloud_cover, pressure_hpa, \
    dew_point_c, visibility_km, wind_gusts_kmh, wind_direction_deg, soil_temp_c, soil_moisture, is_day";

/// A reading as posted by the browser. `id` and `recorded_at` are assigned by
/// the server — client clocks are not trusted for ordering or retention.
#[derive(Deserialize)]
pub struct WeatherReadingInput {
    pub location_id: Option<Uuid>,
    pub latitude: f64,
    pub longitude: f64,
    pub temp_c: Option<f32>,
    pub humidity: Option<f32>,
    pub wind_kmh: Option<f32>,
    pub precipitation_mm: Option<f32>,
    pub weather_code: Option<i32>,
    pub apparent_temp_c: Option<f32>,
    pub uv_index: Option<f32>,
    pub cloud_cover: Option<f32>,
    pub pressure_hpa: Option<f32>,
    pub dew_point_c: Option<f32>,
    pub visibility_km: Option<f32>,
    pub wind_gusts_kmh: Option<f32>,
    pub wind_direction_deg: Option<f32>,
    pub soil_temp_c: Option<f32>,
    pub soil_moisture: Option<f32>,
    pub is_day: Option<bool>,
}

pub async fn post_weather_reading(
    State(state): State<AppState>,
    session: SessionUser,
    Json(input): Json<WeatherReadingInput>,
) -> impl IntoResponse {
    let coords = match Coords::parse(Some(input.latitude), Some(input.longitude)) {
        Ok(Some(c)) => c,
        Ok(None) => unreachable!("both coordinates were provided"),
        Err(msg) => return api_error(StatusCode::BAD_REQUEST, msg),
    };

    // A location id must belong to the caller, or a reading could be attached
    // to someone else's location.
    if let Some(location_id) = input.location_id {
        let owned: Result<i64, _> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM locations WHERE location_id = $1 AND user_id = $2",
        )
        .bind(location_id)
        .bind(session.user_id)
        .fetch_one(&state.pg)
        .await;
        match owned {
            Ok(0) => return api_error(StatusCode::FORBIDDEN, "not your location"),
            Ok(_) => {}
            Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        }
    }

    // Drop near-duplicates for the same place.
    let recent: Result<i64, _> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM weather_history \
         WHERE user_id = $1 AND location_id IS NOT DISTINCT FROM $2 \
           AND recorded_at > NOW() - make_interval(mins => $3)",
    )
    .bind(session.user_id)
    .bind(input.location_id)
    .bind(MIN_MINUTES_BETWEEN_READINGS as i32)
    .fetch_one(&state.pg)
    .await;
    match recent {
        Ok(n) if n > 0 => return Json(serde_json::json!({ "stored": false, "reason": "recent reading exists" })).into_response(),
        Ok(_) => {}
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }

    let id = Uuid::new_v4();
    let result = sqlx::query(&format!(
        "INSERT INTO weather_history (user_id, {WEATHER_COLUMNS}) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NOW(),$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)"
    ))
    .bind(session.user_id)
    .bind(id)
    .bind(input.location_id)
    .bind(coords.latitude)
    .bind(coords.longitude)
    .bind(input.temp_c)
    .bind(input.humidity)
    .bind(input.wind_kmh)
    .bind(input.precipitation_mm)
    .bind(input.weather_code)
    .bind(input.apparent_temp_c)
    .bind(input.uv_index)
    .bind(input.cloud_cover)
    .bind(input.pressure_hpa)
    .bind(input.dew_point_c)
    .bind(input.visibility_km)
    .bind(input.wind_gusts_kmh)
    .bind(input.wind_direction_deg)
    .bind(input.soil_temp_c)
    .bind(input.soil_moisture)
    .bind(input.is_day)
    .execute(&state.pg)
    .await;

    match result {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({ "stored": true, "id": id }))).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub location_id: Option<Uuid>,
    pub hours: Option<i64>,
    pub days: Option<i64>,
}

/// Same range rules as the desktop: `hours` wins over `days`, default 24 h.
fn history_hours(q: &HistoryQuery) -> i64 {
    let h = match (q.hours, q.days) {
        (Some(h), _) => h,
        (None, Some(d)) => d.saturating_mul(24),
        (None, None) => 24,
    };
    h.clamp(1, MAX_HISTORY_HOURS)
}

pub async fn list_weather_history(
    State(state): State<AppState>,
    session: SessionUser,
    Query(q): Query<HistoryQuery>,
) -> impl IntoResponse {
    let hours = history_hours(&q);
    let rows = sqlx::query_as::<_, WeatherRow>(&format!(
        "SELECT {WEATHER_COLUMNS} FROM weather_history \
         WHERE user_id = $1 \
           AND ($2::uuid IS NULL OR location_id = $2) \
           AND recorded_at > NOW() - make_interval(hours => $3) \
         ORDER BY recorded_at ASC"
    ))
    .bind(session.user_id)
    .bind(q.location_id)
    .bind(hours as i32)
    .fetch_all(&state.pg)
    .await;
    match rows {
        Ok(rows) => Json(rows.into_iter().map(WeatherRecord::from).collect::<Vec<_>>()).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct PruneQuery {
    pub older_than_days: i64,
}

/// The user-chosen retention window from Settings. The server additionally
/// enforces its own maximum in the daily retention sweep.
pub async fn prune_weather_history(
    State(state): State<AppState>,
    session: SessionUser,
    Query(q): Query<PruneQuery>,
) -> impl IntoResponse {
    if !(1..=3650).contains(&q.older_than_days) {
        return api_error(StatusCode::BAD_REQUEST, "older_than_days must be 1-3650");
    }
    let result = sqlx::query(
        "DELETE FROM weather_history WHERE user_id = $1 AND recorded_at < NOW() - make_interval(days => $2)",
    )
    .bind(session.user_id)
    .bind(q.older_than_days as i32)
    .execute(&state.pg)
    .await;
    match result {
        Ok(r) => Json(serde_json::json!({ "deleted": r.rows_affected() })).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

/// For the GDPR export: everything stored about the caller in this module.
pub async fn export_for_user(state: &AppState, user_id: Uuid) -> anyhow::Result<serde_json::Value> {
    let climate = sqlx::query_as::<_, ClimateRow>(
        "SELECT latitude, longitude, city_name, last_spring_frost, first_fall_frost, timezone \
         FROM user_climate_config WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pg)
    .await?
    .map(ClimateConfig::from);
    let history: Vec<WeatherRecord> = sqlx::query_as::<_, WeatherRow>(&format!(
        "SELECT {WEATHER_COLUMNS} FROM weather_history WHERE user_id = $1 ORDER BY recorded_at ASC"
    ))
    .bind(user_id)
    .fetch_all(&state.pg)
    .await?
    .into_iter()
    .map(WeatherRecord::from)
    .collect();
    Ok(serde_json::json!({ "climate_config": climate, "weather_history": history }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_day_validation() {
        for ok in ["01-01", "05-15", "12-31", "02-29"] {
            assert!(is_month_day(ok), "{ok} should be valid");
        }
        for bad in ["1-01", "13-01", "00-10", "05-32", "05/15", "05-1", "", "ab-cd", "05-15x"] {
            assert!(!is_month_day(bad), "{bad} should be invalid");
        }
    }

    #[test]
    fn history_range_rules_match_desktop() {
        let q = |hours, days| HistoryQuery { location_id: None, hours, days };
        assert_eq!(history_hours(&q(None, None)), 24);
        assert_eq!(history_hours(&q(Some(6), Some(7))), 6, "hours wins over days");
        assert_eq!(history_hours(&q(None, Some(7))), 168);
        assert_eq!(history_hours(&q(Some(0), None)), 1, "clamped up");
        assert_eq!(history_hours(&q(None, Some(1_000_000))), MAX_HISTORY_HOURS, "clamped down");
    }

    #[test]
    fn text_fields_are_bounded_and_blank_becomes_none() {
        assert_eq!(bounded(Some("Berlin".into()), 120, "f").unwrap().as_deref(), Some("Berlin"));
        assert_eq!(bounded(Some("   ".into()), 120, "f").unwrap(), None);
        assert!(bounded(Some("x".repeat(121)), 120, "f").is_err());
        assert_eq!(bounded(None, 120, "f").unwrap(), None);
    }
}
