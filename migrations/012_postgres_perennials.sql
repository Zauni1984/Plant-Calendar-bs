CREATE TABLE IF NOT EXISTS plant_seasons (
  season_id UUID PRIMARY KEY,
  plant_id UUID NOT NULL REFERENCES plants(plant_id) ON DELETE CASCADE,
  year INTEGER NOT NULL,
  started_at TIMESTAMPTZ,
  ended_at TIMESTAMPTZ,
  notes TEXT,
  harvest_kg REAL,
  harvest_count INTEGER,
  pruning_done BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_seasons_plant_year ON plant_seasons(plant_id, year DESC);
ALTER TABLE plants ADD COLUMN IF NOT EXISTS lifecycle TEXT DEFAULT 'annual';
ALTER TABLE plants ADD COLUMN IF NOT EXISTS plant_year INTEGER;
