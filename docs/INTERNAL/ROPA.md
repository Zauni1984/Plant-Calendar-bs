# Verzeichnis von Verarbeitungstätigkeiten (ROPA)

*Art. 30 DSGVO – Internes Dokument. Nicht zur Weitergabe bestimmt.*

**Verantwortlicher:** BlockSocial UG (haftungsbeschränkt), info@blocksocial.eu
**Stand:** 2026-04-22

## Verarbeitungstätigkeiten

| # | Tätigkeit | Zweck | Rechtsgrundlage | Kategorien pers. Daten | Betroffene | Empfänger | Drittlandtransfer | Speicherdauer | TOMs |
|---|-----------|-------|------------------|--------------------------|-------------|------------|--------------------|----------------|------|
| 1 | Nutzerkonto / Authentifizierung | Bereitstellung Nutzerkonto, Login | Art. 6(1)(b) DSGVO | E-Mail, Passwort-Hash, Locale, Timezone | App-Nutzer | Hoster [TBD] (Auftragsverarbeiter, EU) | Nein | Bis Kontolöschung | TLS, Argon2/bcrypt, JWT-Signatur, Rate-Limit, pseudonymisierte Logs |
| 2 | Cloud-Synchronisation | Geräteübergreifende Datenhaltung | Art. 6(1)(b) DSGVO | Pflanzen-, Aufgaben-, Log-, Standortdaten | App-Nutzer | Hoster [TBD] | Nein | Bis Kontolöschung; Tombstones 30 Tage | TLS, Zugriff nur authentifiziert, Row-Level-Filter nach user_id |
| 3 | Wetterdaten-Abruf (Open-Meteo) | Anzeige Wetter/Klima | Art. 6(1)(f) DSGVO | Koordinaten | App-Nutzer | Open-Meteo (DE) | Nein | Lokal 365 Tage | TLS, keine personenbez. Daten übertragen |
| 4 | Kopplung BLE-/Tuya-Geräte | Sensoranzeige | Art. 6(1)(a) DSGVO | Geräte-IDs, lokale Schlüssel, Messwerte | App-Nutzer | Keine (lokal) | Nein | Bis Nutzer-Entkopplung | Lokal verschlüsselte SQLite, Schlüssel nicht synchronisiert |
| 5 | Zahlungsabwicklung (Stripe) | Abonnement, Rechnungsstellung | Art. 6(1)(b), 6(1)(c) DSGVO | Stripe-Kunden-ID, Abo-Status, Zahlungsverlauf | Zahlende Nutzer | Stripe Payments Europe Ltd (IE), Stripe Inc (US) | Ja – EU-US DPF (Stripe zertifiziert) | gem. § 147 AO / § 257 HGB (bis 10 J.) bei Stripe | Stripe DPA, Webhook-Signatur (HMAC-SHA256), TLS |
| 6 | Server-Logs | IT-Sicherheit, Diagnose | Art. 6(1)(f) DSGVO | IP-Adresse (ggf.), HTTP-Metadaten, pseudonymisierte E-Mail | App-Nutzer | Hoster [TBD] | Nein | Max. 30 Tage | Pseudonymisierung vor Ausgabe (`pseudonymize_email`), rotierende Logs |
| 7 | Refresh-Token-Verwaltung | Session-Erneuerung | Art. 6(1)(b) DSGVO | Hashes der Tokens, Ablaufdatum | Angemeldete Nutzer | Hoster [TBD] | Nein | 30 Tage oder bis Revocation | SHA-256 Hash, automatisches Ablaufen |

## Technisch-organisatorische Maßnahmen (Überblick)

- **Vertraulichkeit**: TLS 1.2+, Passwort-Hashes Argon2/bcrypt, JWT mit Secret aus Env-Variable, Zugriffskontrolle auf Hoster-Ebene.
- **Integrität**: HMAC-geprüfte Stripe-Webhooks, sqlx-Parameter-Binding (SQLi-Schutz), Content-Length-Limits, Rate-Limiter.
- **Verfügbarkeit**: Regelmäßige Backups mit DSGVO-konformer Aufbewahrung (siehe DEPLOYMENT.md).
- **Belastbarkeit**: Health-Checks (`/health`), strukturierte Fehlerbehandlung via `anyhow`/`thiserror`.
- **Rechenschaftspflicht**: Dieses ROPA + ausgefülltes Auftragsverarbeitungsverzeichnis der Subdienstleister.

## Review-Zyklus

Mindestens jährlich sowie bei wesentlichen Änderungen an der Verarbeitung.
