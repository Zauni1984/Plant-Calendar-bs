ALTER TABLE logs ADD COLUMN updated_at TEXT;
UPDATE logs SET updated_at = created_at WHERE updated_at IS NULL;
