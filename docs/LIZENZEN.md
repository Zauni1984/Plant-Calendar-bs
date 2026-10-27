# Desktop-Lizenzen – Betrieb

Die Desktop-App läuft nur mit Lizenz. Freigeschaltet wird sie auf drei Wegen:

| Weg | Wie | Gültigkeit offline |
|---|---|---|
| Abo Basic, Pro oder CSC | in der App „Mit Konto anmelden“ | 14 Tage, dann Online-Erneuerung |
| Einmalkauf (Stripe) | im Web unter Konto → Desktop-App kaufen; dann den angezeigten Lizenzschlüssel eingeben **oder** in der App mit dem Konto anmelden | 60 Tage |
| Eigene Lizenz (Test, Betatester, treue Kunden) | Lizenzschlüssel eingeben **oder** mit der hinterlegten E-Mail anmelden | 60 Tage |

Online erneuert die App ihr Zertifikat einmal täglich. Widerrufene Lizenzen und beendete Abos wirken spätestens nach Ablauf des Zertifikats.
Eine Lizenz gilt standardmäßig für **3 Geräte**. „Lizenz von diesem Computer entfernen“ gibt einen Platz frei.

## Eigene Lizenzen ausstellen

Auf dem Server, mit gesetzter `DATABASE_URL`:

```bash
# Schlüssel für dich selbst / einen Betatester (wird nur jetzt angezeigt!)
plants-backend license create --note "Betatester Max" --devices 2

# An eine E-Mail-Adresse gebunden: Die Person meldet sich in der App mit
# dieser Adresse an, ein Schlüssel ist nicht nötig. Mit Schlüssel geht es zusätzlich.
plants-backend license create --email kunde@example.com --note "Treuer Kunde" --no-key

# Übersicht, Widerruf, Geräte zurücksetzen
plants-backend license list            # --all zeigt auch widerrufene
plants-backend license revoke PLNT-XXXX-XXXX-XXXX-XXXX   # oder Lizenz-ID oder E-Mail
plants-backend license reset-devices <lizenz-id>
```

Schlüssel werden nur als Hash gespeichert. Wer einen selbst ausgestellten Schlüssel verliert, bekommt einen neuen; den alten widerrufen.

**Käufer-Schlüssel** stehen dauerhaft im Web-Konto (Konto → Desktop-App). Sie werden nicht gespeichert, sondern aus `LICENSE_SIGNING_KEY` und der Lizenz-ID berechnet. Nach einer Kontolöschung funktioniert der Schlüssel weiter; die Lizenz verliert nur den Bezug zur Person. **Deshalb `LICENSE_SIGNING_KEY` nie wechseln** – sonst passen die angezeigten Käufer-Schlüssel nicht mehr (das Konto zeigt dann keinen an).

## Einmalig einrichten (Produktion)

1. Schlüsselpaar erzeugen: `plants-backend gen-license-keys`
   - `LICENSE_SIGNING_KEY` → Server-Umgebung (**geheim**, sicher aufbewahren, **nie wechseln**). Ein Wechsel bräuchte eine neue App-Version mit dem neuen öffentlichen Schlüssel, alle Geräte müssten neu freigeschaltet werden, und Käufer-Schlüssel änderten sich. Nur bei einem Leck des Schlüssels.
   - `PLANTS_LICENSE_PUBLIC_KEY` → beim Bauen der Desktop-App setzen. Release-Builds brechen ohne ihn ab.
2. Desktop-Build-Variablen: `PLANTS_LICENSE_PUBLIC_KEY`, `PLANTS_API_URL` (z. B. `https://api.sowling.com`), `PLANTS_WEB_URL` (Web-App, für „Kaufen“).
3. Stripe: Produkt „Desktop-App“ als **einmalige Zahlung** anlegen, Preis-ID als `STRIPE_PRICE_DESKTOP`. Webhook `checkout.session.completed` ist bereits eingerichtet (`STRIPE_WEBHOOK_SECRET`).
4. `DESKTOP_DOWNLOAD_URL` = Link zum Installer. Der Download selbst darf öffentlich sein; ohne Lizenz startet die App nicht.

## Entwicklung

Debug-Builds von Server und App nutzen einen öffentlich bekannten Entwicklungsschlüssel und funktionieren ohne Einrichtung.
`PLANTS_LICENSE_DEV_BYPASS=1` überspringt die Lizenzprüfung in Debug-Builds der App.
Ohne Stripe schaltet „Einmalig kaufen“ in Debug-Builds direkt frei; Release-Builds verweigern Käufe und Abos ohne Stripe-Konfiguration.
