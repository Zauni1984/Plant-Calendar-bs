use chrono::Utc;
use rusqlite::{params, Connection};
use shared::{
    AuthMode, ClimateConfig, Device, DeviceReading, DirtyRecord, LocalProfile, Location, LogEntry, LogType, PhotoRef, Plant,
    PlantLifecycle, PlantPhase, PlantSeason, PlantStatus, SubscriptionPlan, SyncCursor,
    SyncEntityKind, SyncEnvelope, SyncOperation, SyncPushRequest, Task, TaskCategory, TaskStatus,
    UserProfile, WeatherRecord,
};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredSession {
    pub session_key: String,
    pub user_id: Uuid,
    pub auth_mode: AuthMode,
    pub access_token: String,
    pub refresh_token: String,
    pub backend_url: String,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}


#[derive(Debug, Clone, serde::Serialize)]
pub struct ReminderCandidate {
    pub task_id: Uuid,
    pub plant_id: Uuid,
    pub title: String,
    pub category: String,
    pub due_at: chrono::DateTime<Utc>,
    pub overdue: bool,
    pub minutes_until_due: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DirtyBreakdown {
    pub plants: usize,
    pub tasks: usize,
    pub logs: usize,
    pub deletes: usize,
}

pub struct LocalStore {
    conn: Connection,
}

fn parse_plant_status(value: &str, is_active: bool, archived: bool) -> PlantStatus {
    PlantStatus::from_storage_value(value, is_active, archived)
}

fn plant_status_str(value: PlantStatus) -> &'static str {
    value.as_str()
}

fn parse_lifecycle(value: &str) -> PlantLifecycle {
    match value {
        "perennial" => PlantLifecycle::Perennial,
        _ => PlantLifecycle::Annual,
    }
}

fn lifecycle_str(value: PlantLifecycle) -> &'static str {
    match value {
        PlantLifecycle::Annual => "annual",
        PlantLifecycle::Perennial => "perennial",
    }
}

/// Get or generate the SQLCipher master key.
///
/// GDPR Art. 32 — secrets at rest. We store 32 random bytes (hex-encoded) in
/// the OS keychain. Generated once on first launch; reused thereafter.
pub fn get_or_generate_db_key() -> anyhow::Result<String> {
    use rand::RngCore;
    if let Some(existing) = crate::secrets::get("db_master_key")? {
        return Ok(existing);
    }
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let hex_key = hex::encode(bytes);
    crate::secrets::set("db_master_key", &hex_key)?;
    Ok(hex_key)
}

fn apply_key(conn: &Connection, hex_key: &str) -> anyhow::Result<()> {
    // PRAGMA key takes a quoted string. The hex form ('x'…') is not used here
    // because rusqlite's pragma_update needs literal escaping; instead we pass
    // the hex string as a passphrase, which SQLCipher KDFs into a key.
    conn.pragma_update(None, "key", hex_key)?;
    Ok(())
}

fn is_encrypted_open(conn: &Connection) -> bool {
    // After applying the key, this will succeed only if the file is either
    // encrypted with that key or a brand-new (empty) database. It fails for a
    // plaintext SQLite file because SQLCipher then expects an encrypted header.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(())).is_ok()
}

