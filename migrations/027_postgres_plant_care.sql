-- Pot size and soil type. The desktop (SQLite) got these in 018; Postgres
-- never did, yet update_plant writes them, so every plant edit through the
-- REST API failed with "column does not exist".
ALTER TABLE plants ADD COLUMN IF NOT EXISTS pot_size_liters REAL;
ALTER TABLE plants ADD COLUMN IF NOT EXISTS soil_type TEXT;
