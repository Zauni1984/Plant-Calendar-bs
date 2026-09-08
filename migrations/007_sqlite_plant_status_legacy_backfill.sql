-- Repairs rows whose plant_status is missing/invalid, and re-derives the legacy
-- is_active/archived flags from it. Runs on every start, so it is restricted to
-- rows that are actually inconsistent — a plain `WHERE 1 = 1` rewrote every
-- plant on each launch.
UPDATE plants
SET
  plant_status = CASE
    WHEN plant_status IN ('active', 'ended', 'archived', 'deleted') THEN plant_status
    WHEN archived = 1 THEN 'archived'
    WHEN is_active = 1 THEN 'active'
    ELSE 'ended'
  END,
  is_active = CASE WHEN plant_status = 'active' THEN 1 ELSE 0 END,
  archived = CASE WHEN plant_status = 'archived' THEN 1 ELSE 0 END
WHERE plant_status IS NULL
   OR plant_status NOT IN ('active', 'ended', 'archived', 'deleted')
   OR is_active <> (CASE WHEN plant_status = 'active' THEN 1 ELSE 0 END)
   OR archived <> (CASE WHEN plant_status = 'archived' THEN 1 ELSE 0 END);
