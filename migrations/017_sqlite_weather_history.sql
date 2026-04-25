CREATE TABLE IF NOT EXISTS weather_history (
  id TEXT PRIMARY KEY,
  location_id TEXT,
  latitude REAL NOT NULL,
  longitude REAL NOT NULL,
  temp_c REAL,
  humidity REAL,
  wind_kmh REAL,
  precipitation_mm REAL,
  weather_code INTEGER,
  recorded_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_weather_history_time ON weather_history(recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_weather_history_location ON weather_history(location_id, recorded_at DESC);
ALTER TABLE locations ADD COLUMN latitude REAL;
ALTER TABLE locations ADD COLUMN longitude REAL;
