# Datenschutzerklärung – Plants Calendar

**Stand: 16.10.2026**

<!-- Vor Veröffentlichung ausfüllen/prüfen: Anschrift (Abschnitt 1), Hoster und
     Serverstandort (Abschnitt 5), App-Name nach der Umbenennung. Sobald der
     E-Mail-Versand (Hostinger Mail) aktiv ist, Abschnitt 4.9 ergänzen. Eine
     anwaltliche Prüfung vor dem Livegang wird empfohlen. -->

Diese Datenschutzerklärung informiert Sie nach Art. 12–14 DSGVO darüber, welche
personenbezogenen Daten wir verarbeiten, wenn Sie **Plants Calendar** nutzen –
als **Web-App** im Browser (auch installiert auf Smartphone oder Tablet) oder als
**Desktop-App** für Windows.

## 1. Verantwortlicher

BlockSocial UG (haftungsbeschränkt)
[Straße, Hausnummer]
[PLZ Ort]
E-Mail: info@blocksocial.eu

Ein Datenschutzbeauftragter ist nach Art. 37 DSGVO nicht bestellt, da keine
Pflicht dazu besteht.

## 2. Überblick

| Verarbeitung | Zweck | Rechtsgrundlage |
|---|---|---|
| Konto (E-Mail, Passwort-Hash) | Anmeldung, Bereitstellung des Dienstes | Art. 6 Abs. 1 lit. b DSGVO (Vertrag) |
| Pflanzen, Aufgaben, Logs, Standorte, Saisons | Kernfunktion der App | Art. 6 Abs. 1 lit. b DSGVO |
| Fotos (nur Abos Basic, Pro, CSC) | Pflanzentagebuch | Art. 6 Abs. 1 lit. b DSGVO |
| Wetter (Open-Meteo) | Wetter- und Klimaanzeige | Art. 6 Abs. 1 lit. a DSGVO (Einwilligung) |
| Push-Erinnerungen (Web-App) | Erinnerung an fällige Aufgaben | Art. 6 Abs. 1 lit. a DSGVO (Einwilligung) |
| Gastmodus | Nutzung ohne Konto, nur im Browser | Art. 6 Abs. 1 lit. b DSGVO |
| Abos und Einmalkauf (Stripe) | Zahlung, Abrechnung | Art. 6 Abs. 1 lit. b und lit. c DSGVO |
| Desktop-Lizenzen | Freischaltung der Desktop-App | Art. 6 Abs. 1 lit. b DSGVO |
| Mess- und Steuergeräte (nur Desktop) | Anzeige von Sensorwerten | Art. 6 Abs. 1 lit. a DSGVO (Einwilligung) |
| Server-Protokolle, Missbrauchsschutz | IT-Sicherheit, Fehlersuche | Art. 6 Abs. 1 lit. f DSGVO (berechtigtes Interesse) |

Einwilligungen fragt die App beim ersten Start ab. Sie können sie jederzeit in
den Kontoeinstellungen widerrufen; der Widerruf wirkt für die Zukunft
(Art. 7 Abs. 3 DSGVO).

## 3. Konto und Gartendaten

**Daten:** E-Mail-Adresse, Passwort (nur als Argon2-Hash), Sprache, Zeitzone,
Tarif, Erstellungsdatum; Ihre Pflanzen, Aufgaben, Logs (Notizen, Messwerte,
Ernten), Standorte und Saisons.

**Web-App:** Die Daten liegen auf unserem Server, damit Sie sie auf allen
Geräten nutzen können. Das ist der Kern des Dienstes, daher Vertrag als
Rechtsgrundlage.

**Desktop-App:** Die Daten liegen verschlüsselt (SQLCipher) auf Ihrem Computer.
Mit einem Konto und Ihrer Einwilligung „Cloud-Synchronisation“ werden sie mit
dem Server abgeglichen.

**Standort-Koordinaten** speichern wir nur auf etwa 1 km gerundet (zwei
Nachkommastellen).

## 4. Einzelne Funktionen

### 4.1 Fotos

