//! Operator tool for vouchers (influencers, shop orders, flyers).
//!
//!   sowling-backend voucher create --plan basic|pro|nursery
//!                                  (--months N | --days N | --unlimited)
//!                                  [--count N | --code TEXT [--max N]]
//!                                  [--until YYYY-MM-DD] [--note TEXT] [--email ADDRESS]
//!   sowling-backend voucher list [--all]
//!   sowling-backend voucher revoke <batch-id | code>
//!
//! Without --code: N single-use codes (default 1), printed once, one per line
//! on stdout so they can go straight into a file. With --code: one shared code
//! for many accounts, at most --max of them. --until: last day to redeem.
//! --email (single code only): send the code there (needs SMTP).

use chrono::{NaiveDate, Utc};
use shared::SubscriptionPlan;
use uuid::Uuid;

use crate::{
    api::vouchers::{code_hash, create_batch, list_batches, months_to_days, normalize_code, revoke_batch, BatchError, NewBatch, MAX_BATCH},
    db::{repos::parse_plan, AppState},
};

const USAGE: &str = "usage:
  voucher create --plan basic|pro|nursery (--months N | --days N | --unlimited)
                 [--count N | --code TEXT [--max N]] [--until YYYY-MM-DD] [--note TEXT] [--email ADDRESS]
  voucher list [--all]
  voucher revoke <batch-id | code>

examples:
  voucher create --plan pro --months 3 --count 500 --note \"Hanfjack orders\" > hanfjack.txt
  voucher create --plan basic --months 1 --code FRUEHLING26 --max 1000 --until 2027-05-31 --note \"Flyer spring\"
  voucher create --plan nursery --unlimited --email someone@example.com --note \"Influencer\"";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

pub async fn run(state: &AppState, args: Vec<String>) -> anyhow::Result<()> {
    match args.first().map(String::as_str) {
        Some("create") => create(state, &args[1..]).await,
        Some("list") => list(state, has(&args, "--all")).await,
        Some("revoke") => revoke(state, args.get(1).ok_or_else(|| anyhow::anyhow!(USAGE))?).await,
        _ => {
            println!("{USAGE}");
            Ok(())
        }
    }
}

pub fn plan_names(plan: SubscriptionPlan) -> (&'static str, &'static str) {
    match plan {
        SubscriptionPlan::Basic => ("Basic", "Basic"),
        SubscriptionPlan::Pro => ("Pro", "Pro"),
        SubscriptionPlan::Nursery => ("Gärtnerei", "Nursery"),
        SubscriptionPlan::Free => ("Free", "Free"),
    }
}

pub fn duration_text(days: Option<i32>, months: Option<i32>) -> (String, String) {
    match (days, months) {
        (None, _) => ("unbegrenzt".into(), "unlimited".into()),
        (Some(_), Some(1)) => ("1 Monat".into(), "1 month".into()),
        (Some(_), Some(m)) => (format!("{m} Monate"), format!("{m} months")),
        (Some(d), None) => (format!("{d} Tage"), format!("{d} days")),
    }
}

