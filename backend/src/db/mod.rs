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
    /// Signs desktop license certificates (LICENSE_SIGNING_KEY).
    pub license_key: std::sync::Arc<shared::license::SigningKey>,
}

impl AppState {
    pub async fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/sowling".to_string());
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
        let license_key = std::sync::Arc::new(license_signing_key()?);
        Ok(Self { pg, jwt_secret, photos, push, license_key })
    }
}

/// LICENSE_SIGNING_KEY (base64url Ed25519 seed, `sowling-backend gen-license-keys`).
/// Debug builds fall back to the public development key, like JWT_SECRET.
fn license_signing_key() -> anyhow::Result<shared::license::SigningKey> {
    match std::env::var("LICENSE_SIGNING_KEY") {
        Ok(b64) => shared::license::signing_key_from_b64(&b64)
            .ok_or_else(|| anyhow::anyhow!("LICENSE_SIGNING_KEY is not a base64url Ed25519 key")),
        Err(_) if cfg!(debug_assertions) => {
            tracing::warn!("LICENSE_SIGNING_KEY not set, using the development key. DO NOT use in production.");
            Ok(shared::license::SigningKey::from_bytes(&shared::license::DEV_SIGNING_SEED))
        }
        Err(_) => anyhow::bail!("LICENSE_SIGNING_KEY must be set in production (release build)"),
    }
}
