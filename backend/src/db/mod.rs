pub mod migrate;
pub mod models;
pub mod repos;

use sqlx::{postgres::PgPoolOptions, PgPool};

#[derive(Clone)]
pub struct AppState {
    pub pg: PgPool,
    pub jwt_secret: String,
    pub photos: crate::services::photo_store::PhotoStore,
    /// None when VAPID keys are not configured (push reminders off).
    pub push: Option<std::sync::Arc<crate::services::push::PushConfig>>,
}

impl AppState {
    pub async fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/plants_calendar".to_string());
        let pg = PgPoolOptions::new().max_connections(5).connect_lazy(&database_url)?;
        // Accept fallback only in debug builds. In release, require explicit env var.
        let jwt_secret = std::env::var("JWT_SECRET")
            .or_else(|_| {
                if cfg!(debug_assertions) {
                    tracing::warn!("JWT_SECRET not set, using dev fallback. DO NOT use in production.");
                    Ok::<_, std::env::VarError>("dev-secret-change-me".to_string())
                } else {
                    Err(std::env::VarError::NotPresent)
                }
            })
            .map_err(|_| anyhow::anyhow!("JWT_SECRET must be set in production (release build)"))?;
        let photos = crate::services::photo_store::PhotoStore::from_env();
        tracing::info!(dir = %photos.root().display(), "photo storage");
        let push = crate::services::push::PushConfig::from_env()?;
        Ok(Self { pg, jwt_secret, photos, push })
    }
}
