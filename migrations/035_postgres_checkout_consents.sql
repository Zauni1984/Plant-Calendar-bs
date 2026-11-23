-- Proof of the consumer's consent before a paid checkout (§ 356 Abs. 4/5 BGB):
-- a subscription starts immediately (pro-rata payment on withdrawal), the
-- desktop licence is supplied immediately (right of withdrawal expires).
-- Kept after account deletion as legal proof (user_id set to NULL, e-mail kept).

CREATE TABLE IF NOT EXISTS checkout_consents (
    consent_id UUID PRIMARY KEY,
    user_id UUID NULL REFERENCES users(user_id) ON DELETE SET NULL,
    email TEXT NULL,
    kind TEXT NOT NULL,              -- 'subscription' | 'desktop'
    plan TEXT NULL,
    text_version TEXT NOT NULL,      -- which wording was shown
    accepted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_checkout_consents_user ON checkout_consents(user_id, accepted_at DESC);
