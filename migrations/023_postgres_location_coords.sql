-- Per-location coordinates, needed for weather by location in the web app.
-- Stored rounded to ~1 km by the backend (data minimisation: the coordinates
-- of a garden are often a home address, and weather needs no more precision).
ALTER TABLE locations ADD COLUMN IF NOT EXISTS latitude DOUBLE PRECISION;
ALTER TABLE locations ADD COLUMN IF NOT EXISTS longitude DOUBLE PRECISION;
