# Auftrag: Sowling auf einem Hostinger-VPS in Betrieb nehmen

Du (Claude in Chrome) richtest die Web-App **Sowling** auf einem Hostinger-VPS ein.
Arbeite die Schritte **der Reihe nach** ab. Nach jedem Schritt kurz auf Deutsch
melden, was erledigt ist. Bei Fehlern oder Unklarheiten **anhalten und fragen**,
nicht improvisieren.

## Rahmen

- Domain: **sowling.com** (bei Hostinger). Web-App unter **app.sowling.com**,
  Server-API unter **api.sowling.com**.
- Code: privates GitHub-Repository **Zauni1984/Plant-Calendar-bs**, Branch `main`.
- Alles Technische auf dem Server erledigt das Skript `deploy/bootstrap.sh` im
  Repository (Docker, Firewall, Geheimnisse, Start).

## Regeln (wichtig)

1. **Nichts kaufen ohne ausdrückliche Bestätigung.** Vor dem Bestellen Tarif,
   Laufzeit und Gesamtpreis nennen und auf ein klares „Ja“ warten.
2. **Keine Passwörter oder API-Schlüssel eintippen.** Anmeldungen (Hostinger,
   GitHub), das Root-Passwort des VPS und Stripe-Schlüssel gibt der Nutzer selbst ein.
3. **Geheimnisse nicht anzeigen oder wiedergeben.** Die Dateien
   `/root/sowling-backup-key.txt` und `deploy/.env` nicht öffnen und nicht per
   `cat` ausgeben. Das macht der Nutzer selbst (Schritt 8).
4. **Keine bestehenden DNS-Einträge löschen** außer den in Schritt 3 genannten
   für `app` und `api`. Mail-Einträge (MX, TXT/SPF, DKIM) nicht anfassen.
5. Lange Befehle unverändert aus diesem Dokument kopieren.

## Schritt 1 – VPS bestellen (Bestätigung nötig)

hPanel → **VPS** → neuen VPS bestellen:

- **Tarif: KVM 2** (2 vCPU, 8 GB RAM, 100 GB NVMe). KVM 1 würde auch laufen,
  ist aber beim Kompilieren langsam und hat wenig Platz für Fotos und Backups.
- **Standort: Deutschland**; falls nicht verfügbar, ein anderes EU-Land
  (Niederlande, Frankreich, Litauen). Kein Standort außerhalb der EU (DSGVO).
- **Betriebssystem: Ubuntu 24.04** als **reines Betriebssystem**, ohne
  Control Panel und ohne vorinstallierte App.
- Root-Passwort: setzt der Nutzer selbst.
- Falls angeboten: automatische Backups/Snapshots aktivieren.

Vor dem Abschluss: Tarif, Laufzeit, Gesamtpreis nennen → auf Bestätigung warten.
Danach die **IPv4-Adresse** (und ggf. IPv6) des VPS notieren und melden.

## Schritt 2 – Auftragsverarbeitung

Im hPanel prüfen, ob Hostinger einen **Auftragsverarbeitungsvertrag (DPA)**
anbietet, und dem Nutzer den Fundort nennen. Nicht selbst akzeptieren.

## Schritt 3 – DNS für sowling.com

hPanel → **Domains** → sowling.com → **DNS / Nameserver** → DNS-Einträge:

| Typ | Name | Ziel | TTL |
|---|---|---|---|
| A | `app` | IPv4 des VPS | 300 |
| A | `api` | IPv4 des VPS | 300 |
| AAAA | `app` | IPv6 des VPS (nur falls vorhanden) | 300 |
| AAAA | `api` | IPv6 des VPS (nur falls vorhanden) | 300 |

Gibt es für `app` oder `api` schon Einträge (A, AAAA oder CNAME), diese
ersetzen. Nichts anderes ändern. Danach prüfen, z. B. mit
`https://dns.google/resolve?name=api.sowling.com&type=A`. Die IP sollte nach
wenigen Minuten erscheinen.

## Schritt 4 – Server-Terminal öffnen

hPanel → VPS → **Browser-Terminal** (oder „Terminal“). Anmelden als `root`; das
Passwort gibt der Nutzer ein. Alle weiteren Befehle laufen dort.

## Schritt 5 – Zugriff auf das GitHub-Repository

