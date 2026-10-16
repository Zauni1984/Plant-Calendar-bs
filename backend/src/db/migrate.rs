//! Embedded Postgres migrations, applied at startup.
//!
//! Previously nothing applied the schema: a fresh server needed every
//! `migrations/*_postgres_*.sql` run by hand, in order, before the backend
//! would work. The files are now compiled into the binary and applied once
//! each, tracked in `schema_migrations`.
//!
//! Adding a migration: create `migrations/NNN_postgres_<name>.sql` and append
//! it to `MIGRATIONS`. Never edit or reorder an entry that has shipped — a
//! deployed database will not re-run it.

use sqlx::PgPool;

/// Ordered list of (name, sql). The name is the tracking key.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("001_postgres_init", include_str!("../../../migrations/001_postgres_init.sql")),
    ("002_postgres_tombstones", include_str!("../../../migrations/002_postgres_tombstones.sql")),
    ("003_postgres_auth", include_str!("../../../migrations/003_postgres_auth.sql")),
    ("004_postgres_subscription_history", include_str!("../../../migrations/004_postgres_subscription_history.sql")),
    ("006_postgres_plant_status", include_str!("../../../migrations/006_postgres_plant_status.sql")),
    ("007_postgres_plant_status_legacy_backfill", include_str!("../../../migrations/007_postgres_plant_status_legacy_backfill.sql")),
    ("009_postgres_refresh_tokens", include_str!("../../../migrations/009_postgres_refresh_tokens.sql")),
    ("010_postgres_stripe", include_str!("../../../migrations/010_postgres_stripe.sql")),
    ("011_postgres_locations", include_str!("../../../migrations/011_postgres_locations.sql")),
    ("012_postgres_perennials", include_str!("../../../migrations/012_postgres_perennials.sql")),
    ("014_postgres_catalog_id", include_str!("../../../migrations/014_postgres_catalog_id.sql")),
    ("016_postgres_repeat_until", include_str!("../../../migrations/016_postgres_repeat_until.sql")),
    ("021_postgres_consents", include_str!("../../../migrations/021_postgres_consents.sql")),
    ("022_postgres_log_updated_at", include_str!("../../../migrations/022_postgres_log_updated_at.sql")),
    ("022_postgres_tombstone_scope", include_str!("../../../migrations/022_postgres_tombstone_scope.sql")),
    ("023_postgres_location_coords", include_str!("../../../migrations/023_postgres_location_coords.sql")),
    ("024_postgres_weather", include_str!("../../../migrations/024_postgres_weather.sql")),
    ("025_postgres_photos", include_str!("../../../migrations/025_postgres_photos.sql")),
    ("026_postgres_push", include_str!("../../../migrations/026_postgres_push.sql")),
    ("027_postgres_plant_care", include_str!("../../../migrations/027_postgres_plant_care.sql")),
];

/// Arbitrary but fixed key for the advisory lock that serialises migration
/// runs. Two instances starting together (rolling deploy) would otherwise both
/// see a migration as pending and both try to apply it.
const MIGRATION_LOCK_KEY: i64 = 0x504C_414E_5453; // "PLANTS"

/// Apply every migration not yet recorded. Each runs in its own transaction
/// together with its tracking row, so a failure leaves no half-applied step.
pub async fn run(pg: &PgPool) -> anyhow::Result<Vec<&'static str>> {
    let mut conn = pg.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *conn)
        .await?;

    let result = apply_pending(&mut conn).await;

    // Always release, even after a failed migration, so a retry is possible
    // without restarting Postgres.
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *conn)
        .await;
    result
}

async fn apply_pending(conn: &mut sqlx::PgConnection) -> anyhow::Result<Vec<&'static str>> {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(&mut *conn)
    .await?;

    let applied: Vec<String> = sqlx::query_scalar("SELECT name FROM schema_migrations")
        .fetch_all(&mut *conn)
        .await?;

    let mut newly_applied = Vec::new();
    for (name, sql) in MIGRATIONS {
        if applied.iter().any(|a| a == name) {
            continue;
        }
        let mut tx = sqlx::Connection::begin(&mut *conn).await?;
        sqlx::raw_sql(sql)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("migration {name} failed: {e}"))?;
        sqlx::query("INSERT INTO schema_migrations (name) VALUES ($1)")
            .bind(*name)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        newly_applied.push(*name);
    }
    Ok(newly_applied)
}

#[cfg(test)]
mod tests {
    use super::MIGRATIONS;

    #[test]
    fn names_are_unique_and_ordered() {
        for pair in MIGRATIONS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} must sort before {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn every_postgres_file_is_registered() {
        // A migration file that exists but is missing from MIGRATIONS would
        // silently never run on a deployed server.
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../migrations");
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .expect("migrations dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("_postgres_") && n.ends_with(".sql"))
            .map(|n| n.trim_end_matches(".sql").to_string())
            .collect();
        on_disk.sort();
        let registered: Vec<String> = MIGRATIONS.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(on_disk, registered);
    }
}
