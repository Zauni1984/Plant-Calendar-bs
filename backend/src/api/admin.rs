//! Admin area API (used by admin.sowling.com).
//!
//!   POST   /admin/login                 email, password, authenticator code
//!   GET    /admin/stats                 figures and time series for the dashboard
//!   GET    /admin/users?q=&offset=      accounts
//!   POST   /admin/users/{id}/disable    { reason }
//!   POST   /admin/users/{id}/enable
//!   DELETE /admin/users/{id}            { confirm_email }
//!   POST   /admin/users/{id}/plan       { plan, months | days | unlimited }
//!   GET    /admin/vouchers              code batches
//!   POST   /admin/vouchers              create codes
//!   POST   /admin/vouchers/{batch}/revoke
//!   GET    /admin/invoices/problems     invoices not sent
//!   POST   /admin/invoices/{id}/retry
//!   GET    /admin/audit                 who did what
//!
//! Admins are accounts made admin on the server (`sowling-backend admin
//! grant`), which also sets up their authenticator app. An admin token is a
//! separate, short-lived JWT; every request checks again that the account is
//! still an admin and not disabled. Every change is written to admin_audit.

use axum::{
    extract::{FromRequestParts, Path, Query, State},
    http::{request::Parts, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use shared::SubscriptionPlan;

use super::{api_error, vouchers};
use crate::{db::{repos, AppState}, services::totp};

const TOKEN_MINUTES: i64 = 30;

#[derive(Serialize, Deserialize)]
struct AdminClaims {
    sub: String,
    adm: bool,
    exp: usize,
}

/// Admin tokens are signed with a key derived from JWT_SECRET, so a user
/// token can never pass as an admin token or the other way round.
fn admin_key(state: &AppState) -> Vec<u8> {
    format!("{}::sowling-admin", state.jwt_secret).into_bytes()
}

pub struct AdminUser {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let unauthorized = || api_error(StatusCode::UNAUTHORIZED, "admin sign-in required");
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(unauthorized)?;
        let data = decode::<AdminClaims>(token, &DecodingKey::from_secret(&admin_key(state)), &Validation::new(jsonwebtoken::Algorithm::HS256))
            .map_err(|_| unauthorized())?;
        let user_id = Uuid::parse_str(&data.claims.sub).map_err(|_| unauthorized())?;
        if !data.claims.adm {
            return Err(unauthorized());
        }
        let still_admin: Option<bool> = sqlx::query_scalar(
            "SELECT u.disabled_at IS NULL FROM admins a JOIN users u ON u.user_id = a.user_id WHERE a.user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&state.pg)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        match still_admin {
            Some(true) => Ok(AdminUser { user_id }),
            _ => Err(unauthorized()),
        }
    }
}

async fn audit(state: &AppState, admin: Uuid, action: &str, target: Option<String>, detail: Option<String>) {
    let result = sqlx::query("INSERT INTO admin_audit (audit_id, admin_id, action, target, detail) VALUES ($1,$2,$3,$4,$5)")
        .bind(Uuid::new_v4())
        .bind(admin)
        .bind(action)
        .bind(target)
        .bind(detail)
        .execute(&state.pg)
        .await;
    if let Err(err) = result {
        tracing::error!("admin audit write failed: {err}");
    }
}

// --- login ----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginPayload {
    email: String,
    password: String,
    code: String,
}

