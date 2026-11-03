# Admin-Bereich

**Adresse:** https://admin.sowling.com

Der Bereich ist nur für Admins. Die Anmeldung braucht drei Dinge: die
E-Mail-Adresse eines Admin-Kontos, dessen Passwort und den 6-stelligen Code aus
einer Authenticator-App. Jeder Code gilt nur einmal. Nach 30 Minuten endet die
Sitzung, ebenso beim Neuladen der Seite. Nach 5 Fehlversuchen ist die Anmeldung
15 Minuten gesperrt.

## Einrichten (einmalig, auf dem Server)

1. In der App ein normales Konto mit deiner Adresse anlegen (falls noch nicht
   vorhanden). Ein langes, eigenes Passwort verwenden.
2. Im Server-Terminal:
   ```bash
   cd /opt/sowling/deploy
   docker compose exec backend sowling-backend admin grant DEINE-ADRESSE
   ```
3. Den angezeigten QR-Code mit einer Authenticator-App scannen, z. B.
   Google Authenticator, Microsoft Authenticator, 1Password, Bitwarden oder
   Aegis. Der Code wird **nur dieses eine Mal** angezeigt.
4. Auf admin.sowling.com anmelden.

Weitere Befehle:

```bash
docker compose exec backend sowling-backend admin list               # alle Admins
docker compose exec backend sowling-backend admin reset-2fa ADRESSE  # neues Handy: neuen QR-Code
docker compose exec backend sowling-backend admin revoke ADRESSE     # Admin-Rechte entziehen
```

Handy verloren? `reset-2fa` auf dem Server ausführen; der alte Eintrag in der
App funktioniert danach nicht mehr.

## Was der Bereich zeigt

**Übersicht.** Kennzahlen und Diagramme:
- Konten (neu in 7 und 30 Tagen) und aktive Konten (Anmeldung oder Sitzung
  verlängert in 7 und 30 Tagen)
- zahlende Abos und aktive Gutscheine
- Umsatz der letzten 30 Tage und gesamt (brutto, aus den Rechnungen)
- Desktop-Geräte und Einmalkäufe, Pflanzen und Fotos, eingelöste Gutscheine
- Rechnungsprobleme
- Diagramme: neue Konten pro Tag (90 Tage), Tarifverteilung, Umsatz pro Monat
  (12 Monate), aktive Konten pro Tag (30 Tage)

**Konten.** Suche nach E-Mail oder Konto-ID. Pro Konto siehst du Tarif und
woher er kommt (Stripe-Abo, Gutschein mit Ablaufdatum), Registrierung, letzte
Aktivität, Anzahl Pflanzen und Rechnungen.
- **Deaktivieren:** Das Konto wird überall abgemeldet und kann sich nicht mehr
  anmelden; die Desktop-App lässt sich nicht mehr freischalten. Eine laufende
  Sitzung endet spätestens nach 60 Minuten. Die Daten bleiben erhalten.
  **Ein Stripe-Abo wird nicht gekündigt**, das geht im Stripe-Dashboard.
- **Aktivieren:** Hebt die Sperre auf.
- **2FA zurücksetzen** (nur bei Konten mit Zwei-Faktor-Anmeldung, Kennzeichen
  „2FA“): wenn jemand Handy und Wiederherstellungscodes verloren hat. Danach
  reicht wieder das Passwort. Vorher sicherstellen, dass die Anfrage wirklich
  von dieser Person kommt, z. B. per Antwort von genau dieser E-Mail-Adresse.
- **Tarif:** Hoch- oder herabstufen auf Free, Basic, Pro oder Gärtnerei, für
  1, 3, 6 oder 12 Monate, eine Anzahl Tage oder unbegrenzt. Der Tarif wird wie
  ein Gutschein gespeichert und ersetzt einen bestehenden Gutschein; „Free“
  entfernt ihn. Läuft ein Stripe-Abo, behält das Konto mindestens dessen
  Tarif. Tiefer geht es nur, wenn das Abo im Stripe-Dashboard gekündigt wird.
- **Löschen:** Wie „Konto löschen“ in der App: Pflanzen, Fotos, Lizenzen ohne
  Schlüssel und der Stripe-Kundeneintrag werden gelöscht. Zur Bestätigung musst
  du die E-Mail-Adresse eintippen. **Nicht umkehrbar.** Rechnungen bleiben
  wegen der Aufbewahrungspflicht erhalten.
- Dein eigenes Konto kannst du hier weder deaktivieren noch löschen.

**Gutscheine.** Codes für Basic, Pro oder Gärtnerei erstellen: Einzelcodes
(je einmal einlösbar, auch mehrere tausend auf einmal) oder ein lesbarer
Sammelcode für viele Konten, optional mit Höchstzahl und Einlöse-Frist. Ein
einzelner Code lässt sich direkt per E-Mail verschicken. **QR-Code** (bei
Sammelcodes in der Liste, bei bis zu 50 neuen Einzelcodes im Ergebnis): öffnet
die App mit dem Code schon im Feld „Gutschein einlösen“. Download als SVG für
den Druck oder PNG für Social Media. Darunter stehen alle
bisherigen Gutscheine mit Einlösungen; **Sperren** verhindert weitere
Einlösungen. Details: [GUTSCHEINE.md](GUTSCHEINE.md).

**Rechnungen.** Rechnungen, die noch nicht verschickt sind, mit dem letzten
Fehler (z. B. Lexware nicht erreichbar, VIES gestört). „Erneut versuchen“
startet sofort einen neuen Versuch.

**Protokoll.** Jede Admin-Anmeldung und jede Aktion (deaktiviert, aktiviert,
gelöscht, Tarif geändert, 2FA zurückgesetzt, Gutscheine erstellt oder
gesperrt, Rechnung neu angestoßen), mit Zeit und Admin.

## Sicherheit

- Der Bereich hat eine eigene Adresse und ist für Suchmaschinen gesperrt.
- Der Server prüft bei jeder Anfrage, ob das Konto noch Admin und nicht
  deaktiviert ist. Normale Anmelde-Tokens der App funktionieren hier nicht.
- Das Admin-Token liegt nur im Arbeitsspeicher des Browsers.
- Nur am eigenen, vertrauenswürdigen Rechner anmelden und danach „Abmelden“.
