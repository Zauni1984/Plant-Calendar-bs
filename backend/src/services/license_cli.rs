//! Operator tool for desktop licenses: tests, beta testers, loyal customers.
//!
//!   sowling-backend license create [--email ADDRESS] [--note TEXT] [--devices N] [--no-key]
//!   sowling-backend license list [--all]
//!   sowling-backend license revoke <license-id | key | email>
//!   sowling-backend license reset-devices <license-id>
//!
//! `create` prints the key once; only its hash is stored. With --email the
//! license also unlocks the desktop app for whoever signs in with that
//! address, no key needed (--no-key: e-mail only). Uses DATABASE_URL.

use shared::license;
use uuid::Uuid;

use crate::{api::licenses::{create_license, KeyMode, LicenseRow, DEFAULT_MAX_DEVICES, LICENSE_COLUMNS}, db::AppState};

const USAGE: &str = "usage:
  license create [--email ADDRESS] [--note TEXT] [--devices N] [--no-key]
  license list [--all]
  license revoke <license-id | key | email>
  license reset-devices <license-id>";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

pub async fn run(state: &AppState, args: Vec<String>) -> anyhow::Result<()> {
    match args.first().map(String::as_str) {
        Some("create") => create(state, &args[1..]).await,
        Some("list") => list(state, args.iter().any(|a| a == "--all")).await,
        Some("revoke") => revoke(state, args.get(1).ok_or_else(|| anyhow::anyhow!(USAGE))?).await,
        Some("reset-devices") => reset(state, args.get(1).ok_or_else(|| anyhow::anyhow!(USAGE))?).await,
        _ => {
            println!("{USAGE}");
            Ok(())
        }
    }
}

async fn create(state: &AppState, args: &[String]) -> anyhow::Result<()> {
    let email = flag(args, "--email");
    let note = flag(args, "--note");
    let devices = match flag(args, "--devices") {
        Some(n) => n.parse::<i32>().ok().filter(|n| (1..=100).contains(n)).ok_or_else(|| anyhow::anyhow!("--devices must be 1-100"))?,
        None => DEFAULT_MAX_DEVICES,
    };
    let with_key = !args.iter().any(|a| a == "--no-key");
    if !with_key && email.is_none() {
        anyhow::bail!("--no-key needs --email (otherwise nobody could use the license)");
    }
    if let Some(e) = &email {
        if !e.contains('@') || e.len() > 254 {
            anyhow::bail!("--email is not an e-mail address");
        }
    }
    let (id, key) = create_license(state, "complimentary", email.as_deref(), None, note.as_deref(), devices, if with_key { KeyMode::Random } else { KeyMode::None }, None).await?;
    println!("license {id} created ({devices} devices)");
    if let Some(e) = &email {
        println!("unlocks the desktop app for the account {e} after sign-in");
    }
    if let Some(k) = key {
        println!();
        println!("    {k}");
        println!();
        println!("This key is shown only now; it is not stored in readable form.");
    }
    Ok(())
}

async fn list(state: &AppState, all: bool) -> anyhow::Result<()> {
    let rows: Vec<LicenseRow> = sqlx::query_as(&format!(
        "SELECT {LICENSE_COLUMNS} FROM licenses {} ORDER BY created_at",
        if all { "" } else { "WHERE revoked_at IS NULL" }
    ))
    .fetch_all(&state.pg)
    .await?;
    if rows.is_empty() {
        println!("no licenses");
        return Ok(());
    }
    println!("{:<36}  {:<13}  {:<9}  {:<7}  {:<30}  {:<10}  note", "id", "kind", "key", "devices", "email", "created");
    for r in rows {
        let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM license_activations WHERE license_id = $1")
            .bind(r.license_id)
            .fetch_one(&state.pg)
            .await?;
        let kind = if r.revoked_at.is_some() { format!("{} (revoked)", r.kind) } else { r.kind.clone() };
        println!(
            "{:<36}  {:<13}  {:<9}  {:<7}  {:<30}  {:<10}  {}",
            r.license_id,
            kind,
            r.key_hint.map(|h| format!("…{h}")).unwrap_or_else(|| "-".into()),
            format!("{used}/{}", r.max_devices),
            r.email.unwrap_or_else(|| "-".into()),
            r.created_at.format("%Y-%m-%d"),
            r.note.unwrap_or_default(),
        );
    }
    Ok(())
}

async fn revoke(state: &AppState, target: &str) -> anyhow::Result<()> {
    let result = if let Ok(id) = Uuid::parse_str(target) {
        sqlx::query("UPDATE licenses SET revoked_at = NOW() WHERE license_id = $1 AND revoked_at IS NULL").bind(id).execute(&state.pg).await?
    } else if let Some(normalized) = license::normalize_key(target) {
        sqlx::query("UPDATE licenses SET revoked_at = NOW() WHERE key_hash = $1 AND revoked_at IS NULL")
            .bind(license::key_hash(&normalized))
            .execute(&state.pg)
            .await?
    } else if target.contains('@') {
        sqlx::query("UPDATE licenses SET revoked_at = NOW() WHERE lower(email) = lower($1) AND revoked_at IS NULL")
            .bind(target)
            .execute(&state.pg)
            .await?
    } else {
        anyhow::bail!("not a license id, key or e-mail address: {target}");
    };
    println!("{} license(s) revoked; devices lose access at their next certificate renewal", result.rows_affected());
    Ok(())
}

async fn reset(state: &AppState, target: &str) -> anyhow::Result<()> {
    let id = Uuid::parse_str(target).map_err(|_| anyhow::anyhow!("expected a license id"))?;
    let result = sqlx::query("DELETE FROM license_activations WHERE license_id = $1").bind(id).execute(&state.pg).await?;
    println!("{} device(s) removed from license {id}", result.rows_affected());
    Ok(())
}
