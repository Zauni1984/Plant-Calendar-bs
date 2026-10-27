# Developer Guide — Sowling

> **Naming:** The app was called "Plants Calendar" before it was renamed to
> **Sowling**. Code, identifiers and the database now use the new name throughout.

## 1. Architecture Overview

The project is a Rust workspace with three crates:

- **shared** — Domain types, business rules, plant catalog, sync types. Used by both backend and desktop.
- **backend** — Axum HTTP server with PostgreSQL. Handles auth, sync, billing.
- **desktop-ui/src-tauri** — Tauri 2 desktop app with local SQLite database. Offline-first.

The frontend is a single `main.js` file (~8000 lines) using vanilla JavaScript (no framework).

```
User -> Tauri Webview (main.js)
          |
          v
     Tauri Commands (main.rs)
          |
          v
     LocalStore (local_db.rs / SQLite)
          |
          v (if authenticated)
     Backend API (Axum / PostgreSQL)
```

## 2. Building & Running

### Prerequisites
- Rust stable (rustup.rs)
- Node.js 18+
- Optional: PostgreSQL 15+ (for backend)

### Desktop App
```bash
cd desktop-ui
npm install                    # Install @tauri-apps/api, Chart.js, etc.
cd src-tauri
cargo tauri dev                # Build + run with hot-reload
```

### Backend
```bash
export DATABASE_URL=postgres://user:pass@localhost/sowling
export JWT_SECRET=your-secret-key
export CORS_ALLOWED_ORIGINS=http://localhost:1420
# Optional: STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET
cargo run -p backend
```

### Tests
```bash
cargo test --workspace         # 36 tests (30 shared + 6 backend)
cd desktop-ui
npm run check                  # node --check src/main.js
npm run test:smoke             # Smoke status tests
```

## 3. Database & Migrations

### SQLite (Desktop)
Migrations are applied in `local_db.rs::LocalStore::open()` on every startup.
All migrations use `IF NOT EXISTS` or duplicate-column guards for idempotency.

Current migrations (001-019):
- 001: Base tables (users, plants, tasks, logs)
- 002: Tombstones (deleted_records)
- 004: Session storage
- 005: Sync cursor (app_meta)
- 006-007: Plant status + backfill
- 008: Sync state
- 009: Refresh tokens
- 010: Devices + device readings
- 011: Locations
- 012: Perennial seasons
- 013: Photos
- 014: Catalog ID on plants
- 015: Climate config
- 016: Repeat until on tasks
- 017: Weather history + location coordinates
- 018: Plant care (pot_size, soil_type)
- 019: User profiles

### PostgreSQL (Backend)
Migrations are in `migrations/NNN_postgres_*.sql`. Applied by the backend on startup.

### Adding a New Migration
1. Create `migrations/NNN_sqlite_name.sql` and optionally `NNN_postgres_name.sql`
2. In `local_db.rs::open()`, add:
```rust
match conn.execute_batch(include_str!("../../../migrations/NNN_sqlite_name.sql")) {
    Ok(_) => {}
    Err(e) if e.to_string().contains("duplicate column") => {}
    Err(e) => return Err(e.into()),
}
```
3. For Postgres, add `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` syntax.

## 4. Adding a New Feature (Step by Step)

Example: Adding a "notes" field to locations.

### Step 1: Domain Type
In `shared/src/domain.rs`, add field to struct:
```rust
#[serde(default)]
pub notes: Option<String>,
```

### Step 2: Migration
Create `migrations/020_sqlite_location_notes.sql`:
```sql
ALTER TABLE locations ADD COLUMN notes TEXT;
```

### Step 3: Local DB
In `local_db.rs`:
- Apply migration in `open()`
- Update SELECT/INSERT queries in `list_locations()`, `upsert_location()`

### Step 4: Tauri Command
In `main.rs`:
- Update input structs if needed
- Update command handlers

