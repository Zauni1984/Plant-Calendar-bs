-- Billing data and invoices. Every payment (desktop purchase, each
-- subscription payment) becomes an invoice in Lexware Office (Lexoffice); the
-- PDF is mailed to the customer and can be fetched from the account.

-- What the customer entered as invoice recipient. Companies outside Germany
-- with a VAT ID confirmed by VIES get reverse-charge invoices without VAT.
CREATE TABLE IF NOT EXISTS billing_profiles (
    user_id UUID PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,
    is_company BOOLEAN NOT NULL DEFAULT FALSE,
    name TEXT NOT NULL,                  -- person or company name
    street TEXT NOT NULL DEFAULT '',
    zip TEXT NOT NULL DEFAULT '',
    city TEXT NOT NULL DEFAULT '',
    country_code TEXT NOT NULL DEFAULT 'DE',
    vat_id TEXT NULL,
    vat_valid BOOLEAN NOT NULL DEFAULT FALSE,
    vat_checked_at TIMESTAMPTZ NULL,
    vat_request_id TEXT NULL,            -- VIES consultation number (proof)
    lexoffice_contact_id TEXT NULL,      -- created when first needed
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- One row per payment. Kept when the account is deleted (user_id becomes
-- NULL): invoices fall under the statutory retention periods.
CREATE TABLE IF NOT EXISTS invoices (
    invoice_id UUID PRIMARY KEY,
    user_id UUID NULL REFERENCES users(user_id) ON DELETE SET NULL,
    stripe_ref TEXT NOT NULL UNIQUE,     -- Stripe invoice or checkout session id
    kind TEXT NOT NULL,                  -- subscription | desktop
    description TEXT NOT NULL,
    amount_cents BIGINT NOT NULL,
    currency TEXT NOT NULL,
    paid_at TIMESTAMPTZ NOT NULL,
    period_start TIMESTAMPTZ NULL,
    period_end TIMESTAMPTZ NULL,
    email TEXT NULL,
    recipient JSONB NOT NULL,            -- recipient as of the payment
    tax_type TEXT NULL,                  -- gross | intraCommunitySupply | thirdPartyCountryService
    status TEXT NOT NULL DEFAULT 'pending', -- pending | created | sent | issued | failed
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error TEXT NULL,
    lexoffice_id TEXT NULL,
    voucher_number TEXT NULL,
    emailed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_invoices_user ON invoices(user_id, paid_at DESC);
CREATE INDEX IF NOT EXISTS idx_invoices_due ON invoices(next_attempt_at) WHERE status IN ('pending', 'created');
