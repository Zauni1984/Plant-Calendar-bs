//! Vouchers: codes that unlock Basic, Pro or Nursery for a number of days or
//! for good (influencers, shop orders, flyers). Created with the operator CLI
//! (services/voucher_cli.rs), redeemed by a signed-in account:
//!
//!   POST /vouchers/redeem   { code }       rate-limited (code guessing)
//!   GET  /vouchers/me                      the account's active voucher
//!
//! A voucher is stored on the account next to the Stripe subscription;
//! users.plan is the better of the two (repos::apply_effective_plan), so
//! cancelling Stripe does not end a voucher and vice versa. An hourly sweep
//! ends expired vouchers. The desktop app is included like in any paid plan.

use axum::{extract::State, http::StatusCode, response::{IntoResponse, Response}, Json};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use shared::SubscriptionPlan;
use uuid::Uuid;

use super::api_error;
use crate::{
    db::{repos::{self, parse_plan, plan_rank, plan_str}, AppState},
    services::auth::SessionUser,
};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const SINGLE_CODE_CHARS: usize = 16;

/// A new single-use code, e.g. `7K2M-QX9D-4HRT-B6WN` (80 bits).
pub fn generate_code(random: [u8; 16]) -> String {
    let mut bits = u128::from_be_bytes(random);
    let mut chars = String::with_capacity(SINGLE_CODE_CHARS + 3);
    for i in 0..SINGLE_CODE_CHARS {
        if i > 0 && i % 4 == 0 {
            chars.push('-');
        }
        chars.push(ALPHABET[(bits & 31) as usize] as char);
        bits >>= 5;
    }
    chars
}