### Step 5: Frontend
In `main.js`:
- Update form HTML
- Update render functions
- Update handlers

### Step 6: Backend (optional)
Same pattern in `repos.rs`, `api/mod.rs`, `main.rs`.

### Step 7: Test
```bash
cargo check --workspace
cargo test --workspace
cd desktop-ui && npm run check
```

## 5. Adding a Catalog Entry

In `shared/src/catalog.rs`, add to the `plant_catalog()` function:

```rust
entries.push(CatalogEntry {
    id: "new_plant".into(),
    name_de: "Neue Pflanze".into(),
    name_en: "New Plant".into(),
    emoji: "🌱".into(),
    category: "gemuese".into(),
    lifecycle: "annual".into(),
    default_phase: "seed".into(),
    sow_indoor_months: vec![3, 4],
    sow_outdoor_months: vec![5, 6],
    harvest_months: vec![8, 9],
    sun: "full".into(),
    water: "medium".into(),
    spacing_cm: 30,
    good_neighbors: vec!["tomato".into(), "basil".into()],
    bad_neighbors: vec!["fennel".into()],
    tips_de: "Pflegetipp auf Deutsch.".into(),
    color_suggestion: "#27ae60".into(),
    toxic_cats: false,
    toxic_dogs: false,
    toxic_humans: false,
    allergen: false,
    safety_note: "".into(),
    propagation_methods: vec!["samen".into()],
    propagation_note: "Ueber Samen vermehren.".into(),
});
```

Run `cargo test -p shared` to verify.

## 6. Theme System

Themes are defined in `desktop-ui/index.html` as CSS custom properties on `[data-theme="name"]` selectors.

Each theme defines 30+ variables:
```css
[data-theme="ocean"] {
    --bg: #0a1220;
    --accent: #64b5f6;
    --accent-rgb: 100, 181, 246;
    --text: #e3f0ff;
    /* ... 27 more */
}
```

In JavaScript, `getThemeColors()` reads these into a `TC` object.
All inline styles use `${TC.accent}` instead of hardcoded colors.

To add a theme:
1. Add CSS variables in `index.html`
2. Add to `THEMES` array in `main.js`
3. Done — the system handles the rest.

## 7. Tauri Command Pattern

All Tauri commands follow this pattern:

```rust
#[tauri::command]
fn command_name(state: State<AppLocalState>, param: String) -> Result<ReturnType, String> {
    let store = state.store.lock().map_err(|_| "lock".to_string())?;
    // ... do work with store ...
    Ok(result)
}

// For async commands (network calls):
#[tauri::command]
async fn async_command(state: State<'_, AppLocalState>) -> Result<Value, String> {
    let data = {
        let store = state.store.lock().map_err(|_| "lock".to_string())?;
        store.get_data().map_err(|e| e.to_string())?
    }; // MutexGuard dropped before .await
    let result = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    Ok(result)
}
```

Key rules:
- Always use block scoping `{ }` to drop MutexGuard before `.await`
- Return `Result<T, String>` (Tauri requires String errors)
- Register in `invoke_handler` in `main()`

## 8. Security Considerations

- All user strings displayed via innerHTML must use `escapeHtml()`
- Color values from users must use `sanitizeColorTag()` (hex validation)
- Backend returns generic error messages (no internal details)
- JWT algorithm explicitly set to HS256
- Refresh tokens rotated on each use
- Webhook signatures validated with timestamp check (5 min window)
- Rate limiter cleans up stale entries

## 9. Deployment

### Desktop Build
```bash
cd desktop-ui/src-tauri
cargo tauri build
```
Produces installer in `target/release/bundle/`.

### Backend Deployment
Standard Rust binary deployment:
```bash
cargo build -p backend --release
# Set environment variables:
# DATABASE_URL, JWT_SECRET, CORS_ALLOWED_ORIGINS
# Optional: STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET
./target/release/backend
```

The backend listens on `0.0.0.0:8080`.
