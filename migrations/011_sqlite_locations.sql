CREATE TABLE IF NOT EXISTS locations (
  location_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  name TEXT NOT NULL,
  location_type TEXT NOT NULL DEFAULT 'custom',
  description TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_locations_user ON locations(user_id);
ALTER TABLE plants ADD COLUMN location_id TEXT;
