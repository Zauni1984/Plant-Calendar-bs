CREATE TABLE IF NOT EXISTS devices (
  device_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  device_type TEXT NOT NULL DEFAULT 'ggs',
  ble_address TEXT NOT NULL,
  linked_plant_id TEXT,
  last_seen TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (linked_plant_id) REFERENCES plants(plant_id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS device_readings (
  reading_id TEXT PRIMARY KEY,
  device_id TEXT NOT NULL,
  plant_id TEXT,
  temp REAL,
  humidity REAL,
  vpd REAL,
  fan_level INTEGER,
  light_level INTEGER,
  created_at TEXT NOT NULL,
  FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_readings_device_time ON device_readings(device_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_readings_plant_time ON device_readings(plant_id, created_at DESC);
