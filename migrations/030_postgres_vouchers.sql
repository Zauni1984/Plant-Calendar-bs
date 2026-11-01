-- Vouchers: codes that unlock a paid plan for a while (or for good), for
-- influencers, flyers, shop orders. Two kinds:
--   single  one code, one account; created in batches, printed once
--   shared  one readable code (e.g. FRUEHLING26) for many accounts
-- Only a hash of each code is stored; shared codes also keep their text so
-- the operator can find them again.
CREATE TABLE IF NOT EXISTS vouchers (
    voucher_id UUID PRIMARY KEY,
    batch_id UUID NOT NULL,
    kind TEXT NOT NULL,
    code_hash TEXT NOT NULL UNIQUE,
    code_display TEXT NULL,
    plan TEXT NOT NULL,
    duration_days INTEGER NULL,          -- NULL: unlimited
    max_redemptions INTEGER NULL,        -- NULL: unlimited (shared codes)
    redemption_count INTEGER NOT NULL DEFAULT 0,
    redeem_until TIMESTAMPTZ NULL,       -- last day the code can be redeemed
    note TEXT NOT NULL DEFAULT '',
    revoked_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_vouchers_batch ON vouchers(batch_id);

CREATE TABLE IF NOT EXISTS voucher_redemptions (
    voucher_id UUID NOT NULL REFERENCES vouchers(voucher_id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    redeemed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (voucher_id, user_id)
);

-- users.plan stays the effective plan everything reads. It is now derived
-- from two sources, so that one ending does not wipe out the other:
--   stripe_plan               the paid Stripe subscription
--   voucher_plan/voucher_until a redeemed voucher (until NULL: unlimited)
ALTER TABLE users ADD COLUMN IF NOT EXISTS stripe_plan TEXT NULL;
ALTER TABLE users ADD COLUMN IF NOT EXISTS voucher_plan TEXT NULL;
ALTER TABLE users ADD COLUMN IF NOT EXISTS voucher_until TIMESTAMPTZ NULL;

-- Every paid plan so far came from Stripe.
UPDATE users SET stripe_plan = plan WHERE plan <> 'free' AND stripe_plan IS NULL;
