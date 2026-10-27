# Sowling

**Der intelligenteste Pflanzenkalender fuer Desktop und Web — von Hobbygartnern bis zu kommerziellen Anbauern.**

Entwickelt von BlockSocial UG (haftungsbeschraenkt) — info@blocksocial.eu

---

## Features

### Kalender & Planung
- Kalender als Startbildschirm (Monats-/Wochen-/Tagesansicht)
- 8 Kalendermodi: Standard, Mondkalender, Chinesisch, Persisch, Julianisch, Islamisch, Juedisch, Bauernkalender
- Klima-basierte Pflanzplanung mit Open-Meteo Wetterdaten
- Wiederkehrende Tasks mit Enddatum und Kalender-Projektion
- Floating Action Button (FAB) fuer schnelles Erstellen
- Tag-Detail als zentriertes Modal-Popup

### Pflanzen-Management
- 34 Pflanzen im Katalog (Gemuese, Kraeuter, Obst, Beeren, Blumen) mit Emojis
- Mehrjaehrige Pflanzen mit Saisonverlauf ueber Jahre
- Companion Planting (Mischkultur) mit Gruen/Rot-System
- Pflanzenstatus: Aktiv, Beendet, Archiviert, Geloescht
- Topfgroesse und Substrat-Tracking

### Standorte
- Standortverwaltung: Growzelt, Gewaechshaus, Outdoor, Indoor, Balkon
- Pflanzen einem Standort zuweisen
- Pro Standort eigene Wetter-Koordinaten

### Intelligente Bewaesserung
- Dynamischer Bewaesserungs-Kalkulator (Wetter, Topfgroesse, Substrat, Pflanzenart, Saison)
- Ampel-System: Gruen/Gelb/Rot pro Pflanze
- Urlaubsmodus (verlaengert Intervalle automatisch)
- Auto-Bewaesserungslog bei Task-Erledigung

### Ernte & Wachstum
- Ernte-Tracking (kg, Stueck, Qualitaet)
- Wasser-Tracking (Liter, Methode)
- Ernteprognose basierend auf Katalogdaten
- Wachstums-Analytics mit 90-Tage Aktivitaets-Heatmap
- Pflege-Insights mit automatischer Fehlererkennung

### Foto-Tagebuch
- Chronologische Foto-Timeline pro Pflanze
- Wachstumsfortschritt visualisieren (Tag X seit Start)
- Fotos komplett lokal gespeichert (kein Server-Upload)

### Wetter
- Echtzeit-Wetterdaten via Open-Meteo (kostenlos, kein API-Key)
- 24h Stundenvorhersage + 7-Tage Vorhersage
- Historische Wetterdaten mit Charts
- Multi-Standort Wetter mit automatischem stuendlichem Logging

### Smarte Extras
- Allergie- und Haustier-Check (giftig fuer Katzen/Hunde)
- Vermehrungsguide (Stecklinge, Ableger, Samen)
- Lichtmesser via Kamera
- Nachhaltigkeits-Tracker (CO2, Wassereffizienz)
- CSV Import/Export (kompatibel mit anderen Garten-Apps)
- 8 Design-Themes (Forest, Ocean, Sunset, Lavender, Rose, Midnight, Earth, Arctic)
- Hinweistoene (Web Audio API)
- Multi-User Profile (Team-Management)

### Sicherheit
- Argon2 Passwort-Hashing
- JWT mit Refresh-Token-Rotation
- CORS, Rate-Limiting, XSS-Schutz (CSP, escapeHtml)
- IDOR-Schutz, Webhook-Signaturvalidierung
- GDPR-konforme Account-Loeschung

### Billing
- Free (1 Pflanze) / Basic (3) / Pro (100) / CSC (unbegrenzt)
- Stripe Checkout mit Webhook-Validierung
- Customer Portal, Grandfather-Regel bei Downgrade

---

## Schnellstart

### Voraussetzungen
- Rust (stable) — https://rustup.rs/
- Node.js 18+ — https://nodejs.org/
- Optional: PostgreSQL (fuer Backend/Sync)

### Desktop-App starten
```bash
cd desktop-ui
npm install
cd src-tauri
cargo tauri dev
```

### Backend starten (optional, fuer Login/Sync)
```bash
export DATABASE_URL=postgres://user:pass@localhost/plants_calendar
cargo run -p backend
```

### Tests ausfuehren
```bash
cargo test --workspace        # 36 Rust-Tests
cd desktop-ui
npm run check                 # JS Syntax-Check
npm run test:smoke            # Smoke-Tests
```

---

## Projektstruktur

```
shared/                  Gemeinsame Domaenenlogik (Rust)
  src/domain.rs          Plant, Task, Log, Location, Profile, etc.
  src/catalog.rs         34 Pflanzen-Katalog + Companion Planting + Toxizitaet
  src/rules.rs           Geschaeftsregeln, Plan-Limits
  src/ggs.rs             GGS Controller BLE Parser
  src/sync.rs            Sync-Typen

backend/                 Axum HTTP-Server
  src/api/               REST-Endpoints
  src/db/                PostgreSQL Repos + Models
  src/services/          Auth, Stripe, Rate-Limiting

desktop-ui/              Tauri Desktop-App
  src/main.js            Frontend (Vanilla JS, ~8000 Zeilen)
  src/style.css          CSS mit Theme-System (8 Themes, 30+ Variablen)
  src-tauri/
    src/main.rs           Tauri Commands (~100 Commands)
    src/local_db.rs       SQLite LocalStore (~80 Methoden)
    src/ble.rs            BLE Scanner + Telemetrie
    src/tuya_local.rs     Tuya Local Protocol

migrations/              19 SQL-Migrationen (SQLite + Postgres)
docs/                    Dokumentation
```

---

## Technologie-Stack

| Komponente | Technologie |
|------------|-------------|
| Desktop | Tauri 2 (Rust + Webview) |
| Backend | Axum 0.8 (Rust) + PostgreSQL |
| Lokal-DB | SQLite (rusqlite) |
| Frontend | Vanilla JavaScript |
| Sync | Push/Pull mit Last-Write-Wins |
| Wetter | Open-Meteo API (kostenlos) |
| Payment | Stripe (Checkout + Webhooks) |
| Charts | Chart.js |
| IoT | BLE (btleplug) + Tuya Local (experimentell) |

## Dokumentation

- [Benutzerhandbuch (DE)](docs/USER_GUIDE_DE.md)
- [Developer Guide (EN)](docs/DEVELOPER_GUIDE.md)
- [API Reference](docs/API_REFERENCE.md)
- [Changelog](docs/CHANGELOG.md)

## Lizenz

Proprietaer — BlockSocial UG (haftungsbeschraenkt)
