# Deployment Security Checklist

This checklist must be completed before every production launch and after
each major infrastructure change.

## TLS / Transport

- [ ] Backend is exposed **only** through an HTTPS reverse proxy (nginx,
      Caddy, Traefik). No direct `0.0.0.0:8080` exposure.
- [ ] **HSTS** header set (`Strict-Transport-Security: max-age=63072000;
      includeSubDomains; preload`).
- [ ] Certificates from a trusted CA (Let's Encrypt acceptable).
      Auto-renewal verified.
- [ ] Modern TLS config: TLS 1.2+ only, strong ciphers.
- [ ] `CORS_ALLOWED_ORIGINS` restricted to the desktop-ui origin(s);
      no wildcard in production.

## Environment variables (required)

| Variable | Purpose |
|----------|---------|
| `JWT_SECRET` | Signing secret for access tokens. **Release builds refuse to start without it.** |
| `STRIPE_SECRET_KEY` | Stripe API key for checkout / portal / customer delete. |
| `STRIPE_WEBHOOK_SECRET` | Used to verify `Stripe-Signature` headers. |
| `DATABASE_URL` | `postgres://…` connection string. |
| `CORS_ALLOWED_ORIGINS` | Comma-separated list of permitted front-end origins. |

Optional:

- `RUST_LOG=info` (never `debug` in production).

## Database

- [ ] Database runs on a private network, not publicly reachable.
- [ ] Row-level filters enforced on every query through `user_id`
      (verify via code review: repos.rs).
- [ ] Migrations applied up to the latest file in `migrations/`.
- [ ] `plants → plant_seasons`, `plants → tasks`, `plants → logs`
      cascades verified (`ON DELETE CASCADE`).

## Backups (GDPR-aware retention)

- [ ] Nightly full backup, encrypted at rest.
- [ ] **Retention**: 30 days rolling (operational) + 1 monthly snapshot
      for up to 12 months. Longer retention requires a documented
      legal basis.
- [ ] Restoration tested at least quarterly.
- [ ] On user account deletion: backups older than the operational
      window are **not** selectively purged; the retention schedule
      above is documented in the privacy policy.

## Logging

- [ ] Log levels: `info` and above.
- [ ] **Log pseudonymisation already active** – `pseudonymize_email()`
      is used for any email referenced in log output.
- [ ] No plaintext tokens, passwords, or full card numbers logged.
- [ ] Log retention: max. 30 days.

## Stripe

- [ ] **Signed Data Processing Addendum (DPA)** on file with Stripe.
- [ ] Webhook endpoint verified with `STRIPE_WEBHOOK_SECRET`.
- [ ] Production keys used; no `sk_test_*` in prod env.
- [ ] Customer deletion on account delete tested end-to-end
      (`stripe::delete_customer`).

## Rate limiting & abuse

- [ ] Auth endpoints rate-limited (10 req / 60 s).
- [ ] Password change stricter (5 req / 15 min).
- [ ] Reverse-proxy additional protection (mod_evasive / fail2ban).

## Operational

- [ ] Health probe (`GET /health`) wired into orchestration.
- [ ] Monitoring alerts configured for 5xx spikes, error-rate, latency.
- [ ] Incident Response Playbook (`docs/INTERNAL/INCIDENT_RESPONSE.md`)
      accessible to on-call staff.
- [ ] Privacy Policy / Imprint served from app and/or website
      (`docs/PRIVACY_POLICY_DE.md`, `docs/IMPRESSUM_DE.md`).

## Local database encryption (SQLCipher)

The desktop app encrypts its local SQLite database (`plants_calendar_local.sqlite`)
at rest using **SQLCipher** (AES-256). This satisfies GDPR Art. 32 (security of
processing) for personal data stored on the user's device.

- The 32-byte master key is generated on first launch and stored in the OS
  keychain (`db_master_key` under service `blocksocial-plants-calendar`):
  Windows Credential Manager, macOS Keychain, libsecret/KWallet on Linux.
- On upgrade from a pre-SQLCipher build, the plaintext DB is migrated
  automatically on first launch via `sqlcipher_export`. The original plaintext
  file is renamed to `plants_calendar_local.sqlite.bak` and kept indefinitely
  as a safety net. Operators may delete it after confirming successful runs.
- Tuya `local_key` values and other device secrets are also stored in the OS
  keychain (`tuya_<device_id>`); the SQLite row holds only a `keyring:` pointer.

Build prerequisites: on Windows, the `bundled-sqlcipher-vendored-openssl`
feature compiles OpenSSL from source — install Strawberry Perl (or another
full Perl distribution providing `Locale::Maketext::Simple`). macOS and Linux
build out-of-the-box.