Fotos können nur mit einem Abo (Basic, Pro, CSC) hochgeladen werden.

- Jedes Foto wird beim Hochladen **neu berechnet**: auf höchstens
  1000 × 1000 Pixel verkleinert, als WebP gespeichert und dabei **von allen
  Metadaten befreit** (z. B. EXIF, GPS-Position, Kameramodell).
- In der Desktop-App passiert dasselbe lokal; die Fotos bleiben auf Ihrem Computer.
- Fotos werden gelöscht, wenn Sie das Foto, die Pflanze oder Ihr Konto löschen.
- **Endet Ihr bezahltes Abo**, bleiben die Fotos 90 Tage sichtbar. Die App zeigt
  das Löschdatum an und bietet einen Download aller Fotos als ZIP-Datei an.
  Schließen Sie innerhalb der 90 Tage kein neues Abo ab, werden alle Ihre Fotos
  gelöscht.

### 4.2 Wetter (Open-Meteo)

Mit Ihrer Einwilligung „Wetter“ ruft die App Wetterdaten bei
**Open-Meteo.com** (Open-Meteo, Patrick Zippenfenig, Deutschland) ab.

- Übermittelt werden die auf etwa 1 km gerundeten Koordinaten bzw. der gesuchte
  Ortsname. **Die Web-App ruft Open-Meteo direkt aus Ihrem Browser auf**; dabei
  erhält Open-Meteo technisch bedingt auch Ihre IP-Adresse.
- Mit Konto speichern wir die abgerufenen Messwerte als **Wetter-Historie** auf
  dem Server (in der Desktop-App lokal), damit Sie Verläufe sehen können. Die
  Aufbewahrungsdauer stellen Sie selbst ein; spätestens nach **365 Tagen** werden
  Einträge automatisch gelöscht.
- Ohne Einwilligung findet kein Abruf statt.

### 4.3 Push-Erinnerungen (Web-App)

Wenn Sie Push-Erinnerungen einschalten und der Browser-Anfrage zustimmen,
erhalten Sie Erinnerungen an fällige Aufgaben, auch wenn die App geschlossen ist.

- Ihr Browser erzeugt dafür eine Push-Adresse bei seinem Push-Dienst. Je nach
  Browser ist das **Google** (Chrome, Edge, Android), **Mozilla** (Firefox),
  **Apple** (Safari) oder **Microsoft**. Wir speichern diese Adresse zusammen mit
  Sprache und Zeitzonen-Versatz des Geräts.
- Der Inhalt jeder Nachricht ist **Ende-zu-Ende verschlüsselt** (RFC 8291). Der
  Push-Dienst sieht nur, dass und wann eine Nachricht zugestellt wird, nicht
  deren Inhalt.
- Die Push-Dienste können ihren Sitz außerhalb der EU haben (siehe Abschnitt 6).
- Die Push-Adresse wird gelöscht, wenn Sie Push ausschalten, sich abmelden, Ihr
  Konto löschen oder der Push-Dienst sie für ungültig erklärt.
- Welche Erinnerung bereits verschickt wurde, speichern wir 60 Tage.

### 4.4 Gastmodus (Web-App)

Ohne Konto können Sie die Web-App im Gastmodus nutzen. Alle Daten bleiben dann
**ausschließlich in Ihrem Browser** (IndexedDB) und werden nicht an uns
übertragen. Sie gehen verloren, wenn Sie die Websitedaten löschen.

Melden Sie sich später an, fragt die App, ob die Gastdaten in Ihr Konto
übernommen werden sollen. Erst nach Ihrer Bestätigung werden sie übertragen und
danach im Browser gelöscht. Sie können die Gastdaten auch jederzeit löschen.

### 4.5 Abos und Einmalkauf (Stripe)

Zahlungen wickelt **Stripe** ab. Für den Bezahlvorgang werden Sie zu Stripe
weitergeleitet; dabei übermitteln wir Ihre E-Mail-Adresse und eine interne
Kundenkennung. Zahlungsdaten (z. B. Karte) erhalten nur Stripe, nicht wir.

