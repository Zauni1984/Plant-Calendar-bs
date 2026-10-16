-- Climate settings and weather history per account, so they follow the user
-- across devices in the web app. Both cascade on account deletion.

CREATE TABLE IF NOT EXISTS user_climate_config (
  user_id UUID PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,
  latitude DOUBLE PRECISION NOT NULL,
  longitude DOUBLE PRECISION NOT NULL,
  city_name TEXT,
  last_spring_frost TEXT,
  first_fall_frost TEXT,
  timezone TEXT,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS weather_history (
  id UUID PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
  location_id UUID REFERENCES locations(location_id) ON DELETE SET NULL,
  latitude DOUBLE PRECISION NOT NULL,
  longitude DOUBLE PRECISION NOT NULL,
  temp_c REAL,
  humidity REAL,
  wind_kmh REAL,
  precipitation_mm REAL,
  weather_code INTEGER,
  recorded_at TIMESTAMPTZ NOT NULL,
  apparent_temp_c REAL,
  uv_index REAL,
  cloud_cover REAL,
  pressure_hpa REAL,
  dew_point_c REAL,
  visibility_km REAL,
  wind_gusts_kmh REAL,
  wind_direction_deg REAL,
  soil_temp_c REAL,
  soil_moisture REAL,
  is_day BOOLEAN
);
CREATE INDEX IF NOT EXISTS idx_weather_history_user_time
  ON weather_history(user_id, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_weather_history_user_location_time
  ON weather_history(user_id, location_id, recorded_at DESC);
