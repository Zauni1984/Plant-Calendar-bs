# Incident Response Playbook

*Internes Dokument. Art. 33/34 DSGVO.*

## 1. Erkennungsquellen

- **Log-basierte 5xx-Alerts** auf Backend (`/health` + HTTP status counter).
- **Stripe Webhook-Fehler** (Signaturfehler, unbekannte Events).
- **Sentry / Tracing** (sofern aktiviert) – panic-Fehler, sqlx-Fehler.
- **Nutzer-Meldung** per Mail an info@blocksocial.eu.
- **Externe Hinweise** (Security-Researcher, CERT).

## 2. Rollen

| Rolle | Verantwortung |
|-------|----------------|
| **Incident Commander (IC)** | Gesamtleitung, Entscheidet Ausruf Incident / Eskalation, Dokumentation |
| **Technical Lead** | Technische Analyse, Eindämmung, Recovery |
| **Communications Lead** | Interne/externe Kommunikation, Meldung an Aufsicht, Nutzerinformation |

Mindestens IC + Technical Lead müssen erreichbar sein.

## 3. Ablauf

1. **Erkennen** – Alert-Quelle dokumentieren, Uhrzeit (UTC) festhalten.
2. **Bewerten** – Umfang, Art der Daten, Anzahl Betroffener, Risiko.
3. **Eindämmen** – Key-Rotation (JWT_SECRET, STRIPE_WEBHOOK_SECRET),
   Netzwerk-Isolation, Service-Stop falls nötig.
4. **Beseitigen** – Patch, Redeploy, Revoke betroffene Tokens.
5. **Wiederherstellen** – Services hochfahren, Monitoring verifizieren.
6. **Nachbereiten** – Post-Mortem, ROPA-Update, Log im Incident-Register.

## 4. Meldung an die Aufsichtsbehörde (Art. 33 DSGVO – 72 h)

> **Betreff:** Meldung einer Verletzung des Schutzes personenbezogener Daten gemäß Art. 33 DSGVO
>
> Sehr geehrte Damen und Herren,
>
> am **[Datum/Uhrzeit UTC]** wurde bei BlockSocial UG (haftungsbeschränkt),
> info@blocksocial.eu, eine Verletzung des Schutzes personenbezogener Daten
> festgestellt.
>
> **Art der Verletzung:** [z. B. unbefugter Zugriff, Datenabfluss, Ausfall]
> **Betroffene Datenkategorien:** [E-Mail, Passwort-Hashes, …]
> **Betroffene Personen:** ca. [Anzahl]
> **Wahrscheinliche Folgen:** [Risiko für Rechte der Betroffenen]
> **Getroffene Maßnahmen:** [Eindämmung, Benachrichtigung, Patch]
> **Verantwortlicher:** BlockSocial UG (haftungsbeschränkt), info@blocksocial.eu
> **Kontaktperson:** [Name, E-Mail, Telefon]
>
> Weitere Informationen werden unverzüglich nachgereicht.
>
> Mit freundlichen Grüßen
> [Name, Funktion]

## 5. Benachrichtigung Betroffener (Art. 34 DSGVO)

Bei hohem Risiko:

> **Betreff:** Wichtige Information zum Schutz Ihrer Daten bei Sowling
>
> Sehr geehrte/r Nutzer/in,
>
> wir informieren Sie hiermit in klarer und einfacher Sprache über einen
> Vorfall, der auch Ihre Daten betreffen könnte.
>
> **Was ist passiert?** [Kurze Beschreibung]
> **Wann?** [Datum]
> **Welche Daten sind betroffen?** [Kategorien]
> **Was bedeutet das für Sie?** [Risiko, Handlungsempfehlung z. B. Passwort ändern]
> **Was haben wir unternommen?** [Maßnahmen]
> **Wen können Sie kontaktieren?** info@blocksocial.eu
>
> Wir entschuldigen uns für die Unannehmlichkeiten.
>
> BlockSocial UG (haftungsbeschränkt)

## 6. Dokumentationsvorlage (Incident Log)

```
Incident-ID:         INC-YYYY-NNN
Erkannt:             YYYY-MM-DDThh:mm:ssZ  (Quelle: ...)
IC:                  ...
Tech Lead:           ...
Comms:               ...

Zeitleiste:
  hh:mm  Detection       ...
  hh:mm  Triage          ...
  hh:mm  Containment     ...
  hh:mm  Eradication     ...
  hh:mm  Recovery        ...

Betroffene Systeme:   ...
Betroffene Datenkategorien: ...
Ca. Anzahl Betroffener: ...
Meldung an Aufsicht:  ja/nein — wenn ja: [Behörde, Datum, Az.]
Benachrichtigung Nutzer: ja/nein — wenn ja: [Datum, Kanal]
Post-Mortem-Link:     ...
Follow-Up-Tasks:      ...
```

Incidents sind unabhängig von einer Meldepflicht mindestens **3 Jahre**
zu archivieren.