Wir speichern Tarif, Stripe-Kunden-ID und den Verlauf von Tarifwechseln. Vor dem
ersten Bezahlvorgang holt die App Ihre Zustimmung zur Weitergabe an Stripe ein.

### 4.6 Desktop-Lizenzen

Die Desktop-App ist im Abo (Basic, Pro, CSC) enthalten oder als Einmalkauf
erhältlich. Für die Freischaltung verarbeiten wir:

- **Lizenz:** Art (Kauf oder von uns vergeben), Ihre E-Mail-Adresse bzw.
  Kontokennung, Anzahl erlaubter Geräte, Erstellungs- und ggf. Widerrufsdatum.
  Lizenzschlüssel speichern wir **nur als Hash**; Käufer sehen ihren Schlüssel
  in ihrem Konto.
- **Aktivierte Geräte:** eine zufällige Installationskennung, der
  **Computername** (damit Sie Ihre Geräte in der Liste erkennen), Zeitpunkt der
  Aktivierung und der letzten Prüfung.
- Die Desktop-App prüft ihre Lizenz etwa einmal täglich online. Dabei werden nur
  das Lizenz-Zertifikat und die Installationskennung übertragen.

Löschen Sie Ihr Konto, löschen wir Lizenzen ohne Schlüssel. Lizenzen mit
Schlüssel bleiben gültig, verlieren aber jeden Bezug zu Ihrer Person
(E-Mail-Adresse und Kontokennung werden entfernt).

### 4.7 Mess- und Steuergeräte (nur Desktop-App)

Mit Ihrer Einwilligung koppelt die Desktop-App Bluetooth- und Tuya-Geräte im
lokalen Netzwerk. Gerätekennungen, Schlüssel und Messwerte bleiben auf Ihrem
Computer.

### 4.8 Server-Protokolle und Missbrauchsschutz

Unser Server protokolliert technische Abläufe (Zeitpunkt, aufgerufene Funktion,
Statuscode), um Fehler zu finden und Angriffe zu erkennen. E-Mail-Adressen
erscheinen darin nur pseudonymisiert. Die Protokolle werden fortlaufend
überschrieben und höchstens 30 Tage aufbewahrt.

Zum Schutz vor Passwort- und Schlüssel-Ratversuchen zählen wir Anfragen pro
IP-Adresse; diese Zählung liegt nur im Arbeitsspeicher und verfällt nach
höchstens 15 Minuten.

## 5. Empfänger und Auftragsverarbeiter

- **Hosting:** [Hoster, Anschrift] – Betrieb von Server und Datenbank im
  Auftrag (Art. 28 DSGVO), Serverstandort [Land, EU].
- **Stripe Payments Europe Ltd.** (Irland) – Zahlungsabwicklung; konzernintern
  auch Stripe Inc. (USA).
- **Open-Meteo** (Deutschland) – nur bei erteilter Einwilligung „Wetter“.
- **Push-Dienst Ihres Browsers** (Google, Mozilla, Apple oder Microsoft) – nur
  bei eingeschalteten Push-Erinnerungen; erhält ausschließlich verschlüsselte
  Nachrichten.

Eine Weitergabe zu Werbezwecken findet nicht statt.

## 6. Übermittlung in Drittländer

Stripe Inc. sowie Google LLC, Apple Inc. und Microsoft Corp. haben ihren Sitz in
den USA. Übermittlungen dorthin stützen sich auf den Angemessenheitsbeschluss
zum **EU-US Data Privacy Framework**, soweit das jeweilige Unternehmen
zertifiziert ist, sonst auf EU-Standardvertragsklauseln. Bei den Push-Diensten
übermitteln wir nur verschlüsselte Inhalte.

## 7. Speicherdauer

