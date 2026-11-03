-- Optional two-factor sign-in for accounts: a code from an authenticator app
-- (TOTP, e.g. Google Authenticator) after the password. A row without
-- enabled_at is a setup that was started but not yet confirmed.

CREATE TABLE IF NOT EXISTS user_totp (
    user_id UUID PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,
    secret TEXT NOT NULL,                          -- base32
    enabled_at TIMESTAMPTZ NULL,
    last_step BIGINT NOT NULL DEFAULT 0,           -- a code works only once
    recovery_hashes TEXT[] NOT NULL DEFAULT '{}',  -- sha256 of unused recovery codes
    failed_attempts INT NOT NULL DEFAULT 0,
    locked_until TIMESTAMPTZ NULL,                 -- after too many wrong codes
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
