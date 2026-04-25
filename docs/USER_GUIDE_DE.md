# Benutzerhandbuch — BlockSocial Plants Calendar

## 1. Erste Schritte

### App starten
```bash
cd desktop-ui/src-tauri
cargo tauri dev
```
Beim ersten Start erscheint das **Willkommen-Overlay** mit einem Formular fuer die erste Pflanze.

### Erste Pflanze anlegen
1. Name eingeben (z.B. "Tomate #1")
2. Farbe und Phase waehlen
3. "Pflanze anlegen" klicken
4. Der Kalender erscheint mit deiner Pflanze

### Navigation
Die App hat 9 Tabs in der oberen Leiste:
- **Kalender** — Startseite, Monats-/Wochen-/Tagesansicht
- **Pflanzen** — Alle Pflanzen verwalten
- **Standorte** — Orte wie Growzelt, Gewaechshaus etc.
- **Tipps** — Pflanzkalender, Vermehrung, Nachhaltigkeit
- **Wetter** — Wetterstation mit Vorhersage und Charts
- **Geraete** — IoT-Sensoren (experimentell)
- **Abo** — Tarifmodell und Billing
- **Konto** — Login, Einstellungen, Themes, Profile

---

## 2. Kalender

### Monatsansicht
- Jeder Tag zeigt farbige Punkte (eine pro Pflanze mit Aktivitaet)
- Badges: offene Tasks, ueberfaellige, Logs, Ernte, Bewaesserung
- Klick auf einen Tag oeffnet das **Tag-Detail Modal**

### Tag-Detail Modal
- Zeigt alle Tasks und Logs fuer diesen Tag
- Buttons: + Task, + Log, Giessen, Ernten
- "Done" markiert einen Task als erledigt
- Bei Giess-Tasks wird automatisch ein Bewaesserungslog erstellt
- Schliesst mit X, Escape-Taste, oder Klick neben das Fenster

### Kalender-Modi
Rechts in der Toolbar gibt es ein Dropdown mit 8 Kalendermodi:
- **Standard** — Normaler Gregorianischer Kalender
- **Mondkalender** — Mondphasen mit Gartentipps pro Phase
- **Chinesisch** — Chinesisches Lunardatum + Tierkreisjahr
- **Persisch** — Solar Hijri / Jalali Datum
- **Julianisch** — Julianisches Datum mit Differenz
- **Islamisch** — Hijri Datum
- **Juedisch** — Hebraeisches Datum
- **Bauernkalender** — Lostage, Bauernregeln, phaenoLogische Jahreszeiten

### Wiederkehrende Tasks
- Beim Erstellen: "Wiederholung (Std.)" eingeben, z.B. 48 fuer alle 2 Tage
- "Wiederholen bis" setzt das Enddatum (Standard: 4 Wochen)
- Alle zukuenftigen Termine erscheinen im Kalender (gestrichelt = geplant)
- "Done" verschiebt den Task automatisch zum naechsten Termin

---

## 3. Pflanzen verwalten

### Pflanze aus dem Katalog anlegen
1. FAB (+) druecken > "Pflanze anlegen"
2. Im Dropdown "Pflanze aus Katalog waehlen" eine Sorte auswaehlen
3. Name, Farbe, Phase werden automatisch ausgefuellt
4. Optional: Standort, Topfgroesse, Substrat waehlen
5. "Pflanze anlegen" klicken

### Pflanze bearbeiten
- Im Pflanzen-Tab auf "Edit" klicken
- Name, Farbe, Phase, Woche, Standort, Topfgroesse, Substrat aendern
- "Speichern" druecken

### Pflanzenstatus
- **Aktiv** — Zaehlt ins Planlimit
- **Beendet** — Run beendet, zaehlt nicht mehr
- **Archiviert** — Aufbewahrt, kann reaktiviert werden
- **Geloescht** — Endgueltig entfernt

### Mehrjaehrige Pflanzen
- Beim Anlegen "Mehrjaehrig" als Lebenszyklus waehlen
- Pflanzenalter wird automatisch berechnet
- "Neue Saison" startet ein neues Wachstumsjahr
- Saisonverlauf zeigt Ernte, Schnitt und Notizen pro Jahr

---

## 4. Tasks & Erinnerungen

### Task erstellen
1. FAB (+) > "Task erstellen"
2. Pflanze, Titel, Kategorie (Wasser/Duenger/Check/Training/Notiz) waehlen
3. Faelligkeitsdatum setzen
4. Optional: Wiederholung + Enddatum
5. "Task anlegen" klicken

### Task erledigen
- Im Kalender: Tag anklicken > "Done" im Modal
- Im Pflanzen-Tab: "Done" in der Task-Liste
- Bei Giess-Tasks: Automatischer Bewaesserungslog

---

## 5. Logs & Messwerte

