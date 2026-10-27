//! Plant photos for the web app.
//!
//! Rules (decided with the product owner):
//! - Uploading needs a paid plan. Viewing and deleting your own photos never do.
//! - Every upload is re-encoded to lossy WebP on the server (services/images.rs).
//! - When a paid plan falls back to Free, photos stay viewable for
//!   `RETENTION_AFTER_PAID_DAYS`, then all of that user's photos are deleted
//!   unless they resubscribe first. The app shows the date and offers a ZIP
//!   download of everything in the meantime.

use std::{io::Write, sync::OnceLock};

use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use shared::PhotoRef;
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::api_error;
use crate::{
    db::{repos, AppState},
    services::{auth::SessionUser, images},
};

/// How long photos survive after a paid plan ends.
pub const RETENTION_AFTER_PAID_DAYS: i64 = 90;
/// Request body cap for one upload; enforced on the route.
pub const MAX_UPLOAD_BYTES: usize = 15 * 1024 * 1024;
const MAX_CAPTION_CHARS: usize = 500;
/// Decoding and encoding are CPU-heavy. Bounding concurrency keeps a burst of
/// uploads from starving every other request on the server.
const MAX_CONCURRENT_ENCODES: usize = 2;

fn encode_permits() -> &'static Semaphore {
    static PERMITS: OnceLock<Semaphore> = OnceLock::new();
    PERMITS.get_or_init(|| Semaphore::new(MAX_CONCURRENT_ENCODES))
}

fn photo_url(photo_id: Uuid) -> String {
    format!("/photos/{photo_id}")
}

#[derive(sqlx::FromRow)]
struct PhotoRow {
    photo_id: Uuid,
    plant_id: Uuid,
    log_id: Option<Uuid>,
    season_id: Option<Uuid>,
    caption: Option<String>,
    taken_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<PhotoRow> for PhotoRef {
    fn from(r: PhotoRow) -> Self {
        PhotoRef {
            photo_id: r.photo_id,
            plant_id: r.plant_id,
            log_id: r.log_id,
            season_id: r.season_id,
            file_path: photo_url(r.photo_id),
            caption: r.caption,
            taken_at: r.taken_at,
            created_at: r.created_at,
        }
    }
}

const PHOTO_COLUMNS: &str = "photo_id, plant_id, log_id, season_id, caption, taken_at, created_at";

async fn user_has_paid_plan(state: &AppState, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let plan: Option<String> = sqlx::query_scalar("SELECT plan FROM users WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.pg)
        .await?;
    Ok(matches!(plan.as_deref(), Some(p) if p != "free"))
}

// --- upload ------------------------------------------------------------------

#[derive(Deserialize)]
pub struct UploadQuery {
    pub log_id: Option<Uuid>,
    pub season_id: Option<Uuid>,
    pub caption: Option<String>,
}

pub async fn upload_photo(
    Path(plant_id): Path<Uuid>,
    State(state): State<AppState>,
    session: SessionUser,
    Query(q): Query<UploadQuery>,
    body: Bytes,
) -> Response {
    match user_has_paid_plan(&state, session.user_id).await {
        Ok(true) => {}
        Ok(false) => return api_error(StatusCode::FORBIDDEN, "photos require a paid plan"),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
    if let Err(err) = repos::verify_plant_ownership(&state, session.user_id, plant_id).await {
        return api_error(StatusCode::FORBIDDEN, err);
    }
    // A log or season id must belong to this very plant; otherwise a photo
    // could be attached to another user's records.
    for (id, table, column) in [(q.log_id, "logs", "log_id"), (q.season_id, "plant_seasons", "season_id")] {
        if let Some(id) = id {
            let n: Result<i64, _> = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) FROM {table} WHERE {column} = $1 AND plant_id = $2"
            ))
            .bind(id)
            .bind(plant_id)
            .fetch_one(&state.pg)
            .await;
            match n {
                Ok(0) => return api_error(StatusCode::BAD_REQUEST, format!("{column} does not belong to this plant")),
                Ok(_) => {}
                Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
            }
        }
    }
    let caption = q.caption.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    if caption.as_ref().is_some_and(|c| c.chars().count() > MAX_CAPTION_CHARS) {
        return api_error(StatusCode::BAD_REQUEST, format!("caption must be at most {MAX_CAPTION_CHARS} characters"));
    }
    if body.is_empty() {
        return api_error(StatusCode::BAD_REQUEST, "empty upload");
    }

