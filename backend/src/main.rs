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
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = AppState::from_env().await?;

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
        .route("/me/consents", get(api::list_consents).post(api::upsert_consent))
        .route("/me/adult-confirmation", patch(api::patch_adult_confirmation))
        .route("/locations", get(api::list_locations).post(api::create_location))
        .route("/locations/{id}", patch(api::update_location).delete(api::delete_location))
        .route("/plants", get(api::list_plants).post(api::create_plant))
        .route("/plants/{id}", get(api::get_plant).patch(api::update_plant).delete(api::delete_plant))
        .route("/plants/{id}/seasons", get(api::list_seasons_for_plant).post(api::create_season_for_plant))
        .route("/seasons/{id}", patch(api::update_season).delete(api::delete_season))
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
                ])
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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("backend listening on 0.0.0.0:8080");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}
