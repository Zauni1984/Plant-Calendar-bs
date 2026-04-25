-- Photos table: local-only, never synced to server
CREATE TABLE IF NOT EXISTS photos (
  photo_id TEXT PRIMARY KEY,
  plant_id TEXT NOT NULL,
  log_id TEXT,
  season_id TEXT,
  file_path TEXT NOT NULL,
  caption TEXT,
  taken_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (plant_id) REFERENCES plants(plant_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_photos_plant ON photos(plant_id, taken_at DESC);
CREATE INDEX IF NOT EXISTS idx_photos_log ON photos(log_id);
