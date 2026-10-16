-- Web Push reminders for the web app (the desktop notifies from Rust locally).
--
-- One row per browser/device that allowed notifications. The endpoint is the
-- push service URL the browser handed out; p256dh/auth are the keys the
-- payload is encrypted for. An endpoint belongs to one account at a time: if
-- someone else signs in on the same browser it moves to them.
CREATE TABLE IF NOT EXISTS push_subscriptions (
  subscription_id UUID PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
  endpoint TEXT NOT NULL UNIQUE,
  p256dh TEXT NOT NULL,
  auth TEXT NOT NULL,
  -- Language and UTC offset of that device, so the server can write the
  -- notification text the way the app would.
  lang TEXT NOT NULL DEFAULT 'de',
  utc_offset_minutes INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_success_at TIMESTAMPTZ,
  failure_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_push_subscriptions_user ON push_subscriptions(user_id);

-- Which due date of which task has been pushed already. A recurring task gets
-- a new due_at after completion and is reminded again. Rows go with the task
-- and are pruned by the retention sweep after 60 days.
CREATE TABLE IF NOT EXISTS push_reminders_sent (
  task_id UUID NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  due_at TIMESTAMPTZ NOT NULL,
  sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (task_id, due_at)
);
