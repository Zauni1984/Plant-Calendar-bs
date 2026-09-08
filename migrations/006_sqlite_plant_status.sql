ALTER TABLE plants ADD COLUMN plant_status TEXT DEFAULT 'active';
-- SQLite's ADD COLUMN ... DEFAULT 'active' back-fills EVERY existing row with
-- 'active', so a `WHERE plant_status IS NULL OR plant_status = ''` guard would
-- never match and archived/ended plants would silently come back as active.
-- Derive unconditionally instead — this statement only ever runs once, because
-- on later starts the ALTER above fails with "duplicate column" and aborts the
-- batch before reaching it.
UPDATE plants
SET plant_status = CASE
  WHEN archived = 1 THEN 'archived'
  WHEN is_active = 1 THEN 'active'
  ELSE 'ended'
END;
CREATE INDEX IF NOT EXISTS idx_plants_user_status ON plants(user_id, plant_status);
