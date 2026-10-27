# Server einrichten (Hostinger VPS)

Anleitung für den Livegang von Web-App und Backend auf einem eigenen Server.
Alles läuft in Docker: Caddy (HTTPS + Web-App), Backend, PostgreSQL und ein
Backup-Dienst. Die Dateien liegen in [`deploy/`](../deploy).

```
Internet ──443──▶ Caddy ──┬── app.sowling.com  → Web-App (statische Dateien)
                          └── api.sowling.com  → Backend :8080 ──▶ PostgreSQL
                                                      Backup (nachts, verschlüsselt)
```

## 0. Voraussetzungen

- **VPS** mit Ubuntu 24.04, mindestens 2 GB RAM (Hostinger „KVM 2“ reicht für den
  Start; das erste Bauen des Backends braucht etwas Zeit und Speicher).
- **Domain** mit zwei Subdomains, z. B. `app.` und `api.` (Namen frei wählbar).
- SSH-Zugang zum Server.

## 1. DNS

Im Hostinger-DNS-Editor zwei **A-Einträge** (und, falls der VPS eine hat, AAAA
für IPv6) auf die Server-IP setzen:

| Name | Typ | Ziel |
|---|---|---|
| `app` | A | IP des VPS |
| `api` | A | IP des VPS |

Bis die Einträge weltweit sichtbar sind, kann es einige Minuten dauern.

## 2. Server vorbereiten

```bash
ssh root@DEINE-SERVER-IP
apt update && apt upgrade -y
# Docker (offizielles Skript)
curl -fsSL https://get.docker.com | sh
# Firewall: nur SSH und Web
ufw allow OpenSSH && ufw allow 80/tcp && ufw allow 443/tcp && ufw allow 443/udp && ufw --force enable
# Werkzeuge für Wiederherstellung
apt install -y age git
```

## 3. Code holen

```bash
git clone <REPO-URL> /opt/plants
cd /opt/plants/deploy
cp .env.example .env
chmod 600 .env
```

## 4. Backup-Schlüssel erzeugen (vor allem anderen)

Backups werden mit einem Schlüsselpaar verschlüsselt. Auf dem Server liegt nur
der **öffentliche** Teil; den **privaten** Teil brauchst du nur zum
Wiederherstellen. Bewahre ihn sicher auf, z. B. im Passwort-Manager und
zusätzlich ausgedruckt. **Ohne ihn sind die Backups wertlos.**

```bash
docker run --rm alpine:3.22 sh -c "apk add -q age && age-keygen"
```

Die Ausgabe enthält `# public key: age1…` und darunter `AGE-SECRET-KEY-1…`.
- `age1…` → in `.env` als `BACKUP_AGE_RECIPIENT`
- `AGE-SECRET-KEY-1…` → **nur** in deinen Passwort-Manager, nicht auf dem Server speichern.

## 5. `.env` ausfüllen

```bash
nano .env
```

1. `APP_DOMAIN`, `API_DOMAIN`, `ACME_EMAIL`
2. Zufallswerte erzeugen und eintragen:
   ```bash
   openssl rand -hex 32   # → POSTGRES_PASSWORD
   openssl rand -hex 48   # → JWT_SECRET
   ```
3. Lizenz- und Push-Schlüssel (einmalig, **danach nie mehr ändern**):
   ```bash
   docker compose build backend
   docker compose run --rm --no-deps backend gen-license-keys
   docker compose run --rm --no-deps backend gen-vapid-keys
   ```
   - `LICENSE_SIGNING_KEY` und `VAPID_PRIVATE_KEY` in `.env` eintragen.
   - `PLANTS_LICENSE_PUBLIC_KEY` aufbewahren: Den brauchst du beim Bauen der
     Desktop-App (siehe [LIZENZEN.md](LIZENZEN.md)).
   - Beide geheimen Schlüssel zusätzlich im Passwort-Manager sichern.
4. Stripe-Werte (Live-Schlüssel, Webhook-Secret, Preis-IDs), siehe Abschnitt 8.
5. `DESKTOP_DOWNLOAD_URL`, sobald der Installer online ist.

## 6. Starten

```bash
docker compose up -d --build
docker compose ps          # alle Dienste "healthy" bzw. "running"
docker compose logs -f backend
```

Beim ersten Start legt das Backend die Datenbanktabellen an, und Caddy holt die
HTTPS-Zertifikate. Danach:

- `https://app.sowling.com` → Web-App
- `https://api.sowling.com/health` → `{"message":"ok"}`

## 7. Backups prüfen

Backups laufen täglich um `BACKUP_HOUR` Uhr nach `deploy/backups/` und werden
nach 30 Tagen gelöscht. Sofort einmal testen:

```bash
docker compose exec backup backup.sh now
ls -lh backups/
```

**Wiederherstellung üben** (mindestens einmal vor dem Livegang, danach
vierteljährlich). Dafür den privaten Schlüssel vorübergehend in eine Datei legen
und hinterher löschen:

```bash
nano /root/backup-key.txt          # AGE-SECRET-KEY-1… einfügen
./restore.sh /root/backup-key.txt 2026-10-17_0300
shred -u /root/backup-key.txt
```

**Kopie außerhalb des Servers:** Backups auf demselben Server schützen nicht
vor dessen Ausfall. Lade `deploy/backups/` regelmäßig woanders hin, z. B. per
`rclone` in einen Cloud-Speicher in der EU oder per `scp` auf deinen Rechner.
Die Dateien sind verschlüsselt und dürfen dort liegen. Zusätzlich die
Snapshot-Funktion des Hostinger-VPS aktivieren.

## 8. Stripe

1. Im Stripe-Dashboard (Live-Modus) Produkte anlegen: Basic, Pro, CSC als
   **Abo**, „Desktop-App“ als **einmalige Zahlung**. Die Preis-IDs (`price_…`)
   in `.env` eintragen.
2. Webhook anlegen: URL `https://api.sowling.com/subscription/webhook`,
   Ereignisse `checkout.session.completed` und
   `customer.subscription.deleted`. Das Signing-Secret (`whsec_…`) als
   `STRIPE_WEBHOOK_SECRET`.
3. Nach Änderungen an `.env`: `docker compose up -d backend`

## 9. Eigene Lizenzen ausstellen

```bash
docker compose exec backend plants-backend license create --note "Betatester" --devices 2
docker compose exec backend plants-backend license list
```

Mehr dazu in [LIZENZEN.md](LIZENZEN.md).

## 10. Updates einspielen

```bash
cd /opt/plants && git pull
cd deploy && docker compose up -d --build
```

Datenbank-Änderungen laufen beim Start automatisch. Vor größeren Updates
`docker compose exec backup backup.sh now`.

## 11. Vor dem Livegang

- [ ] Impressum und Datenschutzerklärung vervollständigt (Anschrift, Hoster)
- [ ] Auftragsverarbeitungsvertrag mit dem Hoster (Hostinger) abgeschlossen
- [ ] Stripe-Zusatz zur Datenverarbeitung (DPA) akzeptiert
- [ ] Wiederherstellung eines Backups einmal erfolgreich getestet
- [ ] Backup-Schlüssel, `LICENSE_SIGNING_KEY`, `VAPID_PRIVATE_KEY` im Passwort-Manager
- [ ] Testkauf (Abo und Einmalkauf) im Stripe-Testmodus durchgespielt
- [ ] Push-Erinnerung auf einem echten Handy getestet (HTTPS nötig)
