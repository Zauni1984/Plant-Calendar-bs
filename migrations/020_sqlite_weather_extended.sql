-- Extended weather metrics for richer long-term historical logging.
ALTER TABLE weather_history ADD COLUMN apparent_temp_c REAL;
ALTER TABLE weather_history ADD COLUMN uv_index REAL;
ALTER TABLE weather_history ADD COLUMN cloud_cover REAL;
ALTER TABLE weather_history ADD COLUMN pressure_hpa REAL;
ALTER TABLE weather_history ADD COLUMN dew_point_c REAL;
ALTER TABLE weather_history ADD COLUMN visibility_km REAL;
ALTER TABLE weather_history ADD COLUMN wind_gusts_kmh REAL;
ALTER TABLE weather_history ADD COLUMN wind_direction_deg REAL;
ALTER TABLE weather_history ADD COLUMN soil_temp_c REAL;
ALTER TABLE weather_history ADD COLUMN soil_moisture REAL;
ALTER TABLE weather_history ADD COLUMN is_day INTEGER;
