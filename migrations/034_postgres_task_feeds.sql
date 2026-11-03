-- Planned fertilizers on a task (category feed): JSON list of
-- {brand, product, amount, unit}. Completing the task suggests a feeding log.
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS feeds_json JSONB NULL;