pub async fn login(State(state): State<AppState>, Json(p): Json<LoginPayload>) -> Response {
    // One answer for every failure: nothing reveals which part was wrong.
    let denied = || api_error(StatusCode::UNAUTHORIZED, "invalid credentials or code");
    if p.email.len() > 320 || p.password.is_empty() || p.password.len() > 256 {
        return denied();
    }
    let user = match repos::get_user_by_email(&state, &p.email).await {
        Ok(Some(u)) => u,
        Ok(None) => return denied(),
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let row: Option<(String, i64, Option<DateTime<Utc>>)> = match sqlx::query_as(
        "SELECT a.totp_secret, a.totp_last_step, u.disabled_at FROM admins a JOIN users u ON u.user_id = a.user_id WHERE a.user_id = $1",
    )
    .bind(user.user_id)
    .fetch_optional(&state.pg)
    .await
    {
        Ok(r) => r,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let Some((secret, last_step, None)) = row else { return denied() };
    let hash = match repos::get_password_hash_by_user_id(&state, user.user_id).await {
        Ok(Some(h)) => h,
        _ => return denied(),
    };
    if !crate::services::auth::verify_password(&p.password, &hash) {
        return denied();
    }
    let Some(step) = totp::verify(&secret, &p.code, Utc::now().timestamp(), last_step) else {
        return denied();
    };
    // Spend the code; a parallel login with the same code loses here.
    let spent = sqlx::query("UPDATE admins SET totp_last_step = $2 WHERE user_id = $1 AND totp_last_step < $2")
        .bind(user.user_id)
        .bind(step)
        .execute(&state.pg)
        .await;
    if !matches!(spent, Ok(r) if r.rows_affected() == 1) {
        return denied();
    }
    let expires = Utc::now() + Duration::minutes(TOKEN_MINUTES);
    let claims = AdminClaims { sub: user.user_id.to_string(), adm: true, exp: expires.timestamp() as usize };
    let token = match encode(&Header::default(), &claims, &EncodingKey::from_secret(&admin_key(&state))) {
        Ok(t) => t,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    audit(&state, user.user_id, "login", None, None).await;
    Json(serde_json::json!({ "token": token, "expires_at": expires })).into_response()
}

// --- statistics -------------------------------------------------------------------------

async fn scalar(state: &AppState, sql: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql).fetch_one(&state.pg).await.unwrap_or_else(|err| {
        tracing::warn!("admin stats query failed: {err}");
        0
    })
}

async fn series(state: &AppState, sql: &str) -> Vec<(String, i64)> {
    sqlx::query_as::<_, (String, i64)>(sql).fetch_all(&state.pg).await.unwrap_or_else(|err| {
        tracing::warn!("admin stats query failed: {err}");
        vec![]
    })
}

pub async fn stats(State(state): State<AppState>, _admin: AdminUser) -> Response {
    // Accounts = users with an e-mail address (anonymous desktop users have none).
    const ACCOUNTS: &str = "FROM users WHERE email IS NOT NULL";
    let totals = serde_json::json!({
        "accounts": scalar(&state, &format!("SELECT COUNT(*) {ACCOUNTS}")).await,
        "new_7d": scalar(&state, &format!("SELECT COUNT(*) {ACCOUNTS} AND created_at >= NOW() - INTERVAL '7 days'")).await,
        "new_30d": scalar(&state, &format!("SELECT COUNT(*) {ACCOUNTS} AND created_at >= NOW() - INTERVAL '30 days'")).await,
        "active_7d": scalar(&state, &format!("SELECT COUNT(*) {ACCOUNTS} AND last_seen_at >= NOW() - INTERVAL '7 days'")).await,
        "active_30d": scalar(&state, &format!("SELECT COUNT(*) {ACCOUNTS} AND last_seen_at >= NOW() - INTERVAL '30 days'")).await,
        "disabled": scalar(&state, &format!("SELECT COUNT(*) {ACCOUNTS} AND disabled_at IS NOT NULL")).await,
        "paying": scalar(&state, "SELECT COUNT(*) FROM users WHERE stripe_plan IS NOT NULL").await,
        "voucher_active": scalar(&state, "SELECT COUNT(*) FROM users WHERE voucher_plan IS NOT NULL AND (voucher_until IS NULL OR voucher_until > NOW())").await,
        "plants": scalar(&state, "SELECT COUNT(*) FROM plants").await,
        "plants_active": scalar(&state, "SELECT COUNT(*) FROM plants WHERE plant_status = 'active'").await,
        "photos": scalar(&state, "SELECT COUNT(*) FROM photos").await,
        "push_subscriptions": scalar(&state, "SELECT COUNT(*) FROM push_subscriptions").await,
        "desktop_devices": scalar(&state, "SELECT COUNT(*) FROM license_activations").await,
        "desktop_purchases": scalar(&state, "SELECT COUNT(*) FROM licenses WHERE kind = 'purchase' AND revoked_at IS NULL").await,
        "vouchers_redeemed": scalar(&state, "SELECT COUNT(*) FROM voucher_redemptions").await,
        "revenue_30d_cents": scalar(&state, "SELECT COALESCE(SUM(amount_cents), 0)::BIGINT FROM invoices WHERE paid_at >= NOW() - INTERVAL '30 days'").await,
        "revenue_total_cents": scalar(&state, "SELECT COALESCE(SUM(amount_cents), 0)::BIGINT FROM invoices").await,
        "invoice_problems": scalar(&state, "SELECT COUNT(*) FROM invoices WHERE status = 'failed' OR (status IN ('pending', 'created') AND attempts > 0)").await,
    });
    let signups = series(&state, &format!(
        "SELECT to_char(d, 'YYYY-MM-DD'), COUNT(u.user_id) \
         FROM generate_series(date_trunc('day', NOW()) - INTERVAL '89 days', date_trunc('day', NOW()), INTERVAL '1 day') d \
         LEFT JOIN users u ON u.email IS NOT NULL AND date_trunc('day', u.created_at) = d GROUP BY d ORDER BY d"
    ))
    .await;
    let active = series(&state,
        "SELECT to_char(d, 'YYYY-MM-DD'), COUNT(u.user_id) \
         FROM generate_series(date_trunc('day', NOW()) - INTERVAL '29 days', date_trunc('day', NOW()), INTERVAL '1 day') d \
         LEFT JOIN users u ON u.email IS NOT NULL AND date_trunc('day', u.last_seen_at) = d GROUP BY d ORDER BY d",
    )
    .await;
    let plans = series(&state, &format!("SELECT plan, COUNT(*) {ACCOUNTS} GROUP BY plan ORDER BY plan")).await;
    let revenue = series(&state,
        "SELECT to_char(m, 'YYYY-MM'), COALESCE(SUM(i.amount_cents), 0)::BIGINT \
         FROM generate_series(date_trunc('month', NOW()) - INTERVAL '11 months', date_trunc('month', NOW()), INTERVAL '1 month') m \
         LEFT JOIN invoices i ON date_trunc('month', i.paid_at) = m GROUP BY m ORDER BY m",
    )
    .await;
    let revenue_by_kind = series(&state, "SELECT kind, COALESCE(SUM(amount_cents), 0)::BIGINT FROM invoices WHERE paid_at >= NOW() - INTERVAL '12 months' GROUP BY kind").await;
    let pair = |v: Vec<(String, i64)>| v.into_iter().map(|(k, n)| serde_json::json!([k, n])).collect::<Vec<_>>();
    Json(serde_json::json!({
        "totals": totals,
        "signups_per_day": pair(signups),
        "active_per_day": pair(active),
        "plans": pair(plans),
        "revenue_per_month": pair(revenue),
        "revenue_by_kind": pair(revenue_by_kind),
        "generated_at": Utc::now(),
    }))
    .into_response()
}

// --- accounts -------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct UserQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    offset: i64,
}

#[derive(sqlx::FromRow, Serialize)]
struct UserRow {
    user_id: Uuid,
    email: Option<String>,
    created_at: DateTime<Utc>,
    last_seen_at: Option<DateTime<Utc>>,
    plan: String,
    stripe_plan: Option<String>,
    voucher_plan: Option<String>,
    voucher_until: Option<DateTime<Utc>>,
    disabled_at: Option<DateTime<Utc>>,
    disabled_reason: Option<String>,
    is_admin: bool,
    plants: i64,
    invoices: i64,
}

pub async fn list_users(State(state): State<AppState>, _admin: AdminUser, Query(q): Query<UserQuery>) -> Response {
    let pattern = format!("%{}%", q.q.trim().to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let rows: Result<Vec<UserRow>, _> = sqlx::query_as(
        "SELECT u.user_id, u.email, u.created_at, u.last_seen_at, u.plan, u.stripe_plan, u.voucher_plan, u.voucher_until, \
                u.disabled_at, u.disabled_reason, (a.user_id IS NOT NULL) AS is_admin, \
                (SELECT COUNT(*) FROM plants p WHERE p.user_id = u.user_id) AS plants, \
                (SELECT COUNT(*) FROM invoices i WHERE i.user_id = u.user_id) AS invoices \
         FROM users u LEFT JOIN admins a ON a.user_id = u.user_id \
         WHERE u.email IS NOT NULL AND ($1 = '%%' OR lower(u.email) LIKE $1 OR u.user_id::text LIKE $1) \
         ORDER BY u.created_at DESC LIMIT 50 OFFSET $2",
    )
    .bind(&pattern)
    .bind(q.offset.max(0))
    .fetch_all(&state.pg)
    .await;
    match rows {
        Ok(rows) => Json(rows).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize, Default)]
pub struct DisablePayload {
    #[serde(default)]
    reason: String,
}

async fn target_email(state: &AppState, id: Uuid) -> Result<String, Response> {
    match repos::get_user_email(state, id).await {
        Ok(Some(e)) => Ok(e),
        Ok(None) => Err(api_error(StatusCode::NOT_FOUND, "account not found")),
        Err(err) => Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, err)),
    }
}

