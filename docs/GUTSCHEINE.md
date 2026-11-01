# Gutscheine – Abos verschenken

Ein Gutschein schaltet **Basic, Pro oder Gärtnerei** für eine feste Zeit oder
unbegrenzt frei, **inklusive Desktop-App**. Gedacht für Influencer, Beilagen in
Bestellungen (z. B. Hanfjack), Flyer und treue Kunden.

Eingelöst wird in der App unter **Abo → Gutschein einlösen**. Dafür braucht
man ein Konto (Web-App oder Desktop-App mit Konto).

## Zwei Arten

| Art | Wofür | Beispiel |
|---|---|---|
| **Einzelcodes** | ein Code = ein Konto. Für Paketbeilagen, einzelne Influencer, Kunden. Werden in beliebiger Menge erzeugt und **nur einmal angezeigt**. | `7K2M-QX9D-4HRT-B6WN` |
| **Sammelcode** | ein lesbarer Code für viele Konten, optional mit Höchstzahl und Einlöse-Frist. Für Flyer, Posts, Aktionen. | `FRUEHLING26` |

## Erzeugen

**Am einfachsten im Admin-Bereich** (https://admin.sowling.com → **Gutscheine**):
Tarif, Dauer, Art, Anzahl oder Code-Text, optional Frist, Notiz und
E-Mail-Empfänger eintragen und **Erstellen** klicken. Einzelcodes erscheinen
nur dieses eine Mal; dort lassen sie sich kopieren oder als Textdatei
herunterladen. Die Liste darunter zeigt alle bisherigen Gutscheine mit
Einlösungen; **Sperren** entspricht `voucher revoke`.

Alternativ auf dem Server im Ordner `/opt/sowling/deploy`:

```bash
# 500 Einzelcodes, Pro für 3 Monate, als Datei (eine Zeile pro Code)
docker compose exec -T backend sowling-backend voucher create --plan pro --months 3 --count 500 --note "Hanfjack Nov 2026" > hanfjack-codes.txt

# Sammelcode für Flyer: Basic 1 Monat, höchstens 1000 Konten, einlösbar bis 31.05.2027
docker compose exec backend sowling-backend voucher create --plan basic --months 1 --code FRUEHLING26 --max 1000 --until 2027-05-31 --note "Flyer Frühjahr"

# Influencer: eigener Sammelcode für die Community, Pro 2 Monate, höchstens 300
docker compose exec backend sowling-backend voucher create --plan pro --months 2 --code MAXGROWS --max 300 --note "Influencer Max"

# Persönlicher Code direkt per E-Mail (Gärtnerei, unbegrenzt)
docker compose exec backend sowling-backend voucher create --plan nursery --unlimited --email person@example.com --note "Influencer Max persönlich"
```

Optionen:

- `--plan basic | pro | nursery` (nursery = Gärtnerei)
- Laufzeit: genau eine von `--months N`, `--days N`, `--unlimited`
  (1 Monat = 30 Tage, 3 = 91, 6 = 183, 12 = 365)
- `--count N` Anzahl Einzelcodes (Standard 1, höchstens 10 000)
- `--code TEXT` Sammelcode (4–32 Buchstaben/Ziffern, Bindestriche erlaubt, keine Umlaute), `--max N` Höchstzahl Konten
- `--until JJJJ-MM-TT` letzter Tag zum Einlösen (die Laufzeit beginnt beim Einlösen)
- `--note TEXT` eigene Notiz, z. B. Aktion oder Partner
- `--email ADRESSE` schickt einen einzelnen Code per Mail (nur mit eingerichtetem SMTP)

**Einzelcodes sofort sichern.** Gespeichert wird nur ein Hash; verlorene Codes
lassen sich nicht wieder anzeigen. Die Datei `hanfjack-codes.txt` enthält nur
die Codes, eine pro Zeile, und lässt sich direkt in Excel oder ein
Druckprogramm übernehmen. Danach vom Server holen und dort löschen.

Groß-/Kleinschreibung, Leerzeichen und Bindestriche spielen beim Eintippen keine
Rolle; O/0 und I/L/1 werden nicht verwechselt.

## Übersicht und Widerruf

```bash
docker compose exec backend sowling-backend voucher list          # --all zeigt auch widerrufene
docker compose exec backend sowling-backend voucher revoke <batch-id>      # ganze Aktion
docker compose exec backend sowling-backend voucher revoke FRUEHLING26     # einzelner Code
```

`list` zeigt pro Aktion: Code bzw. Anzahl Einzelcodes, Tarif, Laufzeit,
eingelöst/gesamt, Einlöse-Frist und Notiz. Widerrufene Codes lassen sich nicht
mehr einlösen; wer schon eingelöst hat, behält den Gutschein bis zum Ende.

## Regeln beim Einlösen

- Jedes Konto kann jeden Code nur einmal einlösen.
- Gleicher Tarif: Die Zeit wird **addiert** (zwei 3-Monats-Codes = 6 Monate).
- Höherer Tarif: ersetzt den bisherigen Gutschein.
- Niedrigerer Tarif als ein laufender Gutschein: wird abgelehnt.
- Ein unbegrenzter Gutschein wird nie durch einen befristeten ersetzt.
- Gutschein und Stripe-Abo bestehen nebeneinander; es gilt jeweils der höhere
  Tarif. Kündigt jemand das Abo, läuft der Gutschein weiter, und umgekehrt.
- Ist der Gutschein abgelaufen (stündliche Prüfung), fällt das Konto auf sein
  Stripe-Abo bzw. auf Free zurück. Fotos: 90-Tage-Frist wie beim Abo-Ende.
