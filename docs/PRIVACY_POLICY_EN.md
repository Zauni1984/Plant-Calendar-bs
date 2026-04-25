# Privacy Policy – Plants Calendar

**Last updated: 2026-04-22**

This privacy policy informs you pursuant to Art. 12–14 GDPR about the
processing of personal data in connection with the use of the
**Plants Calendar** application (desktop and backend services).

## 1. Controller

BlockSocial UG (haftungsbeschränkt)
[Address to be added]
Email: info@blocksocial.eu

No Data Protection Officer is mandatorily appointed under Art. 37 GDPR.

## 2. Purposes and legal bases

| Activity | Purpose | Legal basis |
|----------|---------|--------------|
| Account creation, login, password management | Provisioning of user accounts | Art. 6(1)(b) GDPR (contract) |
| Cloud synchronisation of plant, task, and log data | Device sync | Art. 6(1)(b) GDPR (contract) |
| Retrieval of local weather data (Open-Meteo) via coordinates | Weather/climate context | Art. 6(1)(f) GDPR (legitimate interest – functional display) |
| Pairing of BLE / Tuya measurement devices | Display of sensor values | Art. 6(1)(a) GDPR (consent) |
| Subscription management / payment processing | Contract performance, billing | Art. 6(1)(b) GDPR (contract), Art. 6(1)(c) GDPR (tax / commercial law) |
| Server logs (no plaintext email) | IT security, diagnostics | Art. 6(1)(f) GDPR (legitimate interest) |

## 3. Categories of data

- **Account data**: email address, password hash, locale/timezone
  preference, account status, creation date.
- **Plant, task, and log data**: plant names, photos (optional),
  phases, due dates, notes, harvest information.
- **Location data**: coordinates of user-created locations (entered
  manually or derived from device sharing).
- **Device keys**: local identifiers and keys of BLE/Tuya devices
  (stored only on the end user's device).
- **Billing data**: Stripe customer ID, subscription status, payment
  history (processed by Stripe).

## 4. Recipients

- **Stripe Payments Europe Ltd** (Ireland) – payment processing and
  subscription management, governed by the Stripe DPA.
- **Stripe Inc.** (USA) – intra-group transfer. Stripe is certified
  under the **EU-US Data Privacy Framework** (Commission adequacy
  decision of 10 July 2023).
- **Open-Meteo** (Bremen, Germany) – retrieval of aggregated weather
  data. No personal data is transferred; only coordinates are sent.
- **Backend hosting provider**: [Host to be named] – processor per
  Art. 28 GDPR; servers located in the EU.

## 5. International transfers

Transfers to Stripe Inc. (USA) rely on the **EU-US Data Privacy
Framework**. Beyond that, no transfers to third countries take place.

## 6. Retention

- **Account data**: until the user deletes the account.
- **Weather history (local)**: **365 days** (default), then pruned
  automatically.
- **Sync tombstones**: 30 days for conflict resolution.
- **Billing data**: as required by commercial/tax law (typically
  10 years) at Stripe.
- **Server logs**: max. 30 days, pseudonymised.

## 7. Data-subject rights

You may at any time:

- **Access** (Art. 15 GDPR) – available in-app as a JSON export;
- **Rectify** (Art. 16 GDPR) – edit directly in the app;
- **Erase** (Art. 17 GDPR) – "Delete account" removes all server data
  and the Stripe customer entry;
- **Restrict processing** (Art. 18 GDPR);
- **Data portability** (Art. 20 GDPR) – JSON/CSV export;
- **Withdraw consent** (Art. 7(3) GDPR) – effective prospectively;
- **Object** (Art. 21 GDPR);
- **Complain** to a supervisory authority (State data-protection
  authority, Art. 77 GDPR).

## 8. Contact for GDPR requests

Email: info@blocksocial.eu

## 9. Cookies and tracking

The application uses **no cookies** and **no tracking or analytics
SDKs**. Only app preferences are stored locally in `localStorage` and
a local SQLite database. This data stays on the user's device.

## 10. Security

All backend connections use TLS (HTTPS). Passwords are stored only as
hashes (Argon2/bcrypt). Infrastructure access is restricted to
authorised personnel.

## 11. Changes to this policy

We amend this policy when legal or technical conditions change. The
current version is available in-app and at
`docs/PRIVACY_POLICY_EN.md` in the repository.
