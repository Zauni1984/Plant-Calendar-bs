-- Tombstones were keyed globally on (record_id, entity_kind), and the upsert
-- rewrote user_id on conflict. That let any user claim another user's tombstone
-- by pushing a delete for the same record id, which then disappeared from the
-- rightful owner's /sync/pull.
ALTER TABLE deleted_records DROP CONSTRAINT IF EXISTS deleted_records_pkey;
DELETE FROM deleted_records a
  USING deleted_records b
  WHERE a.ctid < b.ctid
    AND a.record_id = b.record_id
    AND a.entity_kind = b.entity_kind
    AND a.user_id = b.user_id;
ALTER TABLE deleted_records
  ADD CONSTRAINT deleted_records_pkey PRIMARY KEY (user_id, record_id, entity_kind);
