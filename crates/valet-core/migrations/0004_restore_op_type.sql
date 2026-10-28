-- Allow op_type 'restore': the undo step of a 'trash' operation, which brings
-- the item back from the Recycle Bin to its original path.
-- SQLite cannot change a CHECK constraint, so the table is rebuilt with the
-- same columns and rows. The old table is renamed first, so the new table's
-- self-reference (undo_of) points at the new table; rows are copied in id
-- order, so every undo_of target exists when its row arrives.
ALTER TABLE operations RENAME TO operations_v3;

CREATE TABLE operations (
  -- AUTOINCREMENT: ids are never reused, so `.valet-tmp-<id>` names stay unique.
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id    TEXT NOT NULL REFERENCES batches(id),
  seq         INTEGER NOT NULL,
  op_type     TEXT NOT NULL CHECK (op_type IN ('move', 'copy', 'trash', 'restore')),
  source_path TEXT NOT NULL,
  dest_path   TEXT,
  size        INTEGER,
  hash_before TEXT,
  hash_after  TEXT,
  state       TEXT NOT NULL CHECK (state IN ('planned', 'in_progress', 'done', 'failed', 'rolled_back')),
  error       TEXT,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  finished_at TEXT,
  undo_of     INTEGER REFERENCES operations(id)
);

INSERT INTO operations (id, batch_id, seq, op_type, source_path, dest_path, size,
                        hash_before, hash_after, state, error, created_at, finished_at, undo_of)
  SELECT id, batch_id, seq, op_type, source_path, dest_path, size,
         hash_before, hash_after, state, error, created_at, finished_at, undo_of
  FROM operations_v3 ORDER BY id;

-- Keep the AUTOINCREMENT counter, so ids are never reused.
DELETE FROM sqlite_sequence WHERE name = 'operations';
INSERT INTO sqlite_sequence (name, seq)
  SELECT 'operations', seq FROM sqlite_sequence WHERE name = 'operations_v3';

-- Drops the old table's indexes with it; they are recreated below.
DROP TABLE operations_v3;

CREATE INDEX IF NOT EXISTS idx_operations_batch ON operations(batch_id, seq);
CREATE INDEX IF NOT EXISTS idx_operations_state ON operations(state);
