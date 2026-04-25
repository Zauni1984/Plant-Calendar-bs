CREATE TABLE IF NOT EXISTS climate_config (
  config_key TEXT PRIMARY KEY DEFAULT 'default',
  latitude REAL,
  longitude REAL,
  city_name TEXT,
  last_spring_frost TEXT,
  first_fall_frost TEXT,
  timezone TEXT,
  updated_at TEXT NOT NULL
);