async fn create(state: &AppState, args: &[String]) -> anyhow::Result<()> {
    let plan = match flag(args, "--plan").as_deref().map(|p| p.trim().to_lowercase()) {
        Some(p) if matches!(p.as_str(), "basic" | "pro" | "nursery" | "gaertnerei" | "gärtnerei") => {
            parse_plan(if p.starts_with('g') { "nursery" } else { &p })
        }
        _ => anyhow::bail!("--plan must be basic, pro or nursery\n\n{USAGE}"),
    };
    let positive = |name: &str, max: i32| -> anyhow::Result<Option<i32>> {
        flag(args, name)
            .map(|v| v.parse::<i32>().ok().filter(|n| (1..=max).contains(n)).ok_or_else(|| anyhow::anyhow!("{name} must be 1-{max}")))
            .transpose()
    };
    let months = positive("--months", 120)?;
    let days = positive("--days", 3650)?;
    let unlimited = has(args, "--unlimited");
    let duration_days = match (months, days, unlimited) {
        (Some(m), None, false) => Some(months_to_days(m)),
        (None, Some(d), false) => Some(d),
        (None, None, true) => None,
        _ => anyhow::bail!("give exactly one of --months, --days, --unlimited\n\n{USAGE}"),
    };
    let shared_code = flag(args, "--code");
    let count = match flag(args, "--count") {
        Some(n) => n.parse::<u32>().ok().filter(|n| (1..=MAX_BATCH).contains(n)).ok_or_else(|| anyhow::anyhow!("--count must be 1-{MAX_BATCH}"))?,
        None => 1,
    };
    let max = positive("--max", 1_000_000)?;
    if shared_code.is_some() && has(args, "--count") {
        anyhow::bail!("--count is for single codes, --code for one shared code; not both");
    }
    if shared_code.is_none() && max.is_some() {
        anyhow::bail!("--max only applies to a shared --code (single codes work once)");
    }
    let redeem_until = match flag(args, "--until") {
        Some(d) => {
            let date = NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|_| anyhow::anyhow!("--until must be YYYY-MM-DD"))?;
            // Redeemable through the whole of that day (UTC).
            Some(date.and_hms_opt(23, 59, 59).expect("valid time").and_utc())
        }
        None => None,
    };
    if redeem_until.is_some_and(|u| u < Utc::now()) {
        anyhow::bail!("--until lies in the past");
    }
    let email = flag(args, "--email");
    if let Some(e) = &email {
        if !e.contains('@') || e.len() > 254 {
            anyhow::bail!("--email is not an e-mail address");
        }
        if shared_code.is_some() || count != 1 {
            anyhow::bail!("--email sends one single code; leave out --code and --count");
        }
        if state.mail.is_none() {
            anyhow::bail!("--email needs SMTP_HOST, SMTP_USER and SMTP_PASSWORD");
        }
    }
    let note = flag(args, "--note").unwrap_or_default();
    let batch = NewBatch { plan, duration_days, shared_code: shared_code.clone(), count, max_redemptions: max, redeem_until, note };
    let (batch_id, codes) = match create_batch(state, &batch).await {
        Ok(created) => created,
        Err(BatchError::BadCode) => anyhow::bail!("--code: 4-32 letters/digits (dashes allowed), no umlauts"),
        Err(BatchError::Exists(code)) => anyhow::bail!("the code {code} exists already; nothing was created"),
        Err(BatchError::Db(err)) => return Err(err),
    };

    let (plan_de, plan_en) = plan_names(plan);
    let (dur_de, dur_en) = duration_text(duration_days, months);
    // Summary on stderr, codes alone on stdout: `> codes.txt` gives a clean list.
    eprintln!("batch {batch_id}: {} code(s), {plan_en}, {dur_en}{}{}", codes.len(),
        redeem_until.map(|u| format!(", redeem by {}", u.format("%Y-%m-%d"))).unwrap_or_default(),
        match (&shared_code, max) { (Some(_), Some(m)) => format!(", up to {m} accounts"), (Some(_), None) => ", any number of accounts".into(), _ => String::new() });
    for code in &codes {
        println!("{code}");
    }
    if shared_code.is_none() {
        eprintln!("Single codes are shown only now; they are not stored in readable form.");
    }
    if let (Some(to), Some(mailer)) = (&email, &state.mail) {
        mailer.send(to, &crate::services::mail::voucher_mail(&codes[0], plan_de, plan_en, &dur_de, &dur_en)).await?;
        eprintln!("Code sent by e-mail to {to}.");
    }
    Ok(())
}

async fn list(state: &AppState, all: bool) -> anyhow::Result<()> {
    let rows = list_batches(state, all).await?;
    if rows.is_empty() {
        println!("no vouchers");
        return Ok(());
    }
    println!("{:<36}  {:<14}  {:<7}  {:<10}  {:<14}  {:<10}  {:<10}  note", "batch", "code", "plan", "duration", "redeemed", "redeem by", "created");
    for r in rows {
        let code = match (r.kind.as_str(), r.shared_code) {
            ("shared", Some(c)) => c,
            _ => format!("{} single", r.codes),
        };
        let redeemed = match (r.kind.as_str(), r.max_redemptions) {
            ("shared", Some(m)) => format!("{}/{m}", r.redeemed),
            ("shared", None) => r.redeemed.to_string(),
            _ => format!("{}/{}", r.redeemed, r.codes),
        };
        let revoked = if r.revoked == r.codes { " (revoked)" } else if r.revoked > 0 { " (partly revoked)" } else { "" };
        println!(
            "{:<36}  {:<14}  {:<7}  {:<10}  {:<14}  {:<10}  {:<10}  {}{}",
            r.batch_id,
            code,
            r.plan,
            r.duration_days.map(|d| format!("{d} days")).unwrap_or_else(|| "unlimited".into()),
            redeemed,
            r.redeem_until.map(|u| u.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "-".into()),
            r.created_at.format("%Y-%m-%d"),
            r.note,
            revoked,
        );
    }
    Ok(())
}

/// Revoked codes can no longer be redeemed. Accounts that already redeemed
/// one keep their voucher until it runs out.
async fn revoke(state: &AppState, target: &str) -> anyhow::Result<()> {
    let revoked = if let Ok(batch) = Uuid::parse_str(target) {
        revoke_batch(state, batch).await?
    } else if let Some(normalized) = normalize_code(target) {
        sqlx::query("UPDATE vouchers SET revoked_at = NOW() WHERE code_hash = $1 AND revoked_at IS NULL")
            .bind(code_hash(&normalized))
            .execute(&state.pg)
            .await?
            .rows_affected()
    } else {
        anyhow::bail!("not a batch id or code: {target}");
    };
    println!("{revoked} code(s) revoked; accounts that already redeemed keep their voucher until it ends");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_round_to_sensible_days() {
        assert_eq!(months_to_days(1), 30);
        assert_eq!(months_to_days(3), 91);
        assert_eq!(months_to_days(6), 183);
        assert_eq!(months_to_days(12), 365);
    }
}