Im Server-Terminal einen schreibgeschützten Zugangsschlüssel erzeugen:

```bash
apt-get update -q && apt-get install -y -q git
ssh-keygen -t ed25519 -N "" -C "sowling-vps" -f /root/.ssh/sowling_deploy
cat /root/.ssh/sowling_deploy.pub
```

Die ausgegebene Zeile (`ssh-ed25519 …`) ist **öffentlich** und darf kopiert werden.

Dann in GitHub: Repository **Zauni1984/Plant-Calendar-bs** → **Settings** →
**Deploy keys** → **Add deploy key**:

- Title: `sowling-vps`
- Key: die kopierte Zeile
- „Allow write access“: **nicht** anhaken

Zurück im Server-Terminal:

```bash
GIT_SSH_COMMAND="ssh -i /root/.ssh/sowling_deploy -o StrictHostKeyChecking=accept-new" \
  git clone git@github.com:Zauni1984/Plant-Calendar-bs.git /opt/sowling
git -C /opt/sowling config core.sshCommand "ssh -i /root/.ssh/sowling_deploy"
ls /opt/sowling/deploy/bootstrap.sh
```

Die letzte Zeile muss den Dateipfad ausgeben. Sonst anhalten und melden.

## Schritt 6 – Einrichtungsskript starten

```bash
cd /opt/sowling/deploy && sh bootstrap.sh
```

Das dauert beim ersten Mal **10–20 Minuten**, vor allem wegen „Building the
backend“. Nicht abbrechen. Erfolgreich ist es, wenn am Ende `=== Done` steht und
`docker compose ps` alle Dienste als `running`/`healthy` zeigt.

Bei einem Fehler: die letzten ~30 Zeilen der Ausgabe dem Nutzer zeigen (sie
enthalten keine Geheimnisse) und anhalten.

## Schritt 7 – Prüfen

Im Browser öffnen:

1. `https://api.sowling.com/health` → muss `{"message":"ok"}` zeigen.
2. `https://app.sowling.com` → die App lädt mit gültigem HTTPS-Schloss.

Der erste Aufruf kann bis zu einer Minute dauern (Zertifikat wird ausgestellt).
Klappt es nach 5 Minuten nicht: Schritt 3 (DNS) prüfen und melden.

Dann ein Backup testen:

```bash
cd /opt/sowling/deploy && docker compose exec backup backup.sh now && ls -lh backups/
```

Es müssen zwei Dateien `sowling-db-….dump.age` und `sowling-photos-….tar.age` erscheinen.

## Schritt 8 – An den Nutzer übergeben (nicht selbst erledigen)

Dem Nutzer diese Liste geben und dabei **nicht** helfen, die Inhalte auszulesen:

1. **Backup-Schlüssel sichern:** `cat /root/sowling-backup-key.txt`, den Inhalt in
   den Passwort-Manager kopieren, danach `shred -u /root/sowling-backup-key.txt`.
   Ohne diesen Schlüssel sind die Backups wertlos.
2. **Server-Schlüssel sichern:** `grep -E '^(LICENSE_SIGNING_KEY|VAPID_PRIVATE_KEY)=' /opt/sowling/deploy/.env`
   → beide Werte in den Passwort-Manager. Sie dürfen sich nie ändern.
3. **Öffentlicher Lizenzschlüssel für die Desktop-App** (nicht geheim):
   `cat /root/sowling-license-public-key.txt` an den Entwickler geben.
4. **Stripe** (wenn Zahlungen starten sollen): Live-Schlüssel, Webhook-Secret und
   Preis-IDs in `/opt/sowling/deploy/.env` eintragen, dann
   `cd /opt/sowling/deploy && docker compose up -d backend`.
   Webhook-URL: `https://api.sowling.com/subscription/webhook`, Ereignisse
   `checkout.session.completed` und `customer.subscription.deleted`.
5. **Rechtliches:** Anschrift und Hoster (Hostinger, Serverstandort) in
   Impressum und Datenschutzerklärung eintragen lassen.

## Abschlussmeldung

Kurz melden: VPS-Tarif und Standort, IP-Adresse, ob `health` und die App
erreichbar sind, ob das Test-Backup erstellt wurde, und dass Schritt 8 beim
Nutzer liegt.