| Daten | Dauer |
|---|---|
| Konto und Gartendaten | bis Sie sie oder Ihr Konto löschen |
| Fotos | bis zur Löschung; 90 Tage nach Ende eines bezahlten Abos |
| Wetter-Historie | Ihre Einstellung, höchstens 365 Tage |
| Push-Adresse | bis Push aus, Abmeldung oder Ungültigkeit |
| Versandvermerk für Push-Erinnerungen | 60 Tage |
| Anmelde-Sitzungen (Refresh-Tokens) | 90 Tage ab Anmeldung, nach Ablauf bzw. Abmeldung binnen 7 Tagen gelöscht |
| Lösch-Vermerke für die Synchronisation | 180 Tage |
| Verlauf der Tarifwechsel | die letzten 200 Einträge je Konto |
| Desktop-Lizenzen | solange die Lizenz besteht (siehe 4.6) |
| Abrechnungsunterlagen (bei Stripe) | gesetzliche Aufbewahrungsfristen, i. d. R. 10 Jahre |
| Server-Protokolle | höchstens 30 Tage |
| Datensicherungen | 30 Tage, danach überschrieben |

## 8. Cookies und Speicher auf Ihrem Gerät

Wir verwenden **keine Tracking- oder Werbe-Cookies und keine Analysedienste**.

Technisch notwendig (§ 25 Abs. 2 TDDDG, keine Einwilligung erforderlich):

- **Cookie `pc_refresh`** (Web-App): hält Sie angemeldet. Er ist für Skripte
  unlesbar (httpOnly), wird nur verschlüsselt übertragen, nur an unseren Server
  gesendet und läuft nach 90 Tagen ab bzw. wird beim Abmelden gelöscht.
- **Lokaler Speicher** (localStorage, IndexedDB): Einstellungen wie Sprache,
  Design und Erinnerungs-Status sowie im Gastmodus Ihre Gartendaten.
- **App-Cache** (Service Worker): Programmdateien der Web-App, damit sie schnell
  und offline startet. Er enthält keine persönlichen Daten.
- **Desktop-App:** lokale, verschlüsselte Datenbank sowie Lizenz-Zertifikat und
  Installationskennung im Programmdatenordner.

## 9. Ihre Rechte

Sie haben das Recht auf

- **Auskunft** (Art. 15 DSGVO) und **Datenübertragbarkeit** (Art. 20 DSGVO):
  In den Kontoeinstellungen exportieren Sie alle Daten als JSON-Datei; Fotos
  zusätzlich als ZIP;
- **Berichtigung** (Art. 16 DSGVO): direkt in der App;
- **Löschung** (Art. 17 DSGVO): „Konto löschen“ entfernt Ihr Konto, Ihre
  Gartendaten, Fotos, Push-Adressen und den Stripe-Kundeneintrag;
  gesetzlich aufzubewahrende Abrechnungsunterlagen bleiben bei Stripe;
- **Einschränkung der Verarbeitung** (Art. 18 DSGVO);
- **Widerspruch** gegen Verarbeitungen auf Grundlage berechtigter Interessen
  (Art. 21 DSGVO);
- **Widerruf** erteilter Einwilligungen mit Wirkung für die Zukunft
  (Art. 7 Abs. 3 DSGVO), in den Kontoeinstellungen;
- **Beschwerde** bei einer Datenschutz-Aufsichtsbehörde (Art. 77 DSGVO).

Anfragen richten Sie bitte an info@blocksocial.eu.

## 10. Datensicherheit

Alle Verbindungen sind per TLS (HTTPS) verschlüsselt. Passwörter speichern wir
nur als Argon2-Hash, Lizenzschlüssel nur als SHA-256-Hash. Lizenz-Zertifikate
sind digital signiert (Ed25519). Die Datenbank der Desktop-App ist verschlüsselt.
Server-Datensicherungen werden verschlüsselt erstellt und nach 30 Tagen
überschrieben.

## 11. Pflicht zur Bereitstellung

Für ein Konto benötigen wir Ihre E-Mail-Adresse und ein Passwort. Ohne Konto
können Sie die Web-App im Gastmodus nutzen. Alle Einwilligungen sind freiwillig;
ohne sie stehen nur die jeweiligen Funktionen (z. B. Wetter) nicht zur Verfügung.

## 12. Änderungen

Wir passen diese Erklärung an, wenn sich die App oder die Rechtslage ändert. Die
aktuelle Fassung finden Sie jederzeit in der App.
