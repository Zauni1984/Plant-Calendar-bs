-- 019: Local user profiles for multi-user support within a single app installation
CREATE TABLE IF NOT EXISTS user_profiles (
  profile_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'gardener',
  color TEXT NOT NULL DEFAULT '#6bffbc',
  avatar_emoji TEXT DEFAULT '🧑‍🌾',
  is_owner INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- assigned_profile_id on plants (which profile is responsible)
ALTER TABLE plants ADD COLUMN assigned_profile_id TEXT;

-- assigned_profile_id on tasks
ALTER TABLE tasks ADD COLUMN assigned_profile_id TEXT;

-- created_by_profile_id on logs (who created the log)
ALTER TABLE logs ADD COLUMN created_by_profile_id TEXT;
