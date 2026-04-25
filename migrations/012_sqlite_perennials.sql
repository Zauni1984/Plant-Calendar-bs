CREATE TABLE IF NOT EXISTS plant_seasons (
  season_id TEXT PRIMARY KEY,
  plant_id TEXT NOT NULL,
  year INTEGER NOT NULL,
  started_at TEXT,
  ended_at TEXT,
  notes TEXT,
  harvest_kg REAL,
  harvest_count INTEGER,
  pruning_done INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (plant_id) REFERENCES plants(plant_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_seasons_plant_year ON plant_seasons(plant_id, year DESC);
ALTER TABLE plants ADD COLUMN lifecycle TEXT DEFAULT 'annual';
ALTER TABLE plants ADD COLUMN plant_year INTEGER;