/// True if the file begins with SQLite's plaintext magic header.
/// An empty/short file counts as "not plaintext" so it is never migrated.
fn is_plaintext_sqlite(path: &std::path::Path) -> anyhow::Result<bool> {
    use std::io::Read;
    const MAGIC: &[u8] = b"SQLite format 3\0";
    let mut f = std::fs::File::open(path)?;
    let mut head = [0u8; 16];
    match f.read_exact(&mut head) {
        Ok(()) => Ok(head == MAGIC),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Open the local database with SQLCipher encryption, performing a one-shot
/// migration from plaintext if required.
///
/// GDPR Art. 32 — encryption of personal data at rest.
pub fn open_or_migrate(path: &std::path::Path) -> anyhow::Result<Connection> {
    let hex_key = get_or_generate_db_key()?;

    // Fresh install: just open + key + done.
    if !path.exists() {
        let conn = Connection::open(path)?;
        apply_key(&conn, &hex_key)?;
        return Ok(conn);
    }

    // Try opening as encrypted.
    {
        let conn = Connection::open(path)?;
        apply_key(&conn, &hex_key)?;
        if is_encrypted_open(&conn) {
            return Ok(conn);
        }
        // Either genuinely plaintext, or encrypted with a key we no longer have.
        drop(conn);
    }

    // Distinguish those two cases before doing anything destructive. A real
    // SQLite file starts with the literal header "SQLite format 3\0"; an
    // encrypted one does not. Running the migration on an encrypted file used
    // to fail deep inside sqlcipher_export and abort startup with an opaque
    // "file is not a database".
    if !is_plaintext_sqlite(path)? {
        anyhow::bail!(
            "the local database at {} is encrypted, but its key is missing from the OS keychain. \
             Refusing to touch the file. Restore the keychain entry, or move the file aside to \
             start with an empty database (the data in it cannot be recovered without the key).",
            path.display()
        );
    }

    eprintln!("[local_db] migrating plaintext DB to SQLCipher: {}", path.display());

    let enc_path = path.with_extension("sqlite.enc");
    if enc_path.exists() {
        // Stale partial migration — remove it and retry.
        std::fs::remove_file(&enc_path).ok();
    }

    {
        let plain = Connection::open(path)?;
        // Attach an encrypted DB and copy everything over.
        let attach_sql = format!(
            "ATTACH DATABASE '{}' AS encrypted KEY '{}'",
            enc_path.to_string_lossy().replace('\'', "''"),
            hex_key.replace('\'', "''"),
        );
        plain.execute_batch(&attach_sql)?;
        plain.query_row("SELECT sqlcipher_export('encrypted')", [], |_| Ok(()))?;
        plain.execute_batch("DETACH DATABASE encrypted")?;
    }

    // Swap files, keeping the plaintext copy only until the encrypted DB is
    // proven to open. Leaving it behind permanently would mean an unencrypted
    // copy of all personal data stays on disk, defeating the at-rest encryption.
    let bak_path = path.with_extension("sqlite.bak");
    std::fs::rename(path, &bak_path)?;
    std::fs::rename(&enc_path, path)?;

    let conn = Connection::open(path)?;
    apply_key(&conn, &hex_key)?;
    if !is_encrypted_open(&conn) {
        // Migration failed — restore the plaintext DB so the user keeps their data.
        drop(conn);
        std::fs::remove_file(path).ok();
        std::fs::rename(&bak_path, path).ok();
        anyhow::bail!("post-migration: encrypted DB failed to open with key; restored plaintext DB");
    }

    // Verified: shred the plaintext leftover.
    match std::fs::remove_file(&bak_path) {
        Ok(()) => eprintln!("[local_db] migration complete; plaintext copy removed"),
        Err(e) => eprintln!(
            "[local_db] WARNING: could not remove plaintext backup {}: {e}",
            bak_path.display()
        ),
    }
    Ok(conn)
}

/// Apply a migration statement-by-statement, tolerating columns that already
/// exist.
///
/// `execute_batch` aborts the WHOLE batch on the first error, so a file with
/// several `ALTER TABLE ... ADD COLUMN` guarded only by an outer
/// `contains("duplicate column")` swallow would silently skip every statement
/// after the first already-applied one. Splitting on `;` is safe here because
/// none of these migration files contain a semicolon inside a string literal.
fn apply_migration_tolerant(conn: &Connection, sql: &str) -> anyhow::Result<()> {
    for statement in sql.split(';') {
        let trimmed = statement.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Err(e) = conn.execute_batch(trimmed) {
            let msg = e.to_string();
            // "duplicate column name" — the column is already there, which is
            // exactly the state we want. Anything else is a real failure.
            if !msg.contains("duplicate column") {
                return Err(e.into());
            }
        }
    }
    Ok(())
}

impl LocalStore {
    pub fn open(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let conn = open_or_migrate(path.as_ref())?;
        conn.execute_batch(include_str!("../../../migrations/001_sqlite_init.sql"))?;
        conn.execute_batch(include_str!("../../../migrations/002_sqlite_tombstones.sql"))?;
        // 003: email/password_hash on users. Unused locally (auth lives in
        // Postgres) but applied so the local schema stays a complete 001..021
        // chain — a gap here would break any future migration that assumes it.
        apply_migration_tolerant(&conn, include_str!("../../../migrations/003_sqlite_auth.sql"))?;
        conn.execute_batch(include_str!("../../../migrations/004_sqlite_session.sql"))?;
        conn.execute_batch(include_str!("../../../migrations/005_sqlite_sync_cursor.sql"))?;
        // 006 uses ALTER TABLE ADD COLUMN which fails if column already exists;
        // ignore "duplicate column" errors to keep init idempotent.
        apply_migration_tolerant(&conn, include_str!("../../../migrations/006_sqlite_plant_status.sql"))?;
        conn.execute_batch(include_str!("../../../migrations/007_sqlite_plant_status_legacy_backfill.sql"))?;
        conn.execute_batch(include_str!("../../../migrations/008_sqlite_sync_state.sql"))?;
        // 009: add refresh_token column to app_session (ignore if already exists)
        apply_migration_tolerant(&conn, include_str!("../../../migrations/009_sqlite_refresh_token.sql"))?;
        // 010: devices and device_readings tables for BLE integration
        conn.execute_batch(include_str!("../../../migrations/010_sqlite_devices.sql"))?;
        // 011: locations table and plants.location_id column
        apply_migration_tolerant(&conn, include_str!("../../../migrations/011_sqlite_locations.sql"))?;
        // 012: perennials – plant_seasons table and lifecycle/plant_year columns
        apply_migration_tolerant(&conn, include_str!("../../../migrations/012_sqlite_perennials.sql"))?;
        // 013: photos table (local-only, never synced)
        conn.execute_batch(include_str!("../../../migrations/013_sqlite_photos.sql"))?;
        // 014: catalog_id column on plants
        apply_migration_tolerant(&conn, include_str!("../../../migrations/014_sqlite_catalog_id.sql"))?;
        // 015: climate config table
        conn.execute_batch(include_str!("../../../migrations/015_sqlite_climate.sql"))?;
        // 016: repeat_until column on tasks
        apply_migration_tolerant(&conn, include_str!("../../../migrations/016_sqlite_repeat_until.sql"))?;
        // 017: weather_history table and lat/lon columns on locations
        // Split into table create + per-ALTER to be robust against partial prior runs.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS weather_history (\n  id TEXT PRIMARY KEY,\n  location_id TEXT,\n  latitude REAL NOT NULL,\n  longitude REAL NOT NULL,\n  temp_c REAL,\n  humidity REAL,\n  wind_kmh REAL,\n  precipitation_mm REAL,\n  weather_code INTEGER,\n  recorded_at TEXT NOT NULL\n);\nCREATE INDEX IF NOT EXISTS idx_weather_history_time ON weather_history(recorded_at DESC);\nCREATE INDEX IF NOT EXISTS idx_weather_history_location ON weather_history(location_id, recorded_at DESC);"
        )?;
        for sql in [
            "ALTER TABLE locations ADD COLUMN latitude REAL",
            "ALTER TABLE locations ADD COLUMN longitude REAL",
        ] {
            if let Err(e) = conn.execute(sql, []) {
                let msg = e.to_string();
                if !msg.contains("duplicate column") { return Err(e.into()); }
            }
        }
        // 018: pot_size_liters and soil_type columns on plants
        apply_migration_tolerant(&conn, include_str!("../../../migrations/018_sqlite_plant_care.sql"))?;
        // 019: local user profiles for multi-user support
        apply_migration_tolerant(&conn, include_str!("../../../migrations/019_sqlite_profiles.sql"))?;
        // 020: extended weather metrics — each ALTER applied independently so a
        // partially-applied migration can be resumed without aborting later columns.
        for sql in [
            "ALTER TABLE weather_history ADD COLUMN apparent_temp_c REAL",
            "ALTER TABLE weather_history ADD COLUMN uv_index REAL",
            "ALTER TABLE weather_history ADD COLUMN cloud_cover REAL",
            "ALTER TABLE weather_history ADD COLUMN pressure_hpa REAL",
            "ALTER TABLE weather_history ADD COLUMN dew_point_c REAL",
            "ALTER TABLE weather_history ADD COLUMN visibility_km REAL",
            "ALTER TABLE weather_history ADD COLUMN wind_gusts_kmh REAL",
            "ALTER TABLE weather_history ADD COLUMN wind_direction_deg REAL",
            "ALTER TABLE weather_history ADD COLUMN soil_temp_c REAL",
            "ALTER TABLE weather_history ADD COLUMN soil_moisture REAL",
            "ALTER TABLE weather_history ADD COLUMN is_day INTEGER",
        ] {
            if let Err(e) = conn.execute(sql, []) {
                let msg = e.to_string();
                if !msg.contains("duplicate column") { return Err(e.into()); }
            }
        }
        // 021: GDPR consent persistence. Idempotent (CREATE TABLE/INDEX IF NOT EXISTS).
        conn.execute_batch(include_str!("../../../migrations/021_sqlite_consents.sql"))?;
        // 022: logs.updated_at — the last-write-wins key for log sync.
        apply_migration_tolerant(&conn, include_str!("../../../migrations/022_sqlite_log_updated_at.sql"))?;
        Ok(Self { conn })
    }

    pub fn get_or_create_guest_user(&self) -> anyhow::Result<UserProfile> {
        let guest_id = Uuid::nil();
        let mut stmt = self.conn.prepare(
            "SELECT user_id, auth_mode, adult_confirmed, locale, timezone, plan, created_at, updated_at FROM users WHERE user_id = ?1",
        )?;
        let mut rows = stmt.query(params![guest_id.to_string()])?;
        if let Some(row) = rows.next()? {
            return Ok(UserProfile {
                user_id: Uuid::parse_str(&row.get::<_, String>(0)?)?,
                auth_mode: match row.get::<_, String>(1)?.as_str() {
                    "email" => AuthMode::Email,
                    "google" => AuthMode::Google,
                    _ => AuthMode::Anonymous,
                },
                adult_confirmed: row.get::<_, i64>(2)? == 1,
                locale: row.get(3)?,
                timezone: row.get(4)?,
                plan: match row.get::<_, String>(5)?.as_str() {
                    "basic" => SubscriptionPlan::Basic,
                    "pro" => SubscriptionPlan::Pro,
                    "csc" => SubscriptionPlan::Csc,
                    _ => SubscriptionPlan::Free,
                },
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            });
        }

        let now = Utc::now();
        self.conn.execute(
            "INSERT INTO users (user_id, auth_mode, adult_confirmed, locale, timezone, plan, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![guest_id.to_string(), "anonymous", 1, "de-DE", "Europe/Berlin", "free", now, now],
        )?;

        Ok(UserProfile {
            user_id: guest_id,
            auth_mode: AuthMode::Anonymous,
            adult_confirmed: true,
            locale: "de-DE".into(),
            timezone: "Europe/Berlin".into(),
            plan: SubscriptionPlan::Free,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn link_authenticated_user(&self, user_id: Uuid, auth_mode: AuthMode) -> anyhow::Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT OR REPLACE INTO users (user_id, auth_mode, adult_confirmed, locale, timezone, plan, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,COALESCE((SELECT created_at FROM users WHERE user_id = ?1), ?7),?8)",
            params![
                user_id.to_string(),
                match auth_mode { AuthMode::Anonymous => "anonymous", AuthMode::Email => "email", AuthMode::Google => "google" },
                1,
                "de-DE",
                "Europe/Berlin",
                "free",
                now,
                now,
            ],
        )?;
        self.conn.execute(
            "UPDATE plants SET user_id = ?2, updated_at = ?3 WHERE user_id = ?1",
            params![Uuid::nil().to_string(), user_id.to_string(), now],
        )?;
        self.mark_all_guest_records_dirty(user_id)?;
        Ok(())
    }

    pub fn mark_all_guest_records_dirty(&self, _new_user_id: Uuid) -> anyhow::Result<()> {
        let now = Utc::now();
        // Plants *and* every task/log. Marking only plants left all
        // offline-created tasks and logs permanently unsynced after login.
        for plant in self.list_plants()? {
            self.mark_dirty(DirtyRecord {
                record_id: plant.plant_id,
                entity_kind: SyncEntityKind::Plant,
                operation: SyncOperation::Upsert,
                changed_at: now,
            })?;
        }
        for task in self.list_tasks()? {
            self.mark_dirty(DirtyRecord {
                record_id: task.task_id,
                entity_kind: SyncEntityKind::Task,
                operation: SyncOperation::Upsert,
                changed_at: now,
            })?;
        }
        for log in self.list_logs()? {
            self.mark_dirty(DirtyRecord {
                record_id: log.log_id,
                entity_kind: SyncEntityKind::Log,
                operation: SyncOperation::Upsert,
                changed_at: now,
            })?;
        }
        Ok(())
    }




    pub fn load_last_sync_cursor(&self) -> anyhow::Result<Option<SyncCursor>> {
        let mut stmt = self.conn.prepare("SELECT meta_value FROM app_meta WHERE meta_key = 'last_sync_at' LIMIT 1")?;
        let mut rows = stmt.query([])?;
        let last_synced_at = if let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            Some(chrono::DateTime::parse_from_rfc3339(&raw)?.with_timezone(&Utc))
        } else {
            None
        };

        let session = self.load_session()?;
        Ok(Some(SyncCursor {
            user_id: session.as_ref().map(|s| s.user_id),
            device_id: Some("desktop-dev".into()),
            last_synced_at,
        }))
    }

    pub fn save_last_sync_cursor(&self, cursor: &SyncCursor) -> anyhow::Result<()> {
        if let Some(last_synced_at) = cursor.last_synced_at {
            let now = Utc::now();
            self.conn.execute(
                "INSERT OR REPLACE INTO app_meta (meta_key, meta_value, updated_at) VALUES (?1,?2,?3)",
                params!["last_sync_at", last_synced_at.to_rfc3339(), now.to_rfc3339()],
            )?;
        }
        Ok(())
    }

    pub fn save_session(&self, user_id: Uuid, auth_mode: AuthMode, access_token: String, refresh_token: String, backend_url: String) -> anyhow::Result<StoredSession> {
        let now = Utc::now();
        let session = StoredSession {
            session_key: "active".into(),
            user_id,
            auth_mode,
            access_token,
            refresh_token,
            backend_url,
            created_at: now,
            updated_at: now,
        };
        let auth_mode_value = match session.auth_mode {
            AuthMode::Anonymous => "anonymous",
            AuthMode::Email => "email",
            AuthMode::Google => "google",
        };
        self.conn.execute(
            "INSERT OR REPLACE INTO app_session (session_key, user_id, auth_mode, access_token, refresh_token, backend_url, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,COALESCE((SELECT created_at FROM app_session WHERE session_key = ?1), ?7),?8)",
            params![
                &session.session_key,
                session.user_id.to_string(),
                auth_mode_value,
                &session.access_token,
                &session.refresh_token,
                &session.backend_url,
                now,
                now,
            ],
        )?;
        Ok(session)
    }

    pub fn load_session(&self) -> anyhow::Result<Option<StoredSession>> {
        let mut stmt = self.conn.prepare("SELECT session_key, user_id, auth_mode, access_token, backend_url, created_at, updated_at, refresh_token FROM app_session WHERE session_key = 'active' LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let auth_mode = match row.get::<_, String>(2)?.as_str() {
                "email" => AuthMode::Email,
                "google" => AuthMode::Google,
                _ => AuthMode::Anonymous,
            };
            return Ok(Some(StoredSession {
                session_key: row.get(0)?,
                user_id: Uuid::parse_str(&row.get::<_, String>(1)?)?,
                auth_mode,
                access_token: row.get(3)?,
                refresh_token: row.get::<_, String>(7).unwrap_or_default(),
                backend_url: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            }));
        }
        Ok(None)
    }

    pub fn clear_session(&self) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM app_session WHERE session_key = 'active'", [])?;
        Ok(())
    }

    /// Wipe all user-data tables from the local database. Used when the
    /// server-side account deletion has succeeded so that the local copy
    /// is erased as well (Art. 17 GDPR).
    ///
    /// Each `DELETE` is issued individually and tolerated if the table is
    /// absent, so this stays forward-compatible across schema versions.
    pub fn wipe_all(&self) -> anyhow::Result<()> {
        let tables = [
            // Domain data
            "logs",
            "tasks",
            "plant_seasons",
            "plants",
            "locations",
            // Devices and readings
            "device_readings",
            "devices",
            // Photos (local-only index)
            "photos",
            // Weather history (local cache)
            "weather_history",
            // Climate config
            "climate_config",
            // Sync bookkeeping
            "deleted_records",
            "sync_state",
            "app_meta",
            // Local profiles
            "user_profiles",
            // GDPR consent records
            "user_consents",
            // Users (including guest row)
            "users",
            // Session at the very end
            "app_session",
        ];
        for t in tables {
            let sql = format!("DELETE FROM {t}");
            if let Err(e) = self.conn.execute(&sql, []) {
                let msg = e.to_string();
                // Ignore missing tables so older / partial DBs do not trip us up.
                if !msg.contains("no such table") {
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }

    pub fn list_plants(&self) -> anyhow::Result<Vec<Plant>> {
        let mut stmt = self.conn.prepare(
            "SELECT plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, created_at, updated_at, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type FROM plants WHERE plant_status != 'deleted' ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Plant {
                plant_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                user_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                name: row.get(2)?,
                color_tag: row.get(3)?,
                start_date: row.get(4)?,
                phase: match row.get::<_, String>(5)?.as_str() {
                    "seed" => PlantPhase::Seed,
                    "flower" => PlantPhase::Flower,
                    "harvest" => PlantPhase::Harvest,
                    "dry" => PlantPhase::Dry,
                    "cure" => PlantPhase::Cure,
                    "custom" => PlantPhase::Custom,
                    "dormant" => PlantPhase::Dormant,
                    _ => PlantPhase::Veg,
                },
                phase_week: row.get(6)?,
                status: parse_plant_status(&row.get::<_, String>(7)?, row.get::<_, i64>(8)? == 1, row.get::<_, i64>(9)? == 1),
                is_active: row.get::<_, i64>(8)? == 1,
                archived: row.get::<_, i64>(9)? == 1,
                location_id: row.get::<_, Option<String>>(12)?.and_then(|s| Uuid::parse_str(&s).ok()),
                lifecycle: parse_lifecycle(&row.get::<_, Option<String>>(13)?.unwrap_or_else(|| "annual".into())),
                plant_year: row.get(14)?,
                catalog_id: row.get(15)?,
                pot_size_liters: row.get(16)?,
                soil_type: row.get(17)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            }.normalized())
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn upsert_plant(&self, plant: &Plant) -> anyhow::Result<()> {
        let plant = plant.normalized_ref();
        self.conn.execute(
            "INSERT OR REPLACE INTO plants (plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, created_at, updated_at, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
            params![
                plant.plant_id.to_string(),
                plant.user_id.to_string(),
                &plant.name,
                &plant.color_tag,
                plant.start_date,
                format!("{:?}", plant.phase).to_lowercase(),
                plant.phase_week,
                plant_status_str(plant.status),
                i64::from(plant.is_active_state()),
                i64::from(plant.is_archived_state()),
                plant.created_at,
                plant.updated_at,
                plant.location_id.map(|id| id.to_string()),
                lifecycle_str(plant.lifecycle),
                plant.plant_year,
                &plant.catalog_id,
                plant.pot_size_liters,
                &plant.soil_type,
            ],
        )?;
        self.mark_dirty(DirtyRecord {
            record_id: plant.plant_id,
            entity_kind: SyncEntityKind::Plant,
            operation: SyncOperation::Upsert,
            changed_at: Utc::now(),
        })?;
        Ok(())
    }




    pub fn cache_upsert_plant(&self, plant: &Plant) -> anyhow::Result<()> {
        let plant = plant.normalized_ref();
        self.conn.execute(
            "INSERT OR REPLACE INTO plants (plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, created_at, updated_at, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
            params![
                plant.plant_id.to_string(),
                plant.user_id.to_string(),
                &plant.name,
                &plant.color_tag,
                plant.start_date,
                format!("{:?}", plant.phase).to_lowercase(),
                plant.phase_week,
                plant_status_str(plant.status),
                i64::from(plant.is_active_state()),
                i64::from(plant.is_archived_state()),
                plant.created_at,
                plant.updated_at,
                plant.location_id.map(|id| id.to_string()),
                lifecycle_str(plant.lifecycle),
                plant.plant_year,
                &plant.catalog_id,
                plant.pot_size_liters,
                &plant.soil_type,
            ],
        )?;
        Ok(())
    }

    pub fn replace_cached_plants(&self, plants: &[Plant]) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        // Keep rows that still have unpushed local changes. A plain
        // `DELETE FROM plants` destroyed everything a guest created offline as
        // soon as the first pull after login returned an empty server list.
        tx.execute(
            "DELETE FROM plants WHERE plant_id NOT IN \
             (SELECT record_id FROM sync_state WHERE entity_kind = 'plant')",
            [],
        )?;
        for plant in plants {
            tx.execute(
                "INSERT OR REPLACE INTO plants (plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, created_at, updated_at, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
                params![
                    plant.plant_id.to_string(),
                    plant.user_id.to_string(),
                    &plant.name,
                    &plant.color_tag,
                    plant.start_date,
                    format!("{:?}", plant.phase).to_lowercase(),
                    plant.phase_week,
                    plant_status_str(plant.status),
                    i64::from(plant.is_active_state()),
                    i64::from(plant.is_archived_state()),
                    plant.created_at,
                    plant.updated_at,
                    plant.location_id.map(|id| id.to_string()),
                    lifecycle_str(plant.lifecycle),
                    plant.plant_year,
                    &plant.catalog_id,
                    plant.pot_size_liters,
                    &plant.soil_type,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn cache_delete_plant(&self, plant_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("UPDATE plants SET plant_status = ?2, is_active = 0, archived = 0, updated_at = ?3 WHERE plant_id = ?1", params![plant_id.to_string(), "deleted", Utc::now()])?;
        self.clear_dirty(plant_id)?;
        Ok(())
    }

    pub fn delete_plant(&self, plant_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("UPDATE plants SET plant_status = ?2, is_active = 0, archived = 0, updated_at = ?3 WHERE plant_id = ?1", params![plant_id.to_string(), "deleted", Utc::now()])?;
        self.record_tombstone(Uuid::nil(), plant_id, SyncEntityKind::Plant)?;
        Ok(())
    }

    // ── Location CRUD ──

    pub fn list_locations(&self) -> anyhow::Result<Vec<Location>> {
        let mut stmt = self.conn.prepare(
            "SELECT location_id, user_id, name, location_type, description, created_at, updated_at, latitude, longitude FROM locations ORDER BY name ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Location {
                location_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                user_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                name: row.get(2)?,
                location_type: row.get(3)?,
                description: row.get(4)?,
                latitude: row.get(7)?,
                longitude: row.get(8)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn upsert_location(&self, loc: &Location) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO locations (location_id, user_id, name, location_type, description, created_at, updated_at, latitude, longitude) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                loc.location_id.to_string(),
                loc.user_id.to_string(),
                &loc.name,
                &loc.location_type,
                &loc.description,
                loc.created_at,
                loc.updated_at,
                loc.latitude,
                loc.longitude,
            ],
        )?;
        Ok(())
    }

    pub fn delete_location(&self, location_id: Uuid) -> anyhow::Result<()> {
        // Unlink plants first
        self.conn.execute("UPDATE plants SET location_id = NULL WHERE location_id = ?1", params![location_id.to_string()])?;
        self.conn.execute("DELETE FROM locations WHERE location_id = ?1", params![location_id.to_string()])?;
        Ok(())
    }

    // ── LocalProfile CRUD ──

    pub fn list_profiles(&self) -> anyhow::Result<Vec<LocalProfile>> {
        let mut stmt = self.conn.prepare(
            "SELECT profile_id, name, role, color, avatar_emoji, is_owner, created_at, updated_at FROM user_profiles ORDER BY is_owner DESC, name ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LocalProfile {
                profile_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                name: row.get(1)?,
                role: row.get(2)?,
                color: row.get(3)?,
                avatar_emoji: row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "\u{1F9D1}\u{200D}\u{1F33E}".into()),
                is_owner: row.get::<_, i64>(5)? == 1,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn upsert_profile(&self, profile: &LocalProfile) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO user_profiles (profile_id, name, role, color, avatar_emoji, is_owner, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                profile.profile_id.to_string(),
                &profile.name,
                &profile.role,
                &profile.color,
                &profile.avatar_emoji,
                i64::from(profile.is_owner),
                profile.created_at,
                profile.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_profile(&self, profile_id: Uuid) -> anyhow::Result<()> {
        // Unlink plants and tasks assigned to this profile
        self.conn.execute("UPDATE plants SET assigned_profile_id = NULL WHERE assigned_profile_id = ?1", params![profile_id.to_string()])?;
        self.conn.execute("UPDATE tasks SET assigned_profile_id = NULL WHERE assigned_profile_id = ?1", params![profile_id.to_string()])?;
        self.conn.execute("DELETE FROM user_profiles WHERE profile_id = ?1", params![profile_id.to_string()])?;
        // If the deleted profile was active, clear it
        let active = self.get_active_profile_id()?;
        if active.as_ref() == Some(&profile_id) {
            self.clear_active_profile()?;
        }
        Ok(())
    }

    pub fn get_active_profile_id(&self) -> anyhow::Result<Option<Uuid>> {
        let mut stmt = self.conn.prepare("SELECT meta_value FROM app_meta WHERE meta_key = 'active_profile' LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            if raw.is_empty() {
                return Ok(None);
            }
            return Ok(Uuid::parse_str(&raw).ok());
        }
        Ok(None)
    }

    pub fn get_active_profile(&self) -> anyhow::Result<Option<LocalProfile>> {
        let profile_id = match self.get_active_profile_id()? {
            Some(id) => id,
            None => return Ok(None),
        };
        let profiles = self.list_profiles()?;
        Ok(profiles.into_iter().find(|p| p.profile_id == profile_id))
    }

    pub fn set_active_profile(&self, profile_id: Uuid) -> anyhow::Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT OR REPLACE INTO app_meta (meta_key, meta_value, updated_at) VALUES ('active_profile', ?1, ?2)",
            params![profile_id.to_string(), now.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn clear_active_profile(&self) -> anyhow::Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT OR REPLACE INTO app_meta (meta_key, meta_value, updated_at) VALUES ('active_profile', '', ?1)",
            params![now.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn assign_plant_to_profile(&self, plant_id: Uuid, profile_id: Option<Uuid>) -> anyhow::Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE plants SET assigned_profile_id = ?2, updated_at = ?3 WHERE plant_id = ?1",
            params![plant_id.to_string(), profile_id.map(|id| id.to_string()), now],
        )?;
        Ok(())
    }

    // ── PlantSeason CRUD ──

    pub fn list_seasons_for_plant(&self, plant_id: Uuid) -> anyhow::Result<Vec<PlantSeason>> {
        let mut stmt = self.conn.prepare(
            "SELECT season_id, plant_id, year, started_at, ended_at, notes, harvest_kg, harvest_count, pruning_done, created_at, updated_at FROM plant_seasons WHERE plant_id = ?1 ORDER BY year DESC"
        )?;
        let rows = stmt.query_map(params![plant_id.to_string()], |row| {
            Ok(PlantSeason {
                season_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                plant_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                year: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                notes: row.get(5)?,
                harvest_kg: row.get(6)?,
                harvest_count: row.get(7)?,
                pruning_done: row.get::<_, i64>(8)? == 1,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn upsert_season(&self, season: &PlantSeason) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO plant_seasons (season_id, plant_id, year, started_at, ended_at, notes, harvest_kg, harvest_count, pruning_done, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                season.season_id.to_string(),
                season.plant_id.to_string(),
                season.year,
                season.started_at,
                season.ended_at,
                &season.notes,
                season.harvest_kg,
                season.harvest_count,
                i64::from(season.pruning_done),
                season.created_at,
                season.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_season(&self, season_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM plant_seasons WHERE season_id = ?1", params![season_id.to_string()])?;
        Ok(())
    }

    pub fn cache_upsert_task(&self, task: &Task) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO tasks (task_id, plant_id, title, category, due_at, repeat_interval_hours, notification_enabled, status, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                task.task_id.to_string(),
                task.plant_id.to_string(),
                &task.title,
                format!("{:?}", task.category).to_lowercase(),
                task.due_at,
                task.repeat_interval_hours,
                i64::from(task.notification_enabled),
                format!("{:?}", task.status).to_lowercase(),
                task.created_at,
                task.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn replace_cached_tasks_for_plant(&self, plant_id: Uuid, tasks: &[Task]) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        // Preserve tasks that have not been pushed yet (see replace_cached_plants).
        tx.execute(
            "DELETE FROM tasks WHERE plant_id = ?1 AND task_id NOT IN \
             (SELECT record_id FROM sync_state WHERE entity_kind = 'task')",
            params![plant_id.to_string()],
        )?;
        for task in tasks {
            tx.execute(
                // INSERT OR REPLACE rewrites the whole row, so repeat_until must
                // be listed or every cache refresh would clear the series end.
                "INSERT OR REPLACE INTO tasks (task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    task.task_id.to_string(),
                    task.plant_id.to_string(),
                    &task.title,
                    format!("{:?}", task.category).to_lowercase(),
                    task.due_at,
                    task.repeat_interval_hours,
                    task.repeat_until,
                    i64::from(task.notification_enabled),
                    format!("{:?}", task.status).to_lowercase(),
                    task.created_at,
                    task.updated_at,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn cache_delete_task(&self, task_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM tasks WHERE task_id = ?1", params![task_id.to_string()])?;
        self.clear_dirty(task_id)?;
        Ok(())
    }

    pub fn delete_task(&self, task_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM tasks WHERE task_id = ?1", params![task_id.to_string()])?;
        self.record_tombstone(Uuid::nil(), task_id, SyncEntityKind::Task)?;
        Ok(())
    }


    pub fn cache_upsert_log(&self, log: &LogEntry) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO logs (log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                log.log_id.to_string(),
                log.plant_id.to_string(),
                format!("{:?}", log.log_type).to_lowercase(),
                log.text.as_deref(),
                log.metrics.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                log.action.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                log.photo_refs.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                log.created_at,
                log.last_modified(),
            ],
        )?;
        Ok(())
    }

    pub fn replace_cached_logs_for_plant(&self, plant_id: Uuid, logs: &[LogEntry]) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        // Preserve logs that have not been pushed yet (see replace_cached_plants).
        tx.execute(
            "DELETE FROM logs WHERE plant_id = ?1 AND log_id NOT IN \
             (SELECT record_id FROM sync_state WHERE entity_kind = 'log')",
            params![plant_id.to_string()],
        )?;
        for log in logs {
            tx.execute(
                "INSERT OR REPLACE INTO logs (log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    log.log_id.to_string(),
                    log.plant_id.to_string(),
                    format!("{:?}", log.log_type).to_lowercase(),
                    log.text.as_deref(),
                    log.metrics.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    log.action.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    log.photo_refs.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    log.created_at,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn cache_delete_log(&self, log_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM logs WHERE log_id = ?1", params![log_id.to_string()])?;
        self.clear_dirty(log_id)?;
        Ok(())
    }

    pub fn delete_log(&self, log_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM logs WHERE log_id = ?1", params![log_id.to_string()])?;
        self.record_tombstone(Uuid::nil(), log_id, SyncEntityKind::Log)?;
        Ok(())
    }

    pub fn record_tombstone(&self, user_id: Uuid, record_id: Uuid, entity_kind: SyncEntityKind) -> anyhow::Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT OR REPLACE INTO deleted_records (record_id, user_id, entity_kind, operation, changed_at) VALUES (?1,?2,?3,?4,?5)",
            params![record_id.to_string(), user_id.to_string(), format!("{:?}", entity_kind).to_lowercase(), "delete", now],
        )?;
        self.mark_dirty(DirtyRecord {
            record_id,
            entity_kind,
            operation: SyncOperation::Delete,
            changed_at: now,
        })?;
        Ok(())
    }

    pub fn apply_deleted_records(&self, deleted: &[DirtyRecord]) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for item in deleted {
            match item.entity_kind {
                SyncEntityKind::Plant => { tx.execute("DELETE FROM plants WHERE plant_id = ?1", params![item.record_id.to_string()])?; }
                SyncEntityKind::Task => { tx.execute("DELETE FROM tasks WHERE task_id = ?1", params![item.record_id.to_string()])?; }
                SyncEntityKind::Log => { tx.execute("DELETE FROM logs WHERE log_id = ?1", params![item.record_id.to_string()])?; }
            }
            tx.execute("DELETE FROM deleted_records WHERE record_id = ?1 AND entity_kind = ?2", params![item.record_id.to_string(), format!("{:?}", item.entity_kind).to_lowercase()])?;
            tx.execute("DELETE FROM sync_state WHERE record_id = ?1", params![item.record_id.to_string()])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_tasks(&self) -> anyhow::Result<Vec<Task>> {
        let mut stmt = self.conn.prepare("SELECT task_id, plant_id, title, category, due_at, repeat_interval_hours, notification_enabled, status, created_at, updated_at, repeat_until FROM tasks ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Task {
                task_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                plant_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                title: row.get(2)?,
                category: match row.get::<_, String>(3)?.as_str() {
                    "feed" => TaskCategory::Feed,
                    "check" => TaskCategory::Check,
                    "train" => TaskCategory::Train,
                    "note" => TaskCategory::Note,
                    _ => TaskCategory::Water,
                },
                due_at: row.get(4)?,
                repeat_interval_hours: row.get(5)?,
                repeat_until: row.get(10)?,
                notification_enabled: row.get::<_, i64>(6)? == 1,
                status: match row.get::<_, String>(7)?.as_str() {
                    "done" => TaskStatus::Done,
                    "skipped" => TaskStatus::Skipped,
                    _ => TaskStatus::Open,
                },
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn upsert_task(&self, task: &Task) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO tasks (task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                task.task_id.to_string(),
                task.plant_id.to_string(),
                &task.title,
                format!("{:?}", task.category).to_lowercase(),
                task.due_at,
                task.repeat_interval_hours,
                task.repeat_until,
                i64::from(task.notification_enabled),
                format!("{:?}", task.status).to_lowercase(),
                task.created_at,
                task.updated_at,
            ],
        )?;
        self.mark_dirty(DirtyRecord {
            record_id: task.task_id,
            entity_kind: SyncEntityKind::Task,
            operation: SyncOperation::Upsert,
            changed_at: Utc::now(),
        })?;
        Ok(())
    }

    pub fn list_logs(&self) -> anyhow::Result<Vec<LogEntry>> {
        let mut stmt = self.conn.prepare("SELECT log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at FROM logs ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(LogEntry {
                log_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                plant_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                log_type: match row.get::<_, String>(2)?.as_str() {
                    "measurement" => LogType::Measurement,
                    "harvest" => LogType::Harvest,
                    "watering" => LogType::Watering,
                    "photo" => LogType::Photo,
                    "action" => LogType::Action,
                    _ => LogType::Note,
                },
                text: row.get(3)?,
                metrics: row.get::<_, Option<String>>(4)?.and_then(|v| serde_json::from_str(&v).ok()),
                action: row.get::<_, Option<String>>(5)?.and_then(|v| serde_json::from_str(&v).ok()),
                photo_refs: row.get::<_, Option<String>>(6)?.and_then(|v| serde_json::from_str(&v).ok()),
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn upsert_log(&self, log: &LogEntry) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO logs (log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                log.log_id.to_string(),
                log.plant_id.to_string(),
                format!("{:?}", log.log_type).to_lowercase(),
                log.text.as_deref(),
                log.metrics.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                log.action.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                log.photo_refs.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                log.created_at,
                log.last_modified(),
            ],
        )?;
        self.mark_dirty(DirtyRecord {
            record_id: log.log_id,
            entity_kind: SyncEntityKind::Log,
            operation: SyncOperation::Upsert,
            changed_at: Utc::now(),
        })?;
        Ok(())
    }

    pub fn mark_dirty(&self, item: DirtyRecord) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_state (record_id, entity_kind, operation, changed_at) VALUES (?1,?2,?3,?4)",
            params![
                item.record_id.to_string(),
                format!("{:?}", item.entity_kind).to_lowercase(),
                format!("{:?}", item.operation).to_lowercase(),
                item.changed_at,
            ],
        )?;

        // Phase 4c: Warn if dirty count exceeds 5000
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM sync_state", [], |row| row.get(0))?;
        if count > 5000 {
            eprintln!("[WARN] offline dirty queue is very large: {count} records - consider syncing soon");
        }
        Ok(())
    }



    pub fn list_deleted_records(&self) -> anyhow::Result<Vec<DirtyRecord>> {
        let mut stmt = self.conn.prepare("SELECT record_id, entity_kind, operation, changed_at FROM deleted_records ORDER BY changed_at ASC")?;
        let rows = stmt.query_map([], |row| {
            let entity_kind: String = row.get(1)?;
            let operation: String = row.get(2)?;
            Ok(DirtyRecord {
                record_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                entity_kind: match entity_kind.as_str() {
                    "task" => SyncEntityKind::Task,
                    "log" => SyncEntityKind::Log,
                    _ => SyncEntityKind::Plant,
                },
                operation: match operation.as_str() {
                    "delete" => SyncOperation::Delete,
                    _ => SyncOperation::Upsert,
                },
                changed_at: row.get(3)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn list_dirty(&self) -> anyhow::Result<Vec<DirtyRecord>> {
        let mut stmt = self.conn.prepare("SELECT record_id, entity_kind, operation, changed_at FROM sync_state ORDER BY changed_at ASC")?;
        let rows = stmt.query_map([], |row| {
            let entity_kind: String = row.get(1)?;
            let operation: String = row.get(2)?;
            Ok(DirtyRecord {
                record_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                entity_kind: match entity_kind.as_str() {
                    "task" => SyncEntityKind::Task,
                    "log" => SyncEntityKind::Log,
                    _ => SyncEntityKind::Plant,
                },
                operation: match operation.as_str() {
                    "delete" => SyncOperation::Delete,
                    _ => SyncOperation::Upsert,
                },
                changed_at: row.get(3)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }


    pub fn dirty_breakdown(&self) -> anyhow::Result<DirtyBreakdown> {
        let dirty = self.list_dirty()?;
        let mut plants = 0usize;
        let mut tasks = 0usize;
        let mut logs = 0usize;
        let mut deletes = 0usize;

        for item in dirty {
            if matches!(item.operation, SyncOperation::Delete) {
                deletes += 1;
                continue;
            }
            match item.entity_kind {
                SyncEntityKind::Plant => plants += 1,
                SyncEntityKind::Task => tasks += 1,
                SyncEntityKind::Log => logs += 1,
            }
        }

        Ok(DirtyBreakdown { plants, tasks, logs, deletes })
    }


    pub fn list_dirty_details(&self) -> anyhow::Result<Vec<DirtyRecord>> {
        self.list_dirty()
    }

    pub fn clear_dirty(&self, record_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM sync_state WHERE record_id = ?1", params![record_id.to_string()])?;
        Ok(())
    }

    pub fn clear_all_dirty(&self) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM sync_state", [])?;
        Ok(())
    }

    pub fn clear_confirmed_upserts(&self, payload: &SyncEnvelope) -> anyhow::Result<()> {
        for plant in &payload.plants {
            self.clear_dirty(plant.plant_id)?;
        }
        for task in &payload.tasks {
            self.clear_dirty(task.task_id)?;
        }
        for log in &payload.logs {
            self.clear_dirty(log.log_id)?;
        }
        Ok(())
    }


    fn is_record_dirty(&self, record_id: Uuid) -> anyhow::Result<bool> {
        let mut stmt = self.conn.prepare("SELECT 1 FROM sync_state WHERE record_id = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![record_id.to_string()])?;
        Ok(rows.next()?.is_some())
    }

    pub fn clear_confirmed_deleted(&self, deleted: &[DirtyRecord]) -> anyhow::Result<()> {
        for item in deleted {
            self.conn.execute(
                "DELETE FROM deleted_records WHERE record_id = ?1 AND entity_kind = ?2",
                params![item.record_id.to_string(), format!("{:?}", item.entity_kind).to_lowercase()],
            )?;
            self.clear_dirty(item.record_id)?;
        }
        Ok(())
    }

    fn get_plant_by_id(&self, plant_id: Uuid) -> anyhow::Result<Option<Plant>> {
        let mut stmt = self.conn.prepare(
            "SELECT plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, created_at, updated_at, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type FROM plants WHERE plant_id = ?1 AND plant_status != 'deleted' LIMIT 1",
        )?;
        let mut rows = stmt.query(params![plant_id.to_string()])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(Plant {
                plant_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                user_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                name: row.get(2)?,
                color_tag: row.get(3)?,
                start_date: row.get(4)?,
                phase: match row.get::<_, String>(5)?.as_str() {
                    "seed" => PlantPhase::Seed,
                    "flower" => PlantPhase::Flower,
                    "harvest" => PlantPhase::Harvest,
                    "dry" => PlantPhase::Dry,
                    "cure" => PlantPhase::Cure,
                    "custom" => PlantPhase::Custom,
                    "dormant" => PlantPhase::Dormant,
                    _ => PlantPhase::Veg,
                },
                phase_week: row.get(6)?,
                status: parse_plant_status(&row.get::<_, String>(7)?, row.get::<_, i64>(8)? == 1, row.get::<_, i64>(9)? == 1),
                is_active: row.get::<_, i64>(8)? == 1,
                archived: row.get::<_, i64>(9)? == 1,
                location_id: row.get::<_, Option<String>>(12)?.and_then(|s| Uuid::parse_str(&s).ok()),
                lifecycle: parse_lifecycle(&row.get::<_, Option<String>>(13)?.unwrap_or_else(|| "annual".into())),
                plant_year: row.get(14)?,
                catalog_id: row.get(15)?,
                pot_size_liters: row.get(16)?,
                soil_type: row.get(17)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            }.normalized()));
        }
        Ok(None)
    }

    fn get_task_by_id(&self, task_id: Uuid) -> anyhow::Result<Option<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT task_id, plant_id, title, category, due_at, repeat_interval_hours, notification_enabled, status, created_at, updated_at, repeat_until FROM tasks WHERE task_id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![task_id.to_string()])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(Task {
                task_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                plant_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                title: row.get(2)?,
                category: match row.get::<_, String>(3)?.as_str() {
                    "feed" => TaskCategory::Feed,
                    "check" => TaskCategory::Check,
                    "train" => TaskCategory::Train,
                    "note" => TaskCategory::Note,
                    _ => TaskCategory::Water,
                },
                due_at: row.get(4)?,
                repeat_interval_hours: row.get(5)?,
                repeat_until: row.get(10)?,
                notification_enabled: row.get::<_, i64>(6)? == 1,
                status: match row.get::<_, String>(7)?.as_str() {
                    "done" => TaskStatus::Done,
                    "skipped" => TaskStatus::Skipped,
                    _ => TaskStatus::Open,
                },
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            }));
        }
        Ok(None)
    }

    fn get_log_by_id(&self, log_id: Uuid) -> anyhow::Result<Option<LogEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at FROM logs WHERE log_id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![log_id.to_string()])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(LogEntry {
                log_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                plant_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                log_type: match row.get::<_, String>(2)?.as_str() {
                    "measurement" => LogType::Measurement,
                    "harvest" => LogType::Harvest,
                    "watering" => LogType::Watering,
                    "photo" => LogType::Photo,
                    "action" => LogType::Action,
                    _ => LogType::Note,
                },
                text: row.get(3)?,
                metrics: row.get::<_, Option<String>>(4)?.and_then(|v| serde_json::from_str(&v).ok()),
                action: row.get::<_, Option<String>>(5)?.and_then(|v| serde_json::from_str(&v).ok()),
                photo_refs: row.get::<_, Option<String>>(6)?.and_then(|v| serde_json::from_str(&v).ok()),
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            }));
        }
        Ok(None)
    }

    pub fn export_sync_payload(&self, auth_user_id: Option<Uuid>, device_id: Option<String>) -> anyhow::Result<SyncPushRequest> {
        let existing_cursor = self.load_last_sync_cursor()?;
        let dirty_records = self.list_dirty()?;
        let mut plants = Vec::new();
        let mut tasks = Vec::new();
        let mut logs = Vec::new();
        let mut deleted_records = Vec::new();

        // Phase 4c: Limit to 500 records per batch
        let max_records: usize = 500;
        let dirty_records: Vec<_> = dirty_records.into_iter().take(max_records).collect();

        for dirty in dirty_records {
            match dirty.operation {
                SyncOperation::Delete => deleted_records.push(dirty),
                SyncOperation::Upsert => match dirty.entity_kind {
                    SyncEntityKind::Plant => {
                        if let Some(plant) = self.get_plant_by_id(dirty.record_id)? {
                            plants.push(plant.normalized());
                        }
                    }
                    SyncEntityKind::Task => {
                        if let Some(task) = self.get_task_by_id(dirty.record_id)? {
                            tasks.push(task);
                        }
                    }
                    SyncEntityKind::Log => {
                        if let Some(log) = self.get_log_by_id(dirty.record_id)? {
                            logs.push(log);
                        }
                    }
                },
            }
        }

        Ok(SyncPushRequest {
            cursor: SyncCursor {
                user_id: auth_user_id,
                device_id,
                last_synced_at: existing_cursor.and_then(|c| c.last_synced_at),
            },
            guest_user_id: Some(Uuid::nil()),
            envelope: SyncEnvelope {
                plants,
                tasks,
                logs,
            },
            deleted_records,
        })
    }

    pub fn apply_sync_pull(&self, payload: &SyncEnvelope, deleted: &[DirtyRecord], cursor: Option<&SyncCursor>) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for plant in &payload.plants {
            if self.is_record_dirty(plant.plant_id)? {
                continue;
            }
            let plant = plant.normalized_ref();
            // INSERT OR REPLACE rewrites the whole row, so every column must be
            // listed — omitting location_id/lifecycle/plant_year reset them to
            // NULL on every single pull.
            tx.execute(
                "INSERT OR REPLACE INTO plants (plant_id, user_id, name, color_tag, start_date, phase, phase_week, plant_status, is_active, archived, created_at, updated_at, location_id, lifecycle, plant_year, catalog_id, pot_size_liters, soil_type)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
                params![plant.plant_id.to_string(), plant.user_id.to_string(), plant.name, plant.color_tag, plant.start_date, format!("{:?}", plant.phase).to_lowercase(), plant.phase_week, plant_status_str(plant.status), i64::from(plant.is_active_state()), i64::from(plant.is_archived_state()), plant.created_at, plant.updated_at, plant.location_id.map(|id| id.to_string()), lifecycle_str(plant.lifecycle), plant.plant_year, &plant.catalog_id, plant.pot_size_liters, &plant.soil_type],
            )?;
        }
        for task in &payload.tasks {
            if self.is_record_dirty(task.task_id)? {
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO tasks (task_id, plant_id, title, category, due_at, repeat_interval_hours, repeat_until, notification_enabled, status, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![task.task_id.to_string(), task.plant_id.to_string(), task.title, format!("{:?}", task.category).to_lowercase(), task.due_at, task.repeat_interval_hours, task.repeat_until, i64::from(task.notification_enabled), format!("{:?}", task.status).to_lowercase(), task.created_at, task.updated_at],
            )?;
        }
        for log in &payload.logs {
            if self.is_record_dirty(log.log_id)? {
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO logs (log_id, plant_id, log_type, text, metrics_json, action_json, photo_refs_json, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![log.log_id.to_string(), log.plant_id.to_string(), format!("{:?}", log.log_type).to_lowercase(), log.text.as_deref(), log.metrics.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()), log.action.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()), log.photo_refs.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()), log.created_at, log.last_modified()],
            )?;
        }
        for item in deleted {
            if self.is_record_dirty(item.record_id)? {
                continue;
            }
            match item.entity_kind {
                SyncEntityKind::Plant => { tx.execute("DELETE FROM plants WHERE plant_id = ?1", params![item.record_id.to_string()])?; }
                SyncEntityKind::Task => { tx.execute("DELETE FROM tasks WHERE task_id = ?1", params![item.record_id.to_string()])?; }
                SyncEntityKind::Log => { tx.execute("DELETE FROM logs WHERE log_id = ?1", params![item.record_id.to_string()])?; }
            }
            tx.execute("DELETE FROM deleted_records WHERE record_id = ?1 AND entity_kind = ?2", params![item.record_id.to_string(), format!("{:?}", item.entity_kind).to_lowercase()])?;
        }
        tx.commit()?;
        if let Some(cursor) = cursor {
            self.save_last_sync_cursor(cursor)?;
        }
        Ok(())
    }

    fn reminder_delivery_key(task_id: Uuid, due_at: chrono::DateTime<Utc>) -> String {
        format!("reminder_sent:{}:{}", task_id, due_at.to_rfc3339())
    }

    pub fn list_due_notification_candidates(&self, within_minutes: i64) -> anyhow::Result<Vec<ReminderCandidate>> {
        let now = Utc::now();
        let upper = now + chrono::Duration::minutes(within_minutes);
        let tasks = self.list_tasks()?;
        let mut out = Vec::new();
        for task in tasks.into_iter() {
            if !task.notification_enabled || task.status != TaskStatus::Open {
                continue;
            }
            if task.due_at > upper {
                continue;
            }
            let key = Self::reminder_delivery_key(task.task_id, task.due_at);
            let mut stmt = self.conn.prepare("SELECT 1 FROM app_meta WHERE meta_key = ?1 LIMIT 1")?;
            let mut rows = stmt.query(params![key])?;
            if rows.next()?.is_some() {
                continue;
            }
            let minutes_until_due = (task.due_at - now).num_minutes();
            out.push(ReminderCandidate {
                task_id: task.task_id,
                plant_id: task.plant_id,
                title: task.title.clone(),
                category: format!("{:?}", task.category).to_lowercase(),
                due_at: task.due_at,
                overdue: task.due_at < now,
                minutes_until_due,
            });
        }
        out.sort_by_key(|item| item.due_at);
        Ok(out)
    }

    pub fn mark_notification_sent(&self, task_id: Uuid, due_at: chrono::DateTime<Utc>) -> anyhow::Result<()> {
        let now = Utc::now();
        let key = Self::reminder_delivery_key(task_id, due_at);
        self.conn.execute(
            "INSERT OR REPLACE INTO app_meta (meta_key, meta_value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, now.to_rfc3339(), now.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn clear_notification_sent_for_task(&self, task_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM app_meta WHERE meta_key LIKE ?1",
            params![format!("reminder_sent:{}:%", task_id)],
        )?;
        Ok(())
    }

    // ── Device CRUD ──────────────────────────────────────────────

    pub fn upsert_device(&self, device: &Device) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO devices (device_id, name, device_type, ble_address, linked_plant_id, last_seen, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,COALESCE((SELECT created_at FROM devices WHERE device_id = ?1), ?7),?8)",
            params![
                device.device_id.to_string(),
                &device.name,
                &device.device_type,
                &device.ble_address,
                device.linked_plant_id.map(|id| id.to_string()),
                device.last_seen.to_rfc3339(),
                device.created_at.to_rfc3339(),
                device.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_devices(&self) -> anyhow::Result<Vec<Device>> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, name, device_type, ble_address, linked_plant_id, last_seen, created_at, updated_at FROM devices ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Device {
                device_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                name: row.get(1)?,
                device_type: row.get(2)?,
                ble_address: row.get(3)?,
                linked_plant_id: row.get::<_, Option<String>>(4)?
                    .and_then(|s| Uuid::parse_str(&s).ok()),
                last_seen: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn get_device(&self, device_id: Uuid) -> anyhow::Result<Option<Device>> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, name, device_type, ble_address, linked_plant_id, last_seen, created_at, updated_at FROM devices WHERE device_id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![device_id.to_string()])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(Device {
                device_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                name: row.get(1)?,
                device_type: row.get(2)?,
                ble_address: row.get(3)?,
                linked_plant_id: row.get::<_, Option<String>>(4)?
                    .and_then(|s| Uuid::parse_str(&s).ok()),
                last_seen: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            }));
        }
        Ok(None)
    }

    pub fn link_device_to_plant(&self, device_id: Uuid, plant_id: Option<Uuid>) -> anyhow::Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE devices SET linked_plant_id = ?2, updated_at = ?3 WHERE device_id = ?1",
            params![
                device_id.to_string(),
                plant_id.map(|id| id.to_string()),
                now.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_device(&self, device_id: Uuid) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM devices WHERE device_id = ?1", params![device_id.to_string()])?;
        Ok(())
    }

    // ── Device Readings CRUD ─────────────────────────────────────

    pub fn insert_reading(&self, reading: &DeviceReading) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO device_readings (reading_id, device_id, plant_id, temp, humidity, vpd, fan_level, light_level, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                reading.reading_id.to_string(),
                reading.device_id.to_string(),
                reading.plant_id.map(|id| id.to_string()),
                reading.temp,
                reading.humidity,
                reading.vpd,
                reading.fan_level,
                reading.light_level,
                reading.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_readings_for_device(&self, device_id: Uuid, limit: usize) -> anyhow::Result<Vec<DeviceReading>> {
        let mut stmt = self.conn.prepare(
            "SELECT reading_id, device_id, plant_id, temp, humidity, vpd, fan_level, light_level, created_at
             FROM device_readings WHERE device_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![device_id.to_string(), limit as i64], |row| {
            Ok(DeviceReading {
                reading_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                device_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                plant_id: row.get::<_, Option<String>>(2)?
                    .and_then(|s| Uuid::parse_str(&s).ok()),
                temp: row.get(3)?,
                humidity: row.get(4)?,
                vpd: row.get(5)?,
                fan_level: row.get(6)?,
                light_level: row.get(7)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn list_readings_for_plant(&self, plant_id: Uuid, limit: usize) -> anyhow::Result<Vec<DeviceReading>> {
        let mut stmt = self.conn.prepare(
            "SELECT reading_id, device_id, plant_id, temp, humidity, vpd, fan_level, light_level, created_at
             FROM device_readings WHERE plant_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![plant_id.to_string(), limit as i64], |row| {
            Ok(DeviceReading {
                reading_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                device_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or(Uuid::nil()),
                plant_id: row.get::<_, Option<String>>(2)?
                    .and_then(|s| Uuid::parse_str(&s).ok()),
                temp: row.get(3)?,
                humidity: row.get(4)?,
                vpd: row.get(5)?,
                fan_level: row.get(6)?,
                light_level: row.get(7)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn prune_old_readings(&self, days: i64) -> anyhow::Result<()> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        self.conn.execute(
            "DELETE FROM device_readings WHERE created_at < ?1",
            params![cutoff],
        )?;
        Ok(())
    }

    // ── Photos (local-only) ─────────────────────────────────────

    pub fn insert_photo(&self, photo: &PhotoRef) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO photos (photo_id, plant_id, log_id, season_id, file_path, caption, taken_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                photo.photo_id.to_string(),
                photo.plant_id.to_string(),
                photo.log_id.map(|id| id.to_string()),
                photo.season_id.map(|id| id.to_string()),
                photo.file_path,
                photo.caption,
                photo.taken_at,
                photo.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_photos_for_plant(&self, plant_id: Uuid) -> anyhow::Result<Vec<PhotoRef>> {
        let mut stmt = self.conn.prepare(
            "SELECT photo_id, plant_id, log_id, season_id, file_path, caption, taken_at, created_at
             FROM photos WHERE plant_id = ?1 ORDER BY taken_at DESC",
        )?;
        let rows = stmt.query_map(params![plant_id.to_string()], |row| {
            Ok(PhotoRef {
                photo_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                plant_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                log_id: row.get::<_, Option<String>>(2)?.and_then(|s| Uuid::parse_str(&s).ok()),
                season_id: row.get::<_, Option<String>>(3)?.and_then(|s| Uuid::parse_str(&s).ok()),
                file_path: row.get(4)?,
                caption: row.get(5)?,
                taken_at: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn list_photos_for_log(&self, log_id: Uuid) -> anyhow::Result<Vec<PhotoRef>> {
        let mut stmt = self.conn.prepare(
            "SELECT photo_id, plant_id, log_id, season_id, file_path, caption, taken_at, created_at
             FROM photos WHERE log_id = ?1 ORDER BY taken_at DESC",
        )?;
        let rows = stmt.query_map(params![log_id.to_string()], |row| {
            Ok(PhotoRef {
                photo_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                plant_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                log_id: row.get::<_, Option<String>>(2)?.and_then(|s| Uuid::parse_str(&s).ok()),
                season_id: row.get::<_, Option<String>>(3)?.and_then(|s| Uuid::parse_str(&s).ok()),
                file_path: row.get(4)?,
                caption: row.get(5)?,
                taken_at: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn delete_photo(&self, photo_id: Uuid) -> anyhow::Result<String> {
        let file_path: String = self.conn.query_row(
            "SELECT file_path FROM photos WHERE photo_id = ?1",
            params![photo_id.to_string()],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "DELETE FROM photos WHERE photo_id = ?1",
            params![photo_id.to_string()],
        )?;
        Ok(file_path)
    }

    pub fn count_photos_for_plant(&self, plant_id: Uuid) -> anyhow::Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM photos WHERE plant_id = ?1",
            params![plant_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn get_climate_config(&self) -> anyhow::Result<Option<ClimateConfig>> {
        let mut stmt = self.conn.prepare(
            "SELECT latitude, longitude, city_name, last_spring_frost, first_fall_frost, timezone FROM climate_config WHERE config_key = 'default'",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let lat: Option<f64> = row.get(0)?;
            let lon: Option<f64> = row.get(1)?;
            match (lat, lon) {
                (Some(latitude), Some(longitude)) => Ok(Some(ClimateConfig {
                    latitude,
                    longitude,
                    city_name: row.get(2)?,
                    last_spring_frost: row.get(3)?,
                    first_fall_frost: row.get(4)?,
                    timezone: row.get(5)?,
                })),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub fn save_climate_config(&self, config: &ClimateConfig) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO climate_config (config_key, latitude, longitude, city_name, last_spring_frost, first_fall_frost, timezone, updated_at)
             VALUES ('default', ?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(config_key) DO UPDATE SET
               latitude = excluded.latitude,
               longitude = excluded.longitude,
               city_name = excluded.city_name,
               last_spring_frost = excluded.last_spring_frost,
               first_fall_frost = excluded.first_fall_frost,
               timezone = excluded.timezone,
               updated_at = excluded.updated_at",
            params![
                config.latitude,
                config.longitude,
                config.city_name,
                config.last_spring_frost,
                config.first_fall_frost,
                config.timezone,
                now,
            ],
        )?;
        Ok(())
    }

    // ── Weather History ──

    pub fn insert_weather_record(&self, record: &WeatherRecord) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO weather_history (
                id, location_id, latitude, longitude, temp_c, humidity, wind_kmh,
                precipitation_mm, weather_code, recorded_at,
                apparent_temp_c, uv_index, cloud_cover, pressure_hpa, dew_point_c,
                visibility_km, wind_gusts_kmh, wind_direction_deg, soil_temp_c, soil_moisture, is_day)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
            params![
                record.id.to_string(),
                record.location_id.map(|id| id.to_string()),
                record.latitude,
                record.longitude,
                record.temp_c,
                record.humidity,
                record.wind_kmh,
                record.precipitation_mm,
                record.weather_code,
                record.recorded_at.to_rfc3339(),
                record.apparent_temp_c,
                record.uv_index,
                record.cloud_cover,
                record.pressure_hpa,
                record.dew_point_c,
                record.visibility_km,
                record.wind_gusts_kmh,
                record.wind_direction_deg,
                record.soil_temp_c,
                record.soil_moisture,
                record.is_day.map(|v| if v { 1i32 } else { 0i32 }),
            ],
        )?;
        Ok(())
    }

    pub fn list_weather_history(&self, location_id: Option<Uuid>, hours: i64) -> anyhow::Result<Vec<WeatherRecord>> {
        let cutoff = (Utc::now() - chrono::Duration::hours(hours)).to_rfc3339();
        let records = if let Some(lid) = location_id {
            let mut stmt = self.conn.prepare(
                "SELECT id, location_id, latitude, longitude, temp_c, humidity, wind_kmh, precipitation_mm, weather_code, recorded_at,
                 apparent_temp_c, uv_index, cloud_cover, pressure_hpa, dew_point_c, visibility_km, wind_gusts_kmh, wind_direction_deg, soil_temp_c, soil_moisture, is_day
                 FROM weather_history WHERE location_id = ?1 AND recorded_at >= ?2 ORDER BY recorded_at ASC"
            )?;
            let rows = stmt.query_map(params![lid.to_string(), cutoff], |row| {
                Ok(WeatherRecord {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                    location_id: row.get::<_, Option<String>>(1)?.and_then(|s| Uuid::parse_str(&s).ok()),
                    latitude: row.get(2)?,
                    longitude: row.get(3)?,
                    temp_c: row.get(4)?,
                    humidity: row.get(5)?,
                    wind_kmh: row.get(6)?,
                    precipitation_mm: row.get(7)?,
                    weather_code: row.get(8)?,
                    recorded_at: row.get::<_, String>(9)?.parse().unwrap_or_else(|_| Utc::now()),
                    apparent_temp_c: row.get(10).ok(),
                    uv_index: row.get(11).ok(),
                    cloud_cover: row.get(12).ok(),
                    pressure_hpa: row.get(13).ok(),
                    dew_point_c: row.get(14).ok(),
                    visibility_km: row.get(15).ok(),
                    wind_gusts_kmh: row.get(16).ok(),
                    wind_direction_deg: row.get(17).ok(),
                    soil_temp_c: row.get(18).ok(),
                    soil_moisture: row.get(19).ok(),
                    is_day: row.get::<_, Option<i32>>(20).ok().flatten().map(|v| v != 0),
                })
            })?;
            rows.flatten().collect()
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, location_id, latitude, longitude, temp_c, humidity, wind_kmh, precipitation_mm, weather_code, recorded_at,
                 apparent_temp_c, uv_index, cloud_cover, pressure_hpa, dew_point_c, visibility_km, wind_gusts_kmh, wind_direction_deg, soil_temp_c, soil_moisture, is_day
                 FROM weather_history WHERE location_id IS NULL AND recorded_at >= ?1 ORDER BY recorded_at ASC"
            )?;
            let rows = stmt.query_map(params![cutoff], |row| {
                Ok(WeatherRecord {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or(Uuid::nil()),
                    location_id: row.get::<_, Option<String>>(1)?.and_then(|s| Uuid::parse_str(&s).ok()),
                    latitude: row.get(2)?,
                    longitude: row.get(3)?,
                    temp_c: row.get(4)?,
                    humidity: row.get(5)?,
                    wind_kmh: row.get(6)?,
                    precipitation_mm: row.get(7)?,
                    weather_code: row.get(8)?,
                    recorded_at: row.get::<_, String>(9)?.parse().unwrap_or_else(|_| Utc::now()),
                    apparent_temp_c: row.get(10).ok(),
                    uv_index: row.get(11).ok(),
                    cloud_cover: row.get(12).ok(),
                    pressure_hpa: row.get(13).ok(),
                    dew_point_c: row.get(14).ok(),
                    visibility_km: row.get(15).ok(),
                    wind_gusts_kmh: row.get(16).ok(),
                    wind_direction_deg: row.get(17).ok(),
                    soil_temp_c: row.get(18).ok(),
                    soil_moisture: row.get(19).ok(),
                    is_day: row.get::<_, Option<i32>>(20).ok().flatten().map(|v| v != 0),
                })
            })?;
            rows.flatten().collect()
        };
        Ok(records)
    }

    pub fn prune_weather_history(&self, days: i64) -> anyhow::Result<usize> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let deleted = self.conn.execute(
            "DELETE FROM weather_history WHERE recorded_at < ?1",
            params![cutoff],
        )?;
        Ok(deleted)
    }

    // ── GDPR consent CRUD (Phase C2) ──

    /// Lists the latest consent record for each (consent_type, policy_version)
    /// combination belonging to the given user.
    pub fn list_consents(&self, user_id: Uuid) -> anyhow::Result<Vec<ConsentRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT consent_id, user_id, consent_type, granted, policy_version, granted_at, revoked_at, updated_at \
             FROM user_consents WHERE user_id = ?1 ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map(params![user_id.to_string()], |row| {
            Ok(ConsentRecord {
                consent_id: row.get::<_, String>(0)?,
                user_id: row.get::<_, String>(1)?,
                consent_type: row.get(2)?,
                granted: row.get::<_, i64>(3)? == 1,
                policy_version: row.get(4)?,
                granted_at: row.get(5)?,
                revoked_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    /// Inserts or updates a consent record for (user_id, consent_type, policy_version).
    /// Revoking sets `granted = 0` and records `revoked_at`.
    pub fn upsert_consent(&self, user_id: Uuid, consent_type: &str, granted: bool, policy_version: &str) -> anyhow::Result<ConsentRecord> {
        let now = Utc::now();
        let now_iso = now.to_rfc3339();
        // Does it already exist?
        let existing: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT consent_id, granted_at FROM user_consents WHERE user_id = ?1 AND consent_type = ?2 AND policy_version = ?3",
                params![user_id.to_string(), consent_type, policy_version],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .ok();
        let (consent_id, granted_at) = match existing {
            Some((id, prev_granted_at)) => (id, prev_granted_at),
            None => (Uuid::new_v4().to_string(), now_iso.clone()),
        };
        let revoked_at = if granted { None } else { Some(now_iso.clone()) };
        self.conn.execute(
            "INSERT INTO user_consents (consent_id, user_id, consent_type, granted, policy_version, granted_at, revoked_at, updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
             ON CONFLICT(user_id, consent_type, policy_version) DO UPDATE SET \
               granted = excluded.granted, revoked_at = excluded.revoked_at, updated_at = excluded.updated_at",
            params![
                consent_id,
                user_id.to_string(),
                consent_type,
                if granted { 1_i64 } else { 0_i64 },
                policy_version,
                granted_at,
                revoked_at,
                now_iso,
            ],
        )?;
        Ok(ConsentRecord {
            consent_id,
            user_id: user_id.to_string(),
            consent_type: consent_type.to_string(),
            granted,
            policy_version: policy_version.to_string(),
            granted_at,
            revoked_at,
            updated_at: now_iso,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsentRecord {
    pub consent_id: String,
    pub user_id: String,
    pub consent_type: String,
    pub granted: bool,
    pub policy_version: String,
    pub granted_at: String,
    pub revoked_at: Option<String>,
    pub updated_at: String,
}

pub fn demo_task(plant_id: Uuid) -> Task {
    let now = Utc::now();
    Task {
        task_id: Uuid::new_v4(),
        plant_id,
        title: "Gießen".into(),
        category: TaskCategory::Water,
        due_at: now,
        repeat_interval_hours: Some(48),
        repeat_until: None,
        notification_enabled: true,
        status: TaskStatus::Open,
        created_at: now,
        updated_at: now,
    }
}

pub fn demo_log(plant_id: Uuid) -> LogEntry {
    LogEntry {
        log_id: Uuid::new_v4(),
        plant_id,
        log_type: LogType::Note,
        text: Some("Erster lokaler Eintrag".into()),
        metrics: None,
        action: None,
        photo_refs: None,
        created_at: Utc::now(),
        updated_at: None,
    }
}
