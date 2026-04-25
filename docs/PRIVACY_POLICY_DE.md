# Datenschutzerklärung – Plants Calendar

**Stand: 2026-04-22**

Diese Datenschutzerklärung informiert Sie gemäß Art. 12–14 DSGVO über die
Verarbeitung personenbezogener Daten im Rahmen der Nutzung der Anwendung
**Plants Calendar** (Desktop- und Backend-Dienste).

## 1. Verantwortlicher

BlockSocial UG (haftungsbeschränkt)
[Adresse wird ergänzt]
E-Mail: info@blocksocial.eu

Ein Datenschutzbeauftragter ist nach Art. 37 DSGVO nicht verpflichtend
bestellt.

## 2. Zwecke und Rechtsgrundlagen der Verarbeitung

| Tätigkeit | Zweck | Rechtsgrundlage |
|-----------|-------|------------------|
| Kontoanlage, Anmeldung, Passwortverwaltung | Bereitstellung der Nutzerkonten | Art. 6 Abs. 1 lit. b DSGVO (Vertrag) |
| Cloud-Synchronisation der Pflanzen-, Aufgaben- und Log-Daten | Synchronisation zwischen Geräten | Art. 6 Abs. 1 lit. b DSGVO (Vertrag) |
| Abruf lokaler Wetterdaten (Open-Meteo) anhand Koordinaten | Anzeige Wetter- und Klimakontext | Art. 6 Abs. 1 lit. f DSGVO (berechtigtes Interesse – funktionale Anzeige) |
| Kopplung BLE-/Tuya-Messgeräte | Anzeige von Sensorwerten | Art. 6 Abs. 1 lit. a DSGVO (Einwilligung) |
| Abonnementverwaltung / Zahlungsabwicklung | Vertragserfüllung, Abrechnung | Art. 6 Abs. 1 lit. b DSGVO (Vertrag), Art. 6 Abs. 1 lit. c DSGVO (Steuer-/Handelsrecht) |
| Server-Logs (ohne Klartext-E-Mails) | IT-Sicherheit, Fehlerdiagnose | Art. 6 Abs. 1 lit. f DSGVO (berechtigtes Interesse) |

## 3. Kategorien verarbeiteter Daten

- **Kontodaten**: E-Mail-Adresse, Passwort-Hash, Sprach-/Zeitzonen-Präferenz,
  Account-Status, Erstellungsdatum.
- **Pflanzen-, Aufgaben- und Log-Daten**: Pflanzennamen, Fotos (optional),
  Phasen, Termine, Notizen, Ernteinformationen.
- **Standortdaten**: Koordinaten selbst angelegter Standorte (manuell erfasst
  oder aus Gerätefreigaben übernommen).
- **Geräteschlüssel**: Lokale Identifier und Schlüssel von BLE-/Tuya-Geräten
  (nur auf dem Endgerät gespeichert).
- **Abrechnungsdaten**: Stripe-Kunden-ID, Abonnementstatus, Zahlungsverlauf
  (durch Stripe verarbeitet).

## 4. Empfänger

- **Stripe Payments Europe Ltd** (Irland) – Zahlungsabwicklung,
  Abonnementverwaltung. Gemeinsame Verantwortung durch Stripe DPA.
- **Stripe Inc.** (USA) – konzerninterne Datenweitergabe. Stripe ist unter
  dem **EU-US Data Privacy Framework** zertifiziert (Angemessenheits-
  beschluss der EU-Kommission vom 10.07.2023).
- **Open-Meteo** (Bremen, Deutschland) – Abruf aggregierter Wetterdaten.
  Es erfolgt keine Weitergabe personenbezogener Daten; nur Koordinaten
  werden übermittelt.
- **Backend-Hoster**: [Hoster wird benannt] – Auftragsverarbeitung nach
  Art. 28 DSGVO; Server stehen in der EU.

## 5. Internationale Datenübermittlung

Datenübermittlungen an Stripe Inc. (USA) stützen sich auf den
**EU-US Data Privacy Framework**. Darüber hinaus erfolgen keine
Übermittlungen in Drittländer.

## 6. Speicherdauer

- **Kontodaten**: bis zur Löschung des Accounts durch den Nutzer.
- **Wetter-Historie (lokal)**: **365 Tage** (Voreinstellung), darüber
  hinaus Löschung durch automatische Bereinigung.
- **Synchronisations-Tombstones**: 30 Tage zur Konfliktlösung.
- **Abrechnungsdaten**: gemäß handels- und steuerrechtlichen
  Aufbewahrungspflichten (i. d. R. 10 Jahre) bei Stripe.
- **Server-Logs**: max. 30 Tage, pseudonymisiert.

## 7. Rechte der betroffenen Personen

Sie haben jederzeit das Recht:

- **auf Auskunft** (Art. 15 DSGVO) – über die App als JSON-Export abrufbar;
- **auf Berichtigung** (Art. 16 DSGVO) – direkt in der App bearbeitbar;
- **auf Löschung** (Art. 17 DSGVO) – "Konto löschen" entfernt alle
  Serverdaten sowie den Stripe-Kundeneintrag;
- **auf Einschränkung der Verarbeitung** (Art. 18 DSGVO);
- **auf Datenübertragbarkeit** (Art. 20 DSGVO) – Export in JSON/CSV;
- **auf Widerruf erteilter Einwilligungen** (Art. 7 Abs. 3 DSGVO) –
  Widerruf wirkt für die Zukunft;
- **auf Widerspruch** (Art. 21 DSGVO);
- **auf Beschwerde** bei einer Aufsichtsbehörde (Landesdaten-
  schutzbehörde, Art. 77 DSGVO).

## 8. Kontakt für DSGVO-Anfragen

E-Mail: info@blocksocial.eu

## 9. Cookies und Tracking

Die Anwendung setzt **keine Cookies** und verwendet **keine Tracking- oder
Analyse-SDKs**. Lokal werden ausschließlich App-Einstellungen im
`localStorage` bzw. in einer lokalen SQLite-Datenbank gespeichert. Diese
Daten verbleiben auf dem Endgerät.

## 10. Datensicherheit

Alle Verbindungen zum Backend werden über TLS (HTTPS) verschlüsselt.
Passwörter werden ausschließlich als Hash (Argon2/bcrypt) gespeichert.
Zugriff auf Infrastruktur ist auf befugtes Personal beschränkt.

## 11. Änderungen dieser Erklärung

Wir passen diese Datenschutzerklärung an, wenn sich rechtliche oder
technische Rahmenbedingungen ändern. Die jeweils aktuelle Fassung finden
Sie in der App sowie unter docs/PRIVACY_POLICY_DE.md im Repository.