    let processed = {
        let _permit = encode_permits().acquire().await;
        match tokio::task::spawn_blocking(move || images::to_webp(&body)).await {
            Ok(Ok(p)) => p,
            Ok(Err(images::ImageError::Unsupported)) => {
                return api_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, images::ImageError::Unsupported)
            }
            Ok(Err(err)) => return api_error(StatusCode::UNPROCESSABLE_ENTITY, err),
            Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        }
    };

    let photo_id = Uuid::new_v4();
    if let Err(err) = state.photos.put(session.user_id, photo_id, &processed.webp).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }
    let now = Utc::now();
    let inserted = sqlx::query(
        "INSERT INTO photos (photo_id, user_id, plant_id, log_id, season_id, caption, taken_at, created_at, byte_size, width, height) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$7,$8,$9,$10)",
    )
    .bind(photo_id)
    .bind(session.user_id)
    .bind(plant_id)
    .bind(q.log_id)
    .bind(q.season_id)
    .bind(&caption)
    .bind(now)
    .bind(processed.webp.len() as i32)
    .bind(processed.width as i32)
    .bind(processed.height as i32)
    .execute(&state.pg)
    .await;
    if let Err(err) = inserted {
        // No row means nobody can reach or delete the file; remove it now.
        let _ = state.photos.delete(session.user_id, photo_id).await;
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }

    (
        StatusCode::CREATED,
        Json(PhotoRef {
            photo_id,
            plant_id,
            log_id: q.log_id,
            season_id: q.season_id,
            file_path: photo_url(photo_id),
            caption,
            taken_at: now,
            created_at: now,
        }),
    )
        .into_response()
}

// --- listing and serving -----------------------------------------------------

