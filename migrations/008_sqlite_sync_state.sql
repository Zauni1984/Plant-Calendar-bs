CREATE TABLE IF NOT EXISTS sync_state (
  record_id   TEXT PRIMARY KEY,
  entity_kind TEXT NOT NULL,
  operation   TEXT NOT NULL,
  changed_at  TEXT NOT NULL
);
