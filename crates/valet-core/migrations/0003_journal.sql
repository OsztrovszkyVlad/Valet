-- Write-ahead journal of file operations (PLAN.md 4.2).
CREATE TABLE IF NOT EXISTS batches (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('apply', 'undo', 'recovery')),
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  finished_at TEXT,
  summary     TEXT
);

CREATE TABLE IF NOT EXISTS operations (
  -- AUTOINCREMENT: ids are never reused, so `.valet-tmp-<id>` names stay unique.
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id    TEXT NOT NULL REFERENCES batches(id),
  seq         INTEGER NOT NULL,
  op_type     TEXT NOT NULL CHECK (op_type IN ('move', 'copy', 'trash')),
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

CREATE INDEX IF NOT EXISTS idx_operations_batch ON operations(batch_id, seq);
CREATE INDEX IF NOT EXISTS idx_operations_state ON operations(state);
CREATE INDEX IF NOT EXISTS idx_batches_created ON batches(created_at);