pub async fn list_plant_photos(Path(plant_id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> Response {
    let rows = sqlx::query_as::<_, PhotoRow>(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos WHERE user_id = $1 AND plant_id = $2 ORDER BY taken_at DESC"
    ))
    .bind(session.user_id)
    .bind(plant_id)
    .fetch_all(&state.pg)
    .await;
    match rows {
        Ok(rows) => Json(rows.into_iter().map(PhotoRef::from).collect::<Vec<_>>()).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn list_log_photos(Path(log_id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> Response {
    let rows = sqlx::query_as::<_, PhotoRow>(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos WHERE user_id = $1 AND log_id = $2 ORDER BY taken_at DESC"
    ))
    .bind(session.user_id)
    .bind(log_id)
    .fetch_all(&state.pg)
    .await;
    match rows {
        Ok(rows) => Json(rows.into_iter().map(PhotoRef::from).collect::<Vec<_>>()).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn get_photo(Path(photo_id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> Response {
    // Ownership is part of the lookup: another user's photo id is simply "not found".
    let owned: Result<i64, _> = sqlx::query_scalar("SELECT COUNT(*) FROM photos WHERE photo_id = $1 AND user_id = $2")
        .bind(photo_id)
        .bind(session.user_id)
        .fetch_one(&state.pg)
        .await;
    match owned {
        Ok(0) => return api_error(StatusCode::NOT_FOUND, "photo not found"),
        Ok(_) => {}
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
    match state.photos.get(session.user_id, photo_id).await {
        Ok(bytes) => {
            let mut resp = Response::new(Body::from(bytes));
            let h = resp.headers_mut();
            h.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/webp"));
            // Private: personal content must never land in a shared cache.
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=86400, immutable"));
            resp
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => api_error(StatusCode::NOT_FOUND, "photo file missing"),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn delete_photo(Path(photo_id): Path<Uuid>, State(state): State<AppState>, session: SessionUser) -> Response {
    let deleted = sqlx::query("DELETE FROM photos WHERE photo_id = $1 AND user_id = $2")
        .bind(photo_id)
        .bind(session.user_id)
        .execute(&state.pg)
        .await;
    match deleted {
        Ok(r) if r.rows_affected() == 0 => api_error(StatusCode::NOT_FOUND, "photo not found"),
        Ok(_) => match state.photos.delete(session.user_id, photo_id).await {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        },
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

/// Remove the photos of one plant (used when the plant is deleted). Files go
/// first: if the row deletion then failed, the photos would show as broken —
/// the reverse order could leave personal images on disk with no way to reach them.
pub async fn delete_photos_for_plant(state: &AppState, user_id: Uuid, plant_id: Uuid) -> anyhow::Result<u64> {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT photo_id FROM photos WHERE user_id = $1 AND plant_id = $2")
        .bind(user_id)
        .bind(plant_id)
        .fetch_all(&state.pg)
        .await?;
    for id in &ids {
        state.photos.delete(user_id, *id).await?;
    }
    let r = sqlx::query("DELETE FROM photos WHERE user_id = $1 AND plant_id = $2")
        .bind(user_id)
        .bind(plant_id)
        .execute(&state.pg)
        .await?;
    Ok(r.rows_affected())
}

// --- status (drives the deletion banner) ---------------------------------------

#[derive(sqlx::FromRow)]
struct PlanState {
    plan: String,
    paid_plan_ended_at: Option<DateTime<Utc>>,
}

/// When this user's photos will be deleted, if a deletion is pending.
pub fn deletion_date(plan: &str, paid_plan_ended_at: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    if plan != "free" {
        return None;
    }
    paid_plan_ended_at.map(|t| t + Duration::days(RETENTION_AFTER_PAID_DAYS))
}

pub async fn photo_status(State(state): State<AppState>, session: SessionUser) -> Response {
    let plan = sqlx::query_as::<_, PlanState>("SELECT plan, paid_plan_ended_at FROM users WHERE user_id = $1")
        .bind(session.user_id)
        .fetch_optional(&state.pg)
        .await;
    let count: Result<i64, _> = sqlx::query_scalar("SELECT COUNT(*) FROM photos WHERE user_id = $1")
        .bind(session.user_id)
        .fetch_one(&state.pg)
        .await;
    match (plan, count) {
        (Ok(plan), Ok(count)) => {
            let (plan_name, ended) = plan.map(|p| (p.plan, p.paid_plan_ended_at)).unwrap_or_else(|| ("free".into(), None));
            let deletion = if count > 0 { deletion_date(&plan_name, ended) } else { None };
            Json(serde_json::json!({
                "can_upload": plan_name != "free",
                "photo_count": count,
                "deletion_scheduled_at": deletion,
                "retention_days_after_paid": RETENTION_AFTER_PAID_DAYS,
            }))
            .into_response()
        }
        (Err(err), _) | (_, Err(err)) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

// --- download everything as ZIP --------------------------------------------------

pub async fn photo_archive(State(state): State<AppState>, session: SessionUser) -> Response {
    let rows = match sqlx::query_as::<_, PhotoRow>(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos WHERE user_id = $1 ORDER BY taken_at ASC"
    ))
    .bind(session.user_id)
    .fetch_all(&state.pg)
    .await
    {
        Ok(rows) => rows,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };

    let store = state.photos.clone();
    let user_id = session.user_id;
    // Build into an anonymous temp file (deleted when closed) and stream it, so
    // a large library never has to fit in memory.
    let built = tokio::task::spawn_blocking(move || -> anyhow::Result<std::fs::File> {
        let file = tempfile::tempfile()?;
        let mut zip = zip::ZipWriter::new(file);
        // WebP is already compressed; deflating it again only burns CPU.
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let mut index = Vec::with_capacity(rows.len());
        for r in rows {
            let name = format!("photos/{}_{}.webp", r.taken_at.format("%Y-%m-%d"), r.photo_id);
            match std::fs::read(store.path(user_id, r.photo_id)) {
                Ok(bytes) => {
                    zip.start_file(&name, opts)?;
                    zip.write_all(&bytes)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            }
            let photo: PhotoRef = r.into();
            index.push(serde_json::json!({ "file": name, "photo": photo }));
        }
        zip.start_file("photos.json", opts)?;
        zip.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
        let mut file = zip.finish()?;
        std::io::Seek::rewind(&mut file)?;
        Ok(file)
    })
    .await;

    let file = match built {
        Ok(Ok(f)) => tokio::fs::File::from_std(f),
        Ok(Err(err)) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let mut resp = Response::new(Body::from_stream(tokio_util::io::ReaderStream::new(file)));
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"sowling-fotos.zip\""),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

// --- GDPR export and the post-subscription purge ---------------------------------

pub async fn export_for_user(state: &AppState, user_id: Uuid) -> anyhow::Result<Vec<PhotoRef>> {
    Ok(sqlx::query_as::<_, PhotoRow>(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos WHERE user_id = $1 ORDER BY taken_at ASC"
    ))
    .bind(user_id)
    .fetch_all(&state.pg)
    .await?
    .into_iter()
    .map(PhotoRef::from)
    .collect())
}

/// Delete all photos of accounts whose paid plan ended more than
/// `RETENTION_AFTER_PAID_DAYS` ago. Run from the daily retention sweep.
/// Returns (accounts purged, photos deleted).
pub async fn purge_expired(state: &AppState) -> anyhow::Result<(u64, u64)> {
    let users: Vec<Uuid> = sqlx::query_scalar(
        "SELECT u.user_id FROM users u \
         WHERE u.plan = 'free' AND u.paid_plan_ended_at IS NOT NULL \
           AND u.paid_plan_ended_at < NOW() - make_interval(days => $1) \
           AND EXISTS (SELECT 1 FROM photos p WHERE p.user_id = u.user_id)",
    )
    .bind(RETENTION_AFTER_PAID_DAYS as i32)
    .fetch_all(&state.pg)
    .await?;

    let mut photos = 0;
    for user_id in &users {
        // Files first (see delete_photos_for_plant for why).
        state.photos.delete_user(*user_id).await?;
        let r = sqlx::query("DELETE FROM photos WHERE user_id = $1")
            .bind(user_id)
            .execute(&state.pg)
            .await?;
        photos += r.rows_affected();
    }
    Ok((users.len() as u64, photos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn deletion_is_scheduled_only_for_free_accounts_with_an_end_date() {
        let ended = Utc.with_ymd_and_hms(2026, 1, 10, 12, 0, 0).unwrap();
        assert_eq!(
            deletion_date("free", Some(ended)),
            Some(Utc.with_ymd_and_hms(2026, 4, 10, 12, 0, 0).unwrap()),
            "90 days after the paid plan ended"
        );
        assert_eq!(deletion_date("free", None), None, "never paid: nothing to delete");
        assert_eq!(deletion_date("basic", Some(ended)), None, "resubscribed: clock stopped");
        assert_eq!(deletion_date("pro", None), None);
    }
}