pub async fn disable_user(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>, Json(p): Json<DisablePayload>) -> Response {
    if id == admin.user_id {
        return api_error(StatusCode::BAD_REQUEST, "you cannot disable yourself");
    }
    let email = match target_email(&state, id).await {
        Ok(e) => e,
        Err(r) => return r,
    };
    let reason: String = p.reason.trim().chars().take(500).collect();
    let result = async {
        sqlx::query("UPDATE users SET disabled_at = NOW(), disabled_reason = $2 WHERE user_id = $1")
            .bind(id)
            .bind(&reason)
            .execute(&state.pg)
            .await?;
        // Sign out everywhere: no session can be renewed any more. An access
        // token already handed out stays valid for at most its 60 minutes.
        sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1").bind(id).execute(&state.pg).await?;
        anyhow::Ok(())
    }
    .await;
    if let Err(err) = result {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }
    audit(&state, admin.user_id, "disable", Some(email), Some(reason)).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn enable_user(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>) -> Response {
    let email = match target_email(&state, id).await {
        Ok(e) => e,
        Err(r) => return r,
    };
    if let Err(err) = sqlx::query("UPDATE users SET disabled_at = NULL, disabled_reason = NULL WHERE user_id = $1").bind(id).execute(&state.pg).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }
    audit(&state, admin.user_id, "enable", Some(email), None).await;
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
pub struct DeletePayload {
    confirm_email: String,
}

pub async fn delete_user(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>, Json(p): Json<DeletePayload>) -> Response {
    if id == admin.user_id {
        return api_error(StatusCode::BAD_REQUEST, "you cannot delete yourself");
    }
    let email = match target_email(&state, id).await {
        Ok(e) => e,
        Err(r) => return r,
    };
    // Typing the address guards against deleting the wrong row.
    if !p.confirm_email.trim().eq_ignore_ascii_case(&email) {
        return api_error(StatusCode::BAD_REQUEST, "confirmation does not match the account's e-mail");
    }
    if let Err(err) = super::erase_account(&state, id).await {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }
    audit(&state, admin.user_id, "delete", Some(email), None).await;
    StatusCode::NO_CONTENT.into_response()
}

// --- plans ------------------------------------------------------------------------------

/// How long a plan or code applies: exactly one of the three.
#[derive(Deserialize)]
pub struct DurationInput {
    months: Option<i32>,
    days: Option<i32>,
    #[serde(default)]
    unlimited: bool,
}

/// Some(None): unlimited; Some(Some(days)); None: invalid.
fn duration_days(d: &DurationInput) -> Option<Option<i32>> {
    match (d.months, d.days, d.unlimited) {
        (Some(m), None, false) if (1..=120).contains(&m) => Some(Some(vouchers::months_to_days(m))),
        (None, Some(n), false) if (1..=3650).contains(&n) => Some(Some(n)),
        (None, None, true) => Some(None),
        _ => None,
    }
}

fn paid_plan(name: &str) -> Option<SubscriptionPlan> {
    match name {
        "basic" | "pro" | "nursery" => Some(repos::parse_plan(name)),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct PlanPayload {
    plan: String,
    #[serde(flatten)]
    duration: DurationInput,
}

/// Grant a plan by hand, stored like a voucher next to the Stripe
/// subscription: the account gets the better of the two. "free" removes the
/// grant; a running Stripe subscription stays and has to be cancelled in
/// Stripe.
pub async fn set_plan(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>, Json(p): Json<PlanPayload>) -> Response {
    let email = match target_email(&state, id).await {
        Ok(e) => e,
        Err(r) => return r,
    };
    let (plan, until) = if p.plan == "free" {
        (None, None)
    } else {
        let Some(plan) = paid_plan(&p.plan) else { return api_error(StatusCode::BAD_REQUEST, "plan must be free, basic, pro or nursery") };
        let Some(days) = duration_days(&p.duration) else { return api_error(StatusCode::BAD_REQUEST, "give months (1-120), days (1-3650) or unlimited") };
        (Some(plan), days.map(|d| Utc::now() + Duration::days(d.into())))
    };
    let result = async {
        let previous = repos::get_user(&state, id).await?.map(|u| u.plan);
        sqlx::query("UPDATE users SET voucher_plan = $2, voucher_until = $3 WHERE user_id = $1")
            .bind(id)
            .bind(plan.map(repos::plan_str))
            .bind(until)
            .execute(&state.pg)
            .await?;
        let effective = repos::apply_effective_plan(&state, id).await?;
        let stripe: Option<String> = sqlx::query_scalar("SELECT stripe_plan FROM users WHERE user_id = $1").bind(id).fetch_one(&state.pg).await?;
        anyhow::Ok((previous, effective, stripe))
    }
    .await;
    let (previous, effective, stripe) = match result {
        Ok(r) => r,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let detail = match (plan, until) {
        (None, _) => "free".to_string(),
        (Some(pl), Some(u)) => format!("{} until {}", repos::plan_str(pl), u.format("%Y-%m-%d")),
        (Some(pl), None) => format!("{} unlimited", repos::plan_str(pl)),
    };
    let message = format!("Tarif durch Admin gesetzt: {detail}");
    let _ = repos::record_subscription_history(&state, id, "admin_plan_set", effective, previous, None, Some("admin"), Some("active"), Some(&message)).await;
    audit(&state, admin.user_id, "set_plan", Some(email), Some(detail)).await;
    Json(serde_json::json!({ "plan": repos::plan_str(effective), "stripe_plan": stripe, "voucher_until": until })).into_response()
}

// --- vouchers ---------------------------------------------------------------------------

pub async fn list_vouchers(State(state): State<AppState>, _admin: AdminUser) -> Response {
    match vouchers::list_batches(&state, true).await {
        Ok(mut rows) => {
            rows.reverse(); // newest first
            Json(rows).into_response()
        }
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct VoucherPayload {
    plan: String,
    #[serde(flatten)]
    duration: DurationInput,
    /// A readable code for many accounts; empty or missing: single codes.
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    max_redemptions: Option<i32>,
    /// Last day to redeem, YYYY-MM-DD.
    #[serde(default)]
    redeem_until: Option<String>,
    #[serde(default)]
    note: String,
    /// Single code only: send it to this address.
    #[serde(default)]
    email: Option<String>,
}

pub async fn create_vouchers(State(state): State<AppState>, admin: AdminUser, Json(p): Json<VoucherPayload>) -> Response {
    let bad = |msg: &str| api_error(StatusCode::BAD_REQUEST, msg);
    let Some(plan) = paid_plan(&p.plan) else { return bad("plan must be basic, pro or nursery") };
    let Some(duration_days) = duration_days(&p.duration) else { return bad("give months (1-120), days (1-3650) or unlimited") };
    let shared_code = p.code.as_deref().map(str::trim).filter(|c| !c.is_empty()).map(str::to_string);
    let count = p.count.unwrap_or(1);
    if shared_code.is_none() && !(1..=vouchers::MAX_BATCH).contains(&count) {
        return bad("count must be 1-10000");
    }
    if p.max_redemptions.is_some_and(|m| m < 1) {
        return bad("max_redemptions must be at least 1");
    }
    let redeem_until = match p.redeem_until.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => match chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            // Redeemable through the whole of that day (UTC).
            Ok(date) => Some(date.and_hms_opt(23, 59, 59).expect("valid time").and_utc()),
            Err(_) => return bad("redeem_until must be YYYY-MM-DD"),
        },
        None => None,
    };
    if redeem_until.is_some_and(|u| u < Utc::now()) {
        return bad("redeem_until lies in the past");
    }
    let email = p.email.as_deref().map(str::trim).filter(|e| !e.is_empty()).map(str::to_string);
    if let Some(e) = &email {
        if !e.contains('@') || e.len() > 254 {
            return bad("email is not an e-mail address");
        }
        if shared_code.is_some() || count != 1 {
            return bad("email sends one single code");
        }
        if state.mail.is_none() {
            return bad("e-mail is not set up on the server (SMTP)");
        }
    }
    let note: String = p.note.trim().chars().take(200).collect();
    let batch = vouchers::NewBatch {
        plan,
        duration_days,
        shared_code: shared_code.clone(),
        count,
        max_redemptions: p.max_redemptions,
        redeem_until,
        note: note.clone(),
    };
    let (batch_id, codes) = match vouchers::create_batch(&state, &batch).await {
        Ok(created) => created,
        Err(vouchers::BatchError::BadCode) => return bad("code: 4-32 letters or digits, dashes allowed, no umlauts"),
        Err(vouchers::BatchError::Exists(code)) => return api_error(StatusCode::CONFLICT, format!("the code {code} exists already")),
        Err(vouchers::BatchError::Db(err)) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let mut emailed = false;
    if let (Some(to), Some(mailer)) = (&email, &state.mail) {
        let (plan_de, plan_en) = crate::services::voucher_cli::plan_names(plan);
        let (dur_de, dur_en) = crate::services::voucher_cli::duration_text(duration_days, p.duration.months);
        match mailer.send(to, &crate::services::mail::voucher_mail(&codes[0], plan_de, plan_en, &dur_de, &dur_en)).await {
            Ok(()) => emailed = true,
            Err(err) => tracing::warn!("voucher mail failed: {err}"),
        }
    }
    let detail = format!(
        "{}, {}, {}{}{}",
        if shared_code.is_some() { "shared code".to_string() } else { format!("{} single", codes.len()) },
        repos::plan_str(plan),
        duration_days.map(|d| format!("{d} days")).unwrap_or_else(|| "unlimited".into()),
        if note.is_empty() { String::new() } else { format!(", {note}") },
        if emailed { format!(", sent to {}", email.as_deref().unwrap_or_default()) } else { String::new() },
    );
    audit(&state, admin.user_id, "voucher_create", Some(batch_id.to_string()), Some(detail)).await;
    Json(serde_json::json!({ "batch_id": batch_id, "codes": codes, "emailed": emailed })).into_response()
}

pub async fn revoke_vouchers(State(state): State<AppState>, admin: AdminUser, Path(batch): Path<Uuid>) -> Response {
    match vouchers::revoke_batch(&state, batch).await {
        Ok(n) => {
            audit(&state, admin.user_id, "voucher_revoke", Some(batch.to_string()), Some(format!("{n} code(s)"))).await;
            Json(serde_json::json!({ "revoked": n })).into_response()
        }
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

// --- invoices ---------------------------------------------------------------------------

#[derive(sqlx::FromRow, Serialize)]
struct InvoiceProblem {
    invoice_id: Uuid,
    stripe_ref: String,
    kind: String,
    email: Option<String>,
    amount_cents: i64,
    paid_at: DateTime<Utc>,
    status: String,
    attempts: i32,
    last_error: Option<String>,
    next_attempt_at: DateTime<Utc>,
}

pub async fn invoice_problems(State(state): State<AppState>, _admin: AdminUser) -> Response {
    let rows: Result<Vec<InvoiceProblem>, _> = sqlx::query_as(
        "SELECT invoice_id, stripe_ref, kind, email, amount_cents, paid_at, status, attempts, last_error, next_attempt_at \
         FROM invoices WHERE status IN ('failed', 'pending', 'created') ORDER BY paid_at DESC LIMIT 200",
    )
    .fetch_all(&state.pg)
    .await;
    match rows {
        Ok(rows) => Json(rows).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn retry_invoice(State(state): State<AppState>, admin: AdminUser, Path(id): Path<Uuid>) -> Response {
    // A failed invoice goes back to where it stopped: created in Lexware or not.
    let updated = sqlx::query(
        "UPDATE invoices SET status = CASE WHEN lexoffice_id IS NULL THEN 'pending' ELSE 'created' END, \
            attempts = 0, next_attempt_at = NOW() \
         WHERE invoice_id = $1 AND status IN ('failed', 'pending', 'created') RETURNING stripe_ref",
    )
    .bind(id)
    .fetch_optional(&state.pg)
    .await;
    match updated {
        Ok(Some(row)) => {
            use sqlx::Row;
            audit(&state, admin.user_id, "invoice_retry", Some(row.get::<String, _>(0)), None).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(None) => api_error(StatusCode::NOT_FOUND, "no open invoice with this id"),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(sqlx::FromRow, Serialize)]
struct AuditRow {
    at: DateTime<Utc>,
    admin_email: Option<String>,
    action: String,
    target: Option<String>,
    detail: Option<String>,
}

pub async fn audit_log(State(state): State<AppState>, _admin: AdminUser) -> Response {
    let rows: Result<Vec<AuditRow>, _> = sqlx::query_as(
        "SELECT l.at, u.email AS admin_email, l.action, l.target, l.detail \
         FROM admin_audit l LEFT JOIN users u ON u.user_id = l.admin_id ORDER BY l.at DESC LIMIT 200",
    )
    .fetch_all(&state.pg)
    .await;
    match rows {
        Ok(rows) => Json(rows).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}
