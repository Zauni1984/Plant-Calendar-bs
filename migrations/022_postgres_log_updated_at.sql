-- logs had no updated_at, so sync could not tell which version of an edited log
-- was newer and an older client silently overwrote a newer edit.
ALTER TABLE logs ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ;
UPDATE logs SET updated_at = created_at WHERE updated_at IS NULL;
ALTER TABLE logs ALTER COLUMN updated_at SET DEFAULT NOW();