/// Canonical form for hashing, applied alike when a code is created and when
/// it is typed in: upper case, spaces and dashes dropped, look-alikes mapped
/// (O→0, I/L→1). None if it cannot be a code.
pub fn normalize_code(input: &str) -> Option<String> {
    let s: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect();
    ((4..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(s)
}

pub fn code_hash(normalized: &str) -> String {
    Sha256::digest(normalized.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn voucher_error(status: StatusCode, code: &str) -> Response {
    (status, Json(serde_json::json!({ "code": code, "message": code }))).into_response()
}

#[derive(Deserialize)]
pub struct RedeemPayload {
    pub code: String,
}

#[derive(sqlx::FromRow)]
struct VoucherRow {
    voucher_id: Uuid,
    plan: String,
    duration_days: Option<i32>,
    max_redemptions: Option<i32>,
    redemption_count: i32,
    redeem_until: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

/// What a redemption does to the account's voucher, or why it is refused.
/// `current`: the account's voucher if it has not expired.
fn next_voucher(
    current: Option<(SubscriptionPlan, Option<DateTime<Utc>>)>,
    plan: SubscriptionPlan,
    duration: Option<Duration>,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, &'static str> {
    let fresh = duration.map(|d| now + d);
    let Some((old_plan, old_until)) = current else { return Ok(fresh) };
    match plan_rank(plan).cmp(&plan_rank(old_plan)) {
        std::cmp::Ordering::Less => Err("lower_plan"),
        // A higher plan replaces the old voucher, but never trades a
        // permanent one for one that runs out.
        std::cmp::Ordering::Greater if old_until.is_none() && fresh.is_some() => Err("would_replace_unlimited"),
        std::cmp::Ordering::Greater => Ok(fresh),
        std::cmp::Ordering::Equal => match (old_until, duration) {
            (None, _) => Err("already_unlimited"),
            (Some(_), None) => Ok(None),
            // Same plan: the new time is added on top.
            (Some(until), Some(d)) => Ok(Some(until.max(now) + d)),
        },
    }
}

pub async fn redeem(State(state): State<AppState>, session: SessionUser, Json(payload): Json<RedeemPayload>) -> Response {
    let Some(normalized) = normalize_code(&payload.code) else {
        return voucher_error(StatusCode::BAD_REQUEST, "invalid");
    };
    let now = Utc::now();
    let result: anyhow::Result<Result<(SubscriptionPlan, Option<DateTime<Utc>>), &'static str>> = async {
        let mut tx = state.pg.begin().await?;
        let voucher: Option<VoucherRow> = sqlx::query_as(
            "SELECT voucher_id, plan, duration_days, max_redemptions, redemption_count, redeem_until, revoked_at \
             FROM vouchers WHERE code_hash = $1 FOR UPDATE",
        )
        .bind(code_hash(&normalized))
        .fetch_optional(&mut *tx)
        .await?;
        let Some(v) = voucher.filter(|v| v.revoked_at.is_none()) else { return Ok(Err("invalid")) };
        if v.redeem_until.is_some_and(|u| u < now) {
            return Ok(Err("expired"));
        }
        if v.max_redemptions.is_some_and(|m| v.redemption_count >= m) {
            return Ok(Err("used_up"));
        }
        let (old_plan, old_until): (Option<String>, Option<DateTime<Utc>>) =
            sqlx::query_as("SELECT voucher_plan, voucher_until FROM users WHERE user_id = $1 FOR UPDATE")
                .bind(session.user_id)
                .fetch_one(&mut *tx)
                .await?;
        let current = old_plan
            .as_deref()
            .map(parse_plan)
            .filter(|_| old_until.is_none_or(|u| u > now))
            .map(|p| (p, old_until));
        let plan = parse_plan(&v.plan);
        let until = match next_voucher(current, plan, v.duration_days.map(|d| Duration::days(d.into())), now) {
            Ok(until) => until,
            Err(reason) => return Ok(Err(reason)),
        };
        let inserted = sqlx::query("INSERT INTO voucher_redemptions (voucher_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(v.voucher_id)
            .bind(session.user_id)
            .execute(&mut *tx)
            .await?;
        if inserted.rows_affected() == 0 {
            return Ok(Err("already_redeemed"));
        }
        sqlx::query("UPDATE vouchers SET redemption_count = redemption_count + 1 WHERE voucher_id = $1")
            .bind(v.voucher_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE users SET voucher_plan = $2, voucher_until = $3 WHERE user_id = $1")
            .bind(session.user_id)
            .bind(plan_str(plan))
            .bind(until)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Ok((plan, until)))
    }
    .await;

    let (plan, until) = match result {
        Ok(Ok(v)) => v,
        Ok(Err(reason)) => {
            let status = if reason == "invalid" { StatusCode::NOT_FOUND } else { StatusCode::CONFLICT };
            return voucher_error(status, reason);
        }
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let previous = repos::get_user(&state, session.user_id).await.ok().flatten().map(|u| u.plan);
    let effective = match repos::apply_effective_plan(&state, session.user_id).await {
        Ok(p) => p,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let name = match plan {
        SubscriptionPlan::Nursery => "Gärtnerei",
        SubscriptionPlan::Pro => "Pro",
        SubscriptionPlan::Basic => "Basic",
        SubscriptionPlan::Free => "Free",
    };
    let message = match until {
        Some(u) => format!("Gutschein eingelöst: {name} bis {}", u.format("%d.%m.%Y")),
        None => format!("Gutschein eingelöst: {name}, unbegrenzt"),
    };
    let _ = repos::record_subscription_history(&state, session.user_id, "voucher_redeemed", plan, previous, None, Some("voucher"), Some("active"), Some(&message)).await;
    Json(serde_json::json!({
        "voucher_plan": plan_str(plan),
        "voucher_until": until,
        "plan": plan_str(effective),
    }))
    .into_response()
}

pub async fn my_voucher(State(state): State<AppState>, session: SessionUser) -> Response {
    match active_voucher(&state, session.user_id).await {
        Ok(Some((plan, until))) => Json(serde_json::json!({ "plan": plan_str(plan), "until": until })).into_response(),
        Ok(None) => Json(serde_json::Value::Null).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

/// The account's voucher, if it has not expired. Also for the GDPR export.
pub async fn active_voucher(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<(SubscriptionPlan, Option<DateTime<Utc>>)>> {
    let (plan, until): (Option<String>, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT voucher_plan, voucher_until FROM users WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&state.pg)
            .await?
            .unwrap_or((None, None));
    Ok(plan
        .as_deref()
        .map(parse_plan)
        .filter(|_| until.is_none_or(|u| u > Utc::now()))
        .map(|p| (p, until)))
}

/// Hourly: end expired vouchers and fall back to the Stripe plan or Free.
pub async fn expire_vouchers(state: &AppState) -> anyhow::Result<usize> {
    let users: Vec<(Uuid, String)> = sqlx::query_as(
        "UPDATE users SET voucher_plan = NULL, voucher_until = NULL \
         WHERE voucher_plan IS NOT NULL AND voucher_until IS NOT NULL AND voucher_until <= NOW() \
         RETURNING user_id, plan",
    )
    .fetch_all(&state.pg)
    .await?;
    for (user_id, previous) in &users {
        match repos::apply_effective_plan(state, *user_id).await {
            Ok(plan) => {
                let _ = repos::record_subscription_history(state, *user_id, "voucher_expired", plan, Some(parse_plan(previous)), None, Some("voucher"), Some("expired"), Some("Gutschein abgelaufen.")).await;
            }
            Err(err) => tracing::warn!(%user_id, "voucher expiry: could not apply plan: {err}"),
        }
    }
    Ok(users.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use SubscriptionPlan::*;

    #[test]
    fn codes_survive_typing_and_normalize_alike() {
        let code = generate_code([7; 16]);
        assert_eq!(code.len(), 19, "{code}");
        let n = normalize_code(&code).unwrap();
        assert_eq!(normalize_code(&code.to_lowercase().replace('-', " ")), Some(n.clone()));
        // Readable shared codes: the same mapping at creation and redemption.
        assert_eq!(normalize_code("Fruehling26"), normalize_code("FRUEH-LING-26"));
        assert_eq!(normalize_code("abc"), None);
        assert_eq!(normalize_code("GRÜN2026"), None);
        assert_ne!(code_hash(&n), code_hash("OTHER"));
    }

    #[test]
    fn redemption_rules() {
        let now = Utc::now();
        let d30 = Some(Duration::days(30));
        // No voucher yet.
        assert_eq!(next_voucher(None, Pro, d30, now), Ok(Some(now + Duration::days(30))));
        assert_eq!(next_voucher(None, Pro, None, now), Ok(None));
        // Same plan: time adds up.
        let later = now + Duration::days(10);
        assert_eq!(next_voucher(Some((Pro, Some(later))), Pro, d30, now), Ok(Some(later + Duration::days(30))));
        assert_eq!(next_voucher(Some((Pro, Some(later))), Pro, None, now), Ok(None));
        assert_eq!(next_voucher(Some((Pro, None)), Pro, d30, now), Err("already_unlimited"));
        // Lower plan refused, higher plan replaces.
        assert_eq!(next_voucher(Some((Pro, Some(later))), Basic, d30, now), Err("lower_plan"));
        assert_eq!(next_voucher(Some((Basic, Some(later))), Nursery, d30, now), Ok(Some(now + Duration::days(30))));
        // A permanent voucher is never traded for a temporary one.
        assert_eq!(next_voucher(Some((Basic, None)), Pro, d30, now), Err("would_replace_unlimited"));
        assert_eq!(next_voucher(Some((Basic, None)), Pro, None, now), Ok(None));
    }
}
