# Changelog

## v1.0.0 (2026-04-12)

### Architektur & Infrastruktur
- Rust Workspace mit 3 Crates: shared, backend, desktop-ui/src-tauri
- 19 SQL-Migrationen (SQLite + PostgreSQL)
- Tauri 2 Desktop-App mit Offline-First Architektur
- Axum 0.8 Backend mit PostgreSQL

### Security-Haertung
- Argon2id Passwort-Hashing (Migration-Shim fuer alte SHA256-Hashes)
- JWT mit Refresh-Token-Rotation (90 Tage Refresh, 30 Tage Access)
- IDOR-Schutz: Ownership-Checks auf allen Task/Log/Season-Endpoints
- CORS-Einschraenkung (konfigurierbar via CORS_ALLOWED_ORIGINS)
- Rate-Limiting: 10 req/60s auf Auth-Endpoints, In-Memory mit Cleanup
- Input-Validierung: Email-Format, Passwort 8-256 Zeichen, Textlaengen
- XSS-Schutz: escapeHtml() auf allen User-Strings, sanitizeColorTag()
- Content-Security-Policy in Tauri konfiguriert
- Stripe Webhook-Signaturvalidierung mit Timestamp-Check (5 Min)
- Stripe-Fehlermeldungen sanitisiert (keine internen Details an Client)

### Auth & Account
- Refresh-Token-Rotation mit Revocation
- Google OAuth Stub (NOT_IMPLEMENTED, vorbereitet)
- Profil-Update (Locale, Timezone)
- Passwort-Aenderung (mit aktuellem Passwort)
- GDPR Account-Loeschung (CASCADE)
- Login/Register als Modal-Popup

### Stripe Billing
- Stripe Checkout Sessions via reqwest
- Webhook-Signaturvalidierung (HMAC-SHA256)
- Subscription-Lifecycle: checkout.completed, subscription.updated/deleted
- Customer Portal Sessions
- Plan-Enforcement ueber validierte Webhooks
- Fallback auf Stub-Modus ohne STRIPE_SECRET_KEY

### Sync & Notifications
- Postgres-Transaktionen um gesamten sync_push
- Last-Write-Wins mit WHERE updated_at < EXCLUDED Klausel
- Payload-Groessenlimit (500 Records pro Push)
- OS-Desktop-Notifications via tauri-plugin-notification
- 5-Minuten Polling mit HMR-Guard
- Offline-Queue-Warnung bei >5000 Dirty-Records

### UI-Redesign
- Kalender als Startbildschirm (ersetzt Dashboard)
- 4-View Navigation: Kalender, Pflanzen, Standorte, Konto (spaeter erweitert)
- Floating Action Button (FAB) mit 3 Optionen
- Modale Formulare (ersetzt Sidebar)
- Onboarding-Overlay bei leerer DB
- Day-Detail als zentriertes Modal-Popup
- Loading-Spinner, Empty-States mit Aktions-Buttons
- Settings-Seite: Profil, Passwort, Account-Loeschung

### Theme-System (8 Moods)
- Forest (Standard), Ocean, Sunset, Lavender, Rose, Midnight, Earth, Arctic
- 30+ CSS-Variablen pro Theme
- 400+ hardcoded Farben durch TC.* Referenzen ersetzt
- Theme in localStorage persistiert
- Sofortiges Umschalten ohne Reload

### Kalender-Features
- 8 Kalendermodi: Standard, Mond, Chinesisch, Persisch, Julianisch, Islamisch, Juedisch, Bauernkalender
- Mondphasen-Berechnung (rein JS, kein API)
- Bauernkalender mit 40+ Lostagen und Bauernregeln
- Wiederkehrende Tasks mit Kalender-Projektion (max 500 Vorkommen)
- Mini-Wetter-Widget in Kalender-Toolbar

### Pflanzen-Katalog
- 34 Eintraege: 16 Gemuese, 7 Kraeuter, 5 Obst/Beeren, 3 Blumen, Cannabis
- Companion Planting mit good/bad_neighbors
- Emojis fuer alle Sorten
- Aussaat-/Erntemonate, Abstand, Sonne/Wasser
- Toxizitaet (Katzen, Hunde, Menschen, Allergien)
- Vermehrungsmethoden + Anleitungen

### Mehrjaehrige Pflanzen
- PlantLifecycle: Annual / Perennial
- Saisonverlauf (plant_seasons Tabelle)
- Ernte-History ueber Jahre
- Schnitt-Protokollierung
- Dormant-Phase (Ruhephase)
- Pflanzenalter-Berechnung

### Standorte (Locations)
- Eigener Navigation-Tab
- Typen: Growzelt, Gewaechshaus, Outdoor, Indoor, Balkon, Custom
- Pflanzen einem Standort zuweisen
- Geo-Koordinaten pro Standort (fuer Wetter)

### Foto-Tagebuch
- Fotos lokal gespeichert (kein Server-Upload)
- Chronologische Timeline mit Tag-X-Anzeige
- Tauri File-Dialog fuer Bildauswahl
- Fotos an Logs anhaengen
- Loeschen mit Datei-Bereinigung

### Ernte- & Wasser-Tracking
- Log-Typen: Notiz, Messung, Ernte, Bewaesserung
- Ernte: kg, Stueck, Qualitaet
- Bewaesserung: Liter, Methode (Giessen/Tropf/Spruehen/Regen)
- Zusammenfassungen im Pflanzen-Detail
- Ernte/Wasser-Badges im Kalender
- Quick-Action-Buttons im Day-Detail

### Intelligente Bewaesserung
- Dynamischer Kalkulator: Wetter, Topfgroesse, Substrat, Katalog, Saison
- Ampel-System (Gruen/Gelb/Rot) auf Pflanzenkarten
- Auto-Bewaesserungslog bei Task-Erledigung
- Urlaubsmodus (1.5x Intervalle)

### Wetter-System
- Open-Meteo Integration (kostenlos, kein Key)
- Aktuelles Wetter + 7-Tage Vorhersage
- Historische Daten mit Chart.js Diagrammen
- Multi-Standort Wetter
- Stuendliches Auto-Logging in weather_history
- 90 Tage Retention mit Pruning

### Wachstums-Analytics
- Key-Metriken: Alter, Task-Rate, Wasser, Ernte, Fotos, Logs
- Bewaesserungs-Rhythmus mit Ueber-/Unterbewaesserungs-Erkennung
- 90-Tage Aktivitaets-Heatmap
- Log-Verteilung
- Pflege-Insights ueber alle Pflanzen

### Smarte Extras
- Haustier-Check (34 Pflanzen mit Toxizitaetsdaten)
- Vermehrungsguide pro Pflanze
- Ernteprognose
- Lichtmesser via Kamera (getUserMedia)
- Nachhaltigkeits-Tracker (CO2, Wassereffizienz)
- Hinweistoene (4 Sounds via Web Audio API)

### CSV Import/Export
- Export: Pflanzen, Tasks, Logs als CSV (UTF-8 BOM fuer Excel)
- Import: Intelligente Spaltenerkennung (DE + EN)
- Tauri Save/Open File-Dialoge

### Multi-User Profile
- Lokale Profile (Owner, Admin, Gaertner, Betrachter)
- Profil-Wechsel in Toolbar
- 8 Avatar-Emojis
- Pflanzen einem Profil zuweisen

### IoT (Experimentell)
- BLE-Scanning via btleplug
- GGS Telemetrie-Parser (shared/src/ggs.rs, 5 Tests)
- Tuya Local Protocol (TCP/6668, AES-128-ECB)
- Geraete-Tab mit Device-Management
