CREATE TABLE IF NOT EXISTS files (
  id            TEXT PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  size          INTEGER NOT NULL,
  mtime         INTEGER NOT NULL,
  hash_short    TEXT,
  tags_json     TEXT DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS rules (
  id        TEXT PRIMARY KEY,
  json      TEXT NOT NULL,
  version   INTEGER NOT NULL DEFAULT 1,
  enabled   INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS actions (
  id          TEXT PRIMARY KEY,
  file_id     TEXT NOT NULL,
  rule_id     TEXT NOT NULL,
  op          TEXT NOT NULL,
  params_json TEXT NOT NULL,
  ts          INTEGER NOT NULL,
  FOREIGN KEY(file_id) REFERENCES files(id),
  FOREIGN KEY(rule_id) REFERENCES rules(id)
);

CREATE TABLE IF NOT EXISTS undo_log (
  id          TEXT PRIMARY KEY,
  action_id   TEXT NOT NULL,
  inverse_op  TEXT NOT NULL,
  params_json TEXT NOT NULL,
  done        INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY(action_id) REFERENCES actions(id)
);

CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
CREATE INDEX IF NOT EXISTS idx_actions_file ON actions(file_id);
CREATE INDEX IF NOT EXISTS idx_actions_rule ON actions(rule_id);
