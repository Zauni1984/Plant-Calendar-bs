# Privacy Policy – Plants Calendar

**Last updated: 16 October 2026**

<!-- Before publishing: fill in the address (section 1), hosting provider and
     server location (section 5), and the app name after the rename. Add a
     section once e-mail sending (Hostinger Mail) is live. The German version
     is authoritative; a legal review before launch is recommended. -->

This policy explains, in line with Art. 12–14 GDPR, which personal data we
process when you use **Plants Calendar** – as a **web app** in your browser
(also installed on a phone or tablet) or as a **desktop app** for Windows.
In case of doubt, the German version applies.

## 1. Controller

BlockSocial UG (haftungsbeschränkt)
[Street, number]
[Postcode, city], Germany
E-mail: info@blocksocial.eu

No data protection officer is appointed, as there is no obligation to do so
(Art. 37 GDPR).

## 2. Overview

| Processing | Purpose | Legal basis |
|---|---|---|
| Account (e-mail, password hash) | Sign-in, providing the service | Art. 6(1)(b) GDPR (contract) |
| Plants, tasks, logs, locations, seasons | Core function of the app | Art. 6(1)(b) GDPR |
| Photos (Basic, Pro, CSC plans only) | Plant diary | Art. 6(1)(b) GDPR |
| Weather (Open-Meteo) | Weather and climate display | Art. 6(1)(a) GDPR (consent) |
| Push reminders (web app) | Reminders of due tasks | Art. 6(1)(a) GDPR (consent) |
| Guest mode | Use without an account, browser only | Art. 6(1)(b) GDPR |
| Subscriptions and one-time purchase (Stripe) | Payment, invoicing | Art. 6(1)(b) and (c) GDPR |
| Desktop licenses | Unlocking the desktop app | Art. 6(1)(b) GDPR |
| Sensors and controllers (desktop only) | Showing sensor values | Art. 6(1)(a) GDPR (consent) |
| Server logs, abuse protection | IT security, troubleshooting | Art. 6(1)(f) GDPR (legitimate interest) |

The app asks for consent on first start. You can withdraw consent at any time
in the account settings, with effect for the future (Art. 7(3) GDPR).

## 3. Account and garden data

**Data:** e-mail address, password (Argon2 hash only), language, time zone,
plan, creation date; your plants, tasks, logs (notes, measurements, harvests),
locations and seasons.

**Web app:** the data is stored on our server so you can use it on all your
devices. This is the core of the service, hence contract as the legal basis.

**Desktop app:** the data is stored encrypted (SQLCipher) on your computer.
With an account and your "cloud sync" consent it is synchronised with the server.

**Location coordinates** are stored rounded to about 1 km (two decimal places).

## 4. Individual features

### 4.1 Photos

Photos can only be uploaded with a subscription (Basic, Pro, CSC).

- Every photo is **re-encoded** on upload: scaled to at most 1000 × 1000 pixels,
  stored as WebP and **stripped of all metadata** (e.g. EXIF, GPS position,
  camera model).
- The desktop app does the same locally; photos stay on your computer.
- Photos are deleted when you delete the photo, the plant or your account.
- **When your paid subscription ends**, photos remain visible for 90 days. The
  app shows the deletion date and offers a ZIP download of all photos. Unless
  you subscribe again within those 90 days, all your photos are deleted.

### 4.2 Weather (Open-Meteo)

With your "weather" consent the app fetches weather data from **Open-Meteo.com**
(Open-Meteo, Patrick Zippenfenig, Germany).

- Coordinates rounded to about 1 km, or the place name you search for, are
  transmitted. **The web app calls Open-Meteo directly from your browser**, so
  Open-Meteo technically also receives your IP address.
- With an account we store the fetched readings as **weather history** on the
  server (locally in the desktop app) so you can see trends. You choose the
  retention period; entries are deleted automatically after **365 days** at the
  latest.
- Without consent, no data is fetched.

### 4.3 Push reminders (web app)

If you turn on push reminders and accept the browser prompt, you receive
reminders of due tasks even when the app is closed.

- Your browser creates a push address with its push service – depending on the
  browser **Google** (Chrome, Edge, Android), **Mozilla** (Firefox), **Apple**
  (Safari) or **Microsoft**. We store this address with the device's language
  and time-zone offset.
- The content of every message is **end-to-end encrypted** (RFC 8291). The push
  service only sees that and when a message is delivered, not its content.
- Push services may be located outside the EU (see section 6).
- The push address is deleted when you turn push off, sign out, delete your
  account, or the push service declares it invalid.
- We keep a record of which reminder was already sent for 60 days.

### 4.4 Guest mode (web app)

Without an account you can use the web app in guest mode. All data then stays
**only in your browser** (IndexedDB) and is not transmitted to us. It is lost
if you clear the site data.

If you sign in later, the app asks whether the guest data should be moved into
your account. Only after you confirm is it transmitted, and then removed from
the browser. You can also delete the guest data at any time.

### 4.5 Subscriptions and one-time purchase (Stripe)

