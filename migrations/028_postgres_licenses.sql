-- Desktop app licenses (see shared/src/license.rs and api/licenses.rs).
--
-- A license comes from a one-time purchase (Stripe) or is issued by the
-- operator (tests, beta testers, loyal customers). It can be used with a key,
-- and/or belongs to an account: by user_id (purchases) or by e-mail address
-- (operator grants, so the person only has to sign in with that address).
-- Subscriptions (Basic/Pro/CSC) give access without any row here.
CREATE TABLE IF NOT EXISTS licenses (
  license_id UUID PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('purchase', 'complimentary')),
  -- SHA-256 of the normalized key; the key itself is never stored.
  key_hash TEXT UNIQUE,
  -- Last four characters, to recognise a key in lists.
  key_hint TEXT,
  email TEXT,
  user_id UUID REFERENCES users(user_id) ON DELETE SET NULL,
  note TEXT,
  max_devices INTEGER NOT NULL DEFAULT 3 CHECK (max_devices > 0),
  stripe_session_id TEXT UNIQUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_licenses_email ON licenses (lower(email)) WHERE email IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_licenses_user ON licenses (user_id) WHERE user_id IS NOT NULL;

-- Installations using a license; counts against max_devices.
CREATE TABLE IF NOT EXISTS license_activations (
  license_id UUID NOT NULL REFERENCES licenses(license_id) ON DELETE CASCADE,
  device_id TEXT NOT NULL,
  device_name TEXT,
  activated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (license_id, device_id)
);
