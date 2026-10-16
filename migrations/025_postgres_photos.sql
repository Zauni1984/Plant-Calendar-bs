-- Photos for the web app. Bytes live in the photo store (see
-- services/photo_store.rs); this table holds metadata and ownership.
CREATE TABLE IF NOT EXISTS photos (
  photo_id UUID PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
  plant_id UUID NOT NULL REFERENCES plants(plant_id) ON DELETE CASCADE,
  log_id UUID REFERENCES logs(log_id) ON DELETE SET NULL,
  season_id UUID REFERENCES plant_seasons(season_id) ON DELETE SET NULL,
  caption TEXT,
  taken_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  byte_size INTEGER NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_photos_user_plant_time ON photos(user_id, plant_id, taken_at DESC);
CREATE INDEX IF NOT EXISTS idx_photos_log ON photos(log_id) WHERE log_id IS NOT NULL;

-- Photos are a paid feature. When a paid plan falls back to Free, this records
-- the moment; photos are deleted 90 days later unless the user resubscribes,
-- which clears it again. Maintained by repos::upsert_user_plan.
ALTER TABLE users ADD COLUMN IF NOT EXISTS paid_plan_ended_at TIMESTAMPTZ;

-- Accounts that were already on Free when this ships have no end date, so
-- the clock does not start for them retroactively.