Payments are handled by **Stripe**. You are redirected to Stripe for payment;
we pass on your e-mail address and an internal customer reference. Payment
details (e.g. card) are received only by Stripe, not by us.

We store your plan, Stripe customer ID and the history of plan changes. Before
the first payment the app asks for your agreement to pass data to Stripe.

### 4.6 Desktop licenses

The desktop app is included in a subscription (Basic, Pro, CSC) or available as
a one-time purchase. To unlock it we process:

- **License:** type (purchase or issued by us), your e-mail address or account
  reference, number of allowed devices, creation and, if applicable, revocation
  date. License keys are stored **as a hash only**; buyers can see their key in
  their account.
- **Activated devices:** a random installation ID, the **computer name** (so you
  can recognise your devices in the list), and the time of activation and of the
  last check.
- The desktop app checks its license online about once a day, sending only the
  license certificate and the installation ID.

If you delete your account, licenses without a key are deleted. Licenses with a
key remain valid but lose any link to you (e-mail address and account reference
are removed).

### 4.7 Sensors and controllers (desktop app only)

With your consent the desktop app pairs Bluetooth and Tuya devices on your local
network. Device identifiers, keys and readings stay on your computer.

### 4.8 Server logs and abuse protection

Our server logs technical events (time, function called, status code) to find
errors and detect attacks. E-mail addresses only appear pseudonymised. Logs are
continuously overwritten and kept for at most 30 days.

To protect against password and key guessing we count requests per IP address;
this count is held in memory only and expires after at most 15 minutes.

## 5. Recipients and processors

- **Hosting:** [provider, address] – operating the server and database on our
  behalf (Art. 28 GDPR), server location [country, EU].
- **Stripe Payments Europe Ltd.** (Ireland) – payment processing; within its
  group also Stripe Inc. (USA).
- **Open-Meteo** (Germany) – only with your "weather" consent.
- **Your browser's push service** (Google, Mozilla, Apple or Microsoft) – only
  with push reminders turned on; receives encrypted messages only.

We do not pass data on for advertising.

## 6. Transfers to third countries

Stripe Inc., Google LLC, Apple Inc. and Microsoft Corp. are based in the USA.
Transfers rely on the adequacy decision for the **EU-US Data Privacy Framework**
where the company is certified, otherwise on EU standard contractual clauses.
Push services only receive encrypted content.

## 7. Retention

| Data | Period |
|---|---|
| Account and garden data | until you delete them or your account |
| Photos | until deleted; 90 days after a paid subscription ends |
| Weather history | your setting, at most 365 days |
| Push address | until push is turned off, you sign out, or it becomes invalid |
| Record of sent push reminders | 60 days |
| Sign-in sessions (refresh tokens) | 90 days from sign-in; deleted within 7 days after expiry or sign-out |
| Deletion markers for synchronisation | 180 days |
| History of plan changes | last 200 entries per account |
| Desktop licenses | as long as the license exists (see 4.6) |
| Invoicing records (at Stripe) | statutory retention, usually 10 years |
| Server logs | at most 30 days |
| Backups | 30 days, then overwritten |

## 8. Cookies and storage on your device

We use **no tracking or advertising cookies and no analytics services**.

Strictly necessary (§ 25(2) TDDDG, no consent required):

- **Cookie `pc_refresh`** (web app): keeps you signed in. It cannot be read by
  scripts (httpOnly), is only sent encrypted and only to our server, and expires
  after 90 days or when you sign out.
- **Local storage** (localStorage, IndexedDB): settings such as language,
  design and reminder status, and in guest mode your garden data.
- **App cache** (service worker): the web app's program files so it starts
  quickly and offline. It contains no personal data.
- **Desktop app:** local encrypted database plus license certificate and
  installation ID in the program data folder.

## 9. Your rights

You have the right to

- **access** (Art. 15 GDPR) and **data portability** (Art. 20 GDPR): export all
  data as a JSON file in the account settings, photos additionally as a ZIP;
- **rectification** (Art. 16 GDPR): directly in the app;
- **erasure** (Art. 17 GDPR): "Delete account" removes your account, garden
  data, photos, push addresses and the Stripe customer record; invoicing records
  that must be kept by law remain at Stripe;
- **restriction of processing** (Art. 18 GDPR);
- **object** to processing based on legitimate interests (Art. 21 GDPR);
- **withdraw** consent with effect for the future (Art. 7(3) GDPR), in the
  account settings;
- **lodge a complaint** with a data protection supervisory authority
  (Art. 77 GDPR).

Please send requests to info@blocksocial.eu.

## 10. Security

All connections are encrypted with TLS (HTTPS). Passwords are stored only as
Argon2 hashes, license keys only as SHA-256 hashes. License certificates are
digitally signed (Ed25519). The desktop app's database is encrypted. Server
backups are encrypted and overwritten after 30 days.

## 11. Obligation to provide data

An account requires an e-mail address and a password. Without an account you
can use the web app in guest mode. All consents are voluntary; without them only
the respective features (e.g. weather) are unavailable.

## 12. Changes

We update this policy when the app or the law changes. The current version is
always available in the app.
