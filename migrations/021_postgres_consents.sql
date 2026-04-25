-- Phase C2: GDPR consent persistence (Postgres).
-- Stores per-user consent decisions with policy version for audit trail.
-- ON DELETE CASCADE ensures consents are wiped alongside the user on Art. 17 erasure.
CREATE TABLE IF NOT EXISTS user_consents (
  consent_id UUID PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
  consent_type TEXT NOT NULL,            -- 'cloud_sync','weather','devices','billing_stripe'
  granted BOOLEAN NOT NULL,
  policy_version TEXT NOT NULL,          -- e.g. '1.0'
  granted_at TIMESTAMPTZ NOT NULL,
  revoked_at TIMESTAMPTZ,
  updated_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_user_consents_user ON user_consents(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_consents_unique ON user_consents(user_id, consent_type, policy_version);