### Log-Typen
- **Notiz** — Freitext
- **Messung** — pH, EC, Temperatur, Luftfeuchtigkeit
- **Ernte** — kg, Stueck, Qualitaet
- **Bewaesserung** — Liter, Methode

### Log erstellen
1. FAB (+) > "Log erstellen"
2. Pflanze und Log-Typ waehlen
3. Werte eintragen
4. Optional: Foto anhaengen
5. "Log erstellen" klicken

---

## 6. Standorte

Im Tab "Standorte":
- Neuen Standort erstellen (Name, Typ, Beschreibung)
- Typen: Growzelt, Gewaechshaus, Outdoor, Indoor, Balkon, Eigener
- Pflanzen beim Bearbeiten einem Standort zuweisen
- Koordinaten setzen (fuer standortspezifisches Wetter)

---

## 7. Fotos

Im Pflanzen-Tab (Pflanze auswaehlen):
- "Foto" Button oeffnet den Dateiauswahl-Dialog
- Fotos werden als chronologische Timeline angezeigt
- Tag X seit Pflanzenbeginn wird berechnet
- Fotos bleiben komplett lokal auf deinem Rechner

---

## 8. Wetter

Im Tab "Wetter":
- Aktuelles Wetter (Temperatur, Luftfeuchtigkeit, Wind, Niederschlag)
- 24h Stundenvorhersage
- 7-Tage Tagesvorhersage
- Historische Charts (Temperatur, Luftfeuchtigkeit, Wind)
- Zwischen Standorten wechseln (falls Koordinaten gesetzt)

### Einrichten
1. Unter Konto > Klima-Einstellungen
2. Stadt suchen und auswaehlen
3. Wetterdaten werden automatisch stuendlich geloggt

---

## 9. Tipps

Im Tab "Tipps":
- **Pflanzkalender** — Was diesen Monat saeen/ernten
- **Jahresuebersicht** — 12 Monate mit allen Aktivitaeten
- **Sicherheit** — Welche deiner Pflanzen sind giftig fuer Haustiere
- **Vermehrungsguide** — Wie jede Pflanze vermehrt werden kann
- **Ernteprognose** — Wann welche Pflanze erntereif ist
- **Nachhaltigkeit** — CO2 gespart, Wassereffizienz, Tipps
- **Lichtmesser** — Kamera nutzen um Helligkeit zu messen

---

## 10. Bewaesserung & Urlaubsmodus

### Intelligenter Kalkulator
Jede aktive Pflanze zeigt ein Ampel-Icon:
- Gruen = Alles OK
- Gelb = Bald giessen (< 6h)
- Rot = Ueberfaellig

Der Kalkulator beruecksichtigt: Wetter, Topfgroesse, Substrat, Pflanzenart, Saison.

### Urlaubsmodus
1. Unter Konto > Urlaubsmodus aktivieren
2. Rueckkehrdatum setzen
3. Alle Bewaesserungsintervalle werden um 50% verlaengert
4. Banner im Kalender zeigt aktiven Urlaubsmodus
5. Nach Rueckkehr: Modus deaktivieren

---

## 11. Mischkultur (Companion Planting)

Automatisch fuer Katalog-Pflanzen:
- **Gruene Badges** = Gute Nachbarn (foerdern sich gegenseitig)
- **Rote Badges** = Schlechte Nachbarn (hemmen sich)
- Wird auf Pflanzenkarten und im Detail angezeigt
- Nur fuer aktive Pflanzen mit Katalog-ID

---

## 12. Sicherheit (Haustiere & Allergien)

Automatisch fuer Katalog-Pflanzen:
- 🐱 Giftig fuer Katzen
- 🐶 Giftig fuer Hunde
- ⚠️ Allergen
- ⚠️ Giftig fuer Menschen (roh)
- Tooltip mit Details (z.B. "Blaetter giftig durch Solanin")

---

## 13. CSV Import/Export

Unter Konto > Import/Export:
- **Export**: Pflanzen, Tasks oder Logs als CSV-Datei speichern
- **Import**: CSV von anderen Apps importieren (erkennt DE + EN Spalten)
- Kompatibel mit Excel, Google Sheets, GrowVeg, Planter

---

## 14. Themes & Sounds

Unter Konto > Mood / Design:
- 8 Farbthemen zur Auswahl
- Hinweistoene ein/aus + Test-Button
- Einstellungen werden gespeichert

---

## 15. Team/Profile

Unter Konto > Team & Profile:
- Profile erstellen: Name, Rolle, Avatar, Farbe
- Rollen: Eigentuemer, Administrator, Gaertner, Betrachter
- Schnelles Wechseln ueber Toolbar-Klick
- Pflanzen koennen Profilen zugewiesen werden

---

## 16. Einstellungen

Unter Konto:
- Anmelden/Registrieren (braucht laufendes Backend)
- Guest-Modus fuer lokale Nutzung
- Profil bearbeiten (Sprache, Zeitzone)
- Passwort aendern
- Account loeschen (GDPR)
- Sync mit Backend
