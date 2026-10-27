mod api;
mod db;
mod services;

use axum::{middleware, routing::{get, patch, post}, Extension, Router};
use db::AppState;
use services::rate_limit::RateLimiter;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `sowling-backend gen-vapid-keys`: one-off helper for setting up push.
    if std::env::args().nth(1).as_deref() == Some("gen-vapid-keys") {
        let (private, public) = services::push::generate_vapid_keys();
        println!("VAPID_PRIVATE_KEY={private}
# public key (served at /push/public-key): {public}");
        return Ok(());
    }

    tracing_subscriber::registry()
        // CLI subcommands print their own output; only warnings in between.
        .with(tracing_subscriber::EnvFilter::new(if std::env::args().nth(1).is_some() { "warn" } else { "info" }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // `sowling-backend gen-license-keys` / `sowling-backend license ...`
    match std::env::args().nth(1).as_deref() {
        Some("gen-license-keys") => {
            use rand_core::{OsRng, RngCore};
            let mut seed = [0u8; 32];
            OsRng.fill_bytes(&mut seed);
            let (private, public) = shared::license::keypair_from_seed(seed);
            println!("# backend env (keep secret):\nLICENSE_SIGNING_KEY={private}");
            println!("# desktop release build env:\nSOWLING_LICENSE_PUBLIC_KEY={public}");
            return Ok(());
        }
        Some("license") => {
            let state = AppState::from_env().await?;
            db::migrate::run(&state.pg).await.map_err(|e| anyhow::anyhow!("database migration failed: {e}"))?;
            return services::license_cli::run(&state, std::env::args().skip(2).collect()).await;
        }
        _ => {}
    }

    let state = AppState::from_env().await?;

    // Bring the schema up to date before serving. Failing here (database
    // unreachable, broken migration) stops the process with a clear error
    // instead of every request failing later.
    let applied = db::migrate::run(&state.pg)
        .await
        .map_err(|e| anyhow::anyhow!("database migration failed: {e}"))?;
    if applied.is_empty() {
        tracing::info!("database schema up to date");
    } else {
        tracing::info!(count = applied.len(), migrations = ?applied, "applied database migrations");
    }
    // Cloned up front: the router takes ownership of `state` below.
    let prune_state = state.clone();
    let push_state = state.clone();

    // Rate limiter: 10 requests per 60 seconds per IP for auth endpoints
    let auth_rate_limiter = RateLimiter::new(10, 60);
    // Stricter limiter for password-change: 5 per 15 minutes per IP
    let password_rate_limiter = RateLimiter::new(5, 900);

    let auth_routes = Router::new()
        .route("/auth/register", post(api::auth_register))
        .route("/auth/login", post(api::auth_login))
        .route("/auth/google", post(api::auth_google))
        .route("/auth/logout", post(api::auth_logout))
        .route("/auth/refresh", post(api::auth_refresh))
        // Unauthenticated license calls share the auth rate limit (key guessing).
        .route("/licenses/activate", post(api::licenses::activate))
        .route("/licenses/refresh", post(api::licenses::refresh))
        .route("/licenses/deactivate", post(api::licenses::deactivate))
        .layer(middleware::from_fn(services::rate_limit::rate_limit_middleware))
        .layer(Extension(auth_rate_limiter.clone()));

    let password_change_routes = Router::new()
        .route("/me/change-password", post(api::change_password))
        .layer(middleware::from_fn(services::rate_limit::rate_limit_middleware))
        .layer(Extension(password_rate_limiter.clone()));

    let app = Router::new()
        .route("/health", get(api::health))
        .route("/catalog", get(api::get_catalog))
        .merge(auth_routes)
        .merge(password_change_routes)
        .route("/me", get(api::get_me).patch(api::update_profile).delete(api::delete_account))
        .route("/me/export", get(api::export_me))
        .route("/me/import-guest", post(api::guest_import::import_guest_data))
        .route("/me/consents", get(api::list_consents).post(api::upsert_consent))
        .route("/me/adult-confirmation", patch(api::patch_adult_confirmation))
        .route("/locations", get(api::list_locations).post(api::create_location))
        .route("/locations/{id}", patch(api::update_location).delete(api::delete_location))
        .route("/plants", get(api::list_plants).post(api::create_plant))
        .route("/plants/{id}", get(api::get_plant).patch(api::update_plant).delete(api::delete_plant))
        .route("/plants/{id}/seasons", get(api::list_seasons_for_plant).post(api::create_season_for_plant))
        .route("/seasons/{id}", patch(api::update_season).delete(api::delete_season))
        .route("/me/climate", get(api::weather::get_climate).put(api::weather::put_climate))
        .route(
            "/weather/history",
            get(api::weather::list_weather_history)
                .post(api::weather::post_weather_reading)
                .delete(api::weather::prune_weather_history),
        )
        .route("/photos/status", get(api::photos::photo_status))
        .route("/photos/archive", get(api::photos::photo_archive))
        .route("/photos/{id}", get(api::photos::get_photo).delete(api::photos::delete_photo))
        .route(
            "/plants/{id}/photos",
            get(api::photos::list_plant_photos)
                .post(api::photos::upload_photo)
                // axum caps bodies at 2 MB by default; phone photos are larger.
                .layer(axum::extract::DefaultBodyLimit::max(api::photos::MAX_UPLOAD_BYTES)),
        )
        .route("/logs/{id}/photos", get(api::photos::list_log_photos))
        .route("/tasks", get(api::list_my_tasks))
        .route("/push/public-key", get(api::push::public_key))
        .route("/push/subscriptions", post(api::push::subscribe).delete(api::push::unsubscribe))
        .route("/push/test", post(api::push::send_test))
        .route("/licenses/desktop-access", post(api::licenses::desktop_access))
        .route("/licenses/me", get(api::licenses::my_license))
        .route("/licenses/checkout", post(api::licenses::checkout))
        .route("/logs", get(api::list_my_logs))
        .route("/plants/{id}/tasks", get(api::list_tasks_for_plant).post(api::create_task_for_plant))
        .route("/tasks/{id}", patch(api::update_task).delete(api::delete_task))
        .route("/plants/{id}/logs", get(api::list_logs_for_plant).post(api::create_log_for_plant))
        .route("/logs/{id}", patch(api::update_log).delete(api::delete_log))
        .route("/subscription", get(api::get_subscription))
        .route("/subscription/status", get(api::get_subscription_status))
        .route("/subscription/history", get(api::get_subscription_history))
        .route("/subscription/checkout", post(api::post_subscription_checkout))
        .route("/subscription/checkout/return", get(api::get_subscription_checkout_return))
        .route("/subscription/webhook", post(api::post_subscription_webhook))
        .route("/subscription/portal", post(api::post_subscription_portal))
        .route("/sync/push", post(api::sync_push))
        .route("/sync/pull", get(api::sync_pull))
        .with_state(state)
        .layer({
            let origins = std::env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:1420,http://127.0.0.1:1420".to_string());
            let origins: Vec<_> = origins
                .split(',')
                .filter_map(|o| o.trim().parse().ok())
                .collect();
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PATCH,
                    axum::http::Method::DELETE,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::HeaderName::from_static("x-client"),
                ])
                // The web client's refresh token is an httpOnly cookie, which the
                // browser only sends cross-origin when credentials are allowed.
                // Safe here because allow_origin is an explicit list, never "*".
                .allow_credentials(true)
                // Every authenticated call is preflighted (Authorization and
                // X-Client are non-simple headers). Caching the preflight halves
                // the round-trips on mobile. Chromium caps this at 2 hours.
                .max_age(std::time::Duration::from_secs(7200))
        })
        .layer(TraceLayer::new_for_http())
        // GDPR Art. 32 / OWASP — defence-in-depth security response headers.
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            axum::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::HeaderName::from_static("permissions-policy"),
            axum::http::HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
        ));

    // Retention sweep (GDPR Art. 5(1)(e)). refresh_tokens, deleted_records and
    // subscription_history previously grew for the life of the deployment.
    // Intervals are generous: this is housekeeping, not a hot path.
    {
        tokio::spawn(async move {
            let token_grace_days: i64 = std::env::var("RETENTION_TOKEN_GRACE_DAYS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(7);
            let tombstone_days: i64 = std::env::var("RETENTION_TOMBSTONE_DAYS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(180);
            let history_keep: i64 = std::env::var("RETENTION_HISTORY_PER_USER")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(200);
            let weather_days: i64 = std::env::var("RETENTION_WEATHER_DAYS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(365);
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
            loop {
                ticker.tick().await;
                match db::repos::prune_operational_data(
                    &prune_state, token_grace_days, tombstone_days, history_keep, weather_days,
                ).await {
                    Ok(report) => tracing::info!(
                        refresh_tokens = report.refresh_tokens,
                        deleted_records = report.deleted_records,
                        subscription_history = report.subscription_history,
                        weather_history = report.weather_history,
                        push_reminders_sent = report.push_reminders_sent,
                        "retention sweep complete"
                    ),
                    Err(err) => tracing::warn!("retention sweep failed: {err}"),
                }
                match api::photos::purge_expired(&prune_state).await {
                    Ok((0, _)) => {}
                    Ok((accounts, photos)) => tracing::info!(accounts, photos, "deleted photos after paid plan ended"),
                    Err(err) => tracing::warn!("photo purge failed: {err}"),
                }
            }
        });
    }

    // Push reminders: check for due tasks every few minutes.
    if let Some(config) = push_state.push.clone() {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(services::push::interval_from_env());
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                match services::push::run_due_reminders(&push_state, &config).await {
                    Ok(r) if r.reminders == 0 => {}
                    Ok(r) => tracing::info!(reminders = r.reminders, delivered = r.delivered, failed = r.failed, removed = r.removed, "push reminders sent"),
                    Err(err) => tracing::warn!("push reminder run failed: {err}"),
                }
            }
        });
    }

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("backend listening on 0.0.0.0:8080");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}
