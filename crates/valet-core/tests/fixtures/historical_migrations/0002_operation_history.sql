-- Add operation history table for statistics tracking
CREATE TABLE IF NOT EXISTS operation_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_path TEXT NOT NULL,
  destination_path TEXT NOT NULL,
  operation_type TEXT NOT NULL, -- 'move' or 'copy'
  rule_name TEXT NOT NULL,
  status TEXT NOT NULL, -- 'success' or 'failed'
  error_message TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_operation_history_created_at ON operation_history(created_at);
CREATE INDEX IF NOT EXISTS idx_operation_history_status ON operation_history(status);
CREATE INDEX IF NOT EXISTS idx_operation_history_rule ON operation_history(rule_name);
