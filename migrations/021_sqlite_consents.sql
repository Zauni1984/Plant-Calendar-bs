-- Phase C2: GDPR consent persistence (SQLite).
-- Stores per-user consent decisions with policy version for audit trail.
CREATE TABLE IF NOT EXISTS user_consents (
  consent_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  consent_type TEXT NOT NULL,       -- 'cloud_sync','weather','devices','billing_stripe'
  granted INTEGER NOT NULL,         -- 0/1
  policy_version TEXT NOT NULL,     -- e.g. '1.0'
  granted_at TEXT NOT NULL,         -- ISO
  revoked_at TEXT,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_user_consents_user ON user_consents(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_consents_unique ON user_consents(user_id, consent_type, policy_version);
