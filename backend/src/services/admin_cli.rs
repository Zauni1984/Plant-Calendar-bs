//! Operator tool for admin accounts (admin.sowling.com).
//!
//!   sowling-backend admin grant <email>       make an existing account admin
//!   sowling-backend admin reset-2fa <email>   new authenticator secret
//!   sowling-backend admin revoke <email>
//!   sowling-backend admin list
//!
//! `grant` and `reset-2fa` print a QR code (and the secret) once; scan it with
//! an authenticator app (Google Authenticator, Microsoft Authenticator,
//! 1Password, Aegis, …). The account signs in to the admin area with its
//! normal password plus the 6-digit code from the app.

use qrcode::{render::unicode, QrCode};
use uuid::Uuid;

use crate::{db::{repos, AppState}, services::totp};

const USAGE: &str = "usage:
  admin grant <email>
  admin reset-2fa <email>
  admin revoke <email>
  admin list";

pub async fn run(state: &AppState, args: Vec<String>) -> anyhow::Result<()> {
    match (args.first().map(String::as_str), args.get(1)) {
        (Some("grant"), Some(email)) => set_secret(state, email, false).await,
        (Some("reset-2fa"), Some(email)) => set_secret(state, email, true).await,
        (Some("revoke"), Some(email)) => revoke(state, email).await,
        (Some("list"), _) => list(state).await,
        _ => {
            println!("{USAGE}");
            Ok(())
        }
    }
}

async fn account(state: &AppState, email: &str) -> anyhow::Result<Uuid> {
    repos::get_user_by_email(state, email)
        .await?
        .map(|u| u.user_id)
        .ok_or_else(|| anyhow::anyhow!("no account with the e-mail {email}; register it in the app first"))
}

async fn set_secret(state: &AppState, email: &str, reset: bool) -> anyhow::Result<()> {
    let user_id = account(state, email).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM admins WHERE user_id = $1)").bind(user_id).fetch_one(&state.pg).await?;
    if reset && !exists {
        anyhow::bail!("{email} is not an admin; use `admin grant`");
    }
    let secret = totp::new_secret();
    sqlx::query(
        "INSERT INTO admins (user_id, totp_secret) VALUES ($1, $2) \
         ON CONFLICT (user_id) DO UPDATE SET totp_secret = $2, totp_last_step = 0",
    )
    .bind(user_id)
    .bind(&secret)
    .execute(&state.pg)
    .await?;
    let uri = totp::provisioning_uri(&secret, email);
    let qr = QrCode::new(uri.as_bytes())?
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build();
    println!("{email} is {} admin.\n", if exists { "still" } else { "now" });
    println!("Scan this with your authenticator app (shown only now):\n\n{qr}\n");
    println!("Or enter the key by hand: {secret}");
    println!("(type: time-based, 6 digits, 30 seconds)\n");
    println!("Sign in at the admin area with the account's password and the 6-digit code.");
    if exists && !reset {
        println!("Note: the previous authenticator entry no longer works.");
    }
    Ok(())
}

async fn revoke(state: &AppState, email: &str) -> anyhow::Result<()> {
    let user_id = account(state, email).await?;
    let removed = sqlx::query("DELETE FROM admins WHERE user_id = $1").bind(user_id).execute(&state.pg).await?;
    println!("{}", if removed.rows_affected() > 0 { format!("{email} is no longer an admin") } else { format!("{email} was not an admin") });
    Ok(())
}

async fn list(state: &AppState) -> anyhow::Result<()> {
    let rows: Vec<(Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("SELECT u.email, a.created_at FROM admins a JOIN users u ON u.user_id = a.user_id ORDER BY a.created_at")
            .fetch_all(&state.pg)
            .await?;
    if rows.is_empty() {
        println!("no admins");
    }
    for (email, since) in rows {
        println!("{}  (since {})", email.unwrap_or_default(), since.format("%Y-%m-%d"));
    }
    Ok(())
}
