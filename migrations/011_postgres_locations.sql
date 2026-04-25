CREATE TABLE IF NOT EXISTS locations (
  location_id UUID PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  location_type TEXT NOT NULL DEFAULT 'custom',
  description TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_locations_user ON locations(user_id);
ALTER TABLE plants ADD COLUMN IF NOT EXISTS location_id UUID;
