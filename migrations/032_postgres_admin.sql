-- Admin area (admin.sowling.com): admins sign in with password plus a code
-- from an authenticator app (TOTP). Accounts can be disabled or deleted there.

ALTER TABLE users ADD COLUMN IF NOT EXISTS disabled_at TIMESTAMPTZ NULL;
ALTER TABLE users ADD COLUMN IF NOT EXISTS disabled_reason TEXT NULL;
-- Last sign-in or session renewal, for "active users" figures.
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ NULL;

CREATE TABLE IF NOT EXISTS admins (
    user_id UUID PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,
    totp_secret TEXT NOT NULL,               -- base32
    totp_last_step BIGINT NOT NULL DEFAULT 0, -- a code works only once
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Who did what in the admin area.
CREATE TABLE IF NOT EXISTS admin_audit (
    audit_id UUID PRIMARY KEY,
    admin_id UUID NULL REFERENCES users(user_id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    target TEXT NULL,
    detail TEXT NULL,
    at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_admin_audit_at ON admin_audit(at DESC);
