use anyhow::Result;
use sqlx::{
  sqlite::{SqliteConnectOptions, SqlitePoolOptions},
  SqlitePool,
};
use std::path::Path;
use crate::rules::Rule;
use crate::model::{FileRow, OperationStats, RecentOperation};

pub struct Db { pub pool: SqlitePool }

impl Db {
  pub async fn connect(db_path: &Path) -> Result<Self> {
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
      std::fs::create_dir_all(parent)?;
    }

    // Use connect options instead of URL string to avoid issues with spaces, etc.
    let options = SqliteConnectOptions::new()
      .filename(db_path)
      .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
      .max_connections(5)
      .connect_with(options)
      .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(Self { pool })
  }

  pub async fn upsert_file(&self, f: &FileRow) -> Result<()> {
    sqlx::query(
      r#"
      INSERT INTO files (id, path, size, mtime, hash_short, tags_json)
      VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6,'[]'))
      ON CONFLICT(path) DO UPDATE SET
        size=excluded.size,
        mtime=excluded.mtime,
        hash_short=excluded.hash_short
      "#,
    )
    .bind(f.id.to_string())
    .bind(&f.path)
    .bind(f.size)
    .bind(f.mtime)
    .bind(&f.hash_short)
    .bind(&f.tags_json)
    .execute(&self.pool)
    .await?;
    Ok(())
  }

  pub async fn upsert_rule(&self, rule: &Rule) -> Result<()> {
    let json = serde_json::to_string(rule)?;
    let enabled = if rule.enabled { 1 } else { 0 };
    sqlx::query(
      r#"INSERT INTO rules(id, json, version, enabled)
         VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET json=excluded.json, version=excluded.version, enabled=excluded.enabled"#,
    )
    .bind(rule.id.to_string())
    .bind(json)
    .bind(rule.version)
    .bind(enabled)
    .execute(&self.pool).await?;
    Ok(())
  }

  pub async fn list_rules(&self) -> Result<Vec<Rule>> {
    let rows = sqlx::query_scalar::<_, String>(r#"SELECT json FROM rules ORDER BY rowid"#)
      .fetch_all(&self.pool).await?;
    let mut out = Vec::with_capacity(rows.len());
    for j in rows { out.push(serde_json::from_str(&j)?); }
    Ok(out)
  }

  pub async fn load_sample_rules_if_empty(&self) -> Result<()> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rules").fetch_one(&self.pool).await?;
    if count == 0 {
      let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sample_rules.json");
      let bytes = std::fs::read(path)?;
      let rules: Vec<crate::rules::Rule> = serde_json::from_slice(&bytes)?;
      for r in rules { self.upsert_rule(&r).await?; }
    }
    Ok(())
  }

  // Operation History & Statistics
  pub async fn record_operation(&self, source_path: &str, destination_path: &str, operation_type: &str, rule_name: &str, status: &str, error_message: Option<&str>) -> Result<()> {
    sqlx::query(
      r#"INSERT INTO operation_history (source_path, destination_path, operation_type, rule_name, status, error_message)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
    )
    .bind(source_path)
    .bind(destination_path)
    .bind(operation_type)
    .bind(rule_name)
    .bind(status)
    .bind(error_message)
    .execute(&self.pool).await?;
    Ok(())
  }

  pub async fn get_operation_statistics(&self, days_back: Option<i64>) -> Result<OperationStats> {
    let date_filter = match days_back {
      Some(days) => format!("WHERE created_at >= datetime('now', '-{} days')", days),
      None => String::new(),
    };

    // Basic stats
    let total_query = format!("SELECT COUNT(*) FROM operation_history {}", date_filter);
    let (total_operations,): (i64,) = sqlx::query_as(&total_query).fetch_one(&self.pool).await?;

    let success_query = format!("SELECT COUNT(*) FROM operation_history {} {}", date_filter, if date_filter.is_empty() { "WHERE" } else { "AND" });
    let success_query = format!("{} status = 'success'", success_query);
    let (successful_operations,): (i64,) = sqlx::query_as(&success_query).fetch_one(&self.pool).await?;

    let failed_operations = total_operations - successful_operations;

    // Operation type counts
    let move_query = format!("SELECT COUNT(*) FROM operation_history {} {} operation_type = 'move' AND status = 'success'", date_filter, if date_filter.is_empty() { "WHERE" } else { "AND" });
    let (files_moved,): (i64,) = sqlx::query_as(&move_query).fetch_one(&self.pool).await?;

    let copy_query = format!("SELECT COUNT(*) FROM operation_history {} {} operation_type = 'copy' AND status = 'success'", date_filter, if date_filter.is_empty() { "WHERE" } else { "AND" });
    let (files_copied,): (i64,) = sqlx::query_as(&copy_query).fetch_one(&self.pool).await?;

    // Rules applied count
    let rules_query = format!("SELECT rule_name, COUNT(*) as count FROM operation_history {} {} status = 'success' GROUP BY rule_name", date_filter, if date_filter.is_empty() { "WHERE" } else { "AND" });
    let rules_rows: Vec<(String, i64)> = sqlx::query_as(&rules_query).fetch_all(&self.pool).await?;
    let mut rules_applied_count = std::collections::HashMap::new();
    for (rule_name, count) in rules_rows {
      rules_applied_count.insert(rule_name, count as u64);
    }

    // File types organized
    let file_types_query = format!("SELECT LOWER(SUBSTR(source_path, INSTR(source_path, '.') + 1)) as ext, COUNT(*) as count FROM operation_history {} {} status = 'success' AND INSTR(source_path, '.') > 0 GROUP BY ext", date_filter, if date_filter.is_empty() { "WHERE" } else { "AND" });
    let file_types_rows: Vec<(String, i64)> = sqlx::query_as(&file_types_query).fetch_all(&self.pool).await?;
    let mut file_types_organized = std::collections::HashMap::new();
    for (ext, count) in file_types_rows {
      file_types_organized.insert(ext, count as u64);
    }

    // Average operations per day
    let days_query = match days_back {
      Some(days) => format!("SELECT COUNT(DISTINCT DATE(created_at)) FROM operation_history WHERE created_at >= datetime('now', '-{} days')", days),
      None => "SELECT COUNT(DISTINCT DATE(created_at)) FROM operation_history".to_string(),
    };
    let (active_days,): (i64,) = sqlx::query_as(&days_query).fetch_one(&self.pool).await?;
    let average_operations_per_day = if active_days > 0 {
      total_operations as f64 / active_days as f64
    } else {
      0.0
    };

    // Last operation date
    let last_op_query = format!("SELECT created_at FROM operation_history {} ORDER BY created_at DESC LIMIT 1", date_filter);
    let last_operation_date: Option<String> = sqlx::query_scalar(&last_op_query).fetch_optional(&self.pool).await?;

    Ok(OperationStats {
      total_operations: total_operations as u64,
      successful_operations: successful_operations as u64,
      failed_operations: failed_operations as u64,
      files_moved: files_moved as u64,
      files_copied: files_copied as u64,
      rules_applied_count,
      file_types_organized,
      average_operations_per_day,
      last_operation_date,
    })
  }

  pub async fn get_recent_operations(&self, limit: i64) -> Result<Vec<RecentOperation>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<String>, String)>(
      "SELECT id, source_path, destination_path, operation_type, rule_name, status, error_message, created_at 
       FROM operation_history ORDER BY created_at DESC LIMIT ?1"
    )
    .bind(limit)
    .fetch_all(&self.pool).await?;

    let mut operations = Vec::new();
    for (id, source_path, destination_path, operation_type, rule_name, status, error_message, created_at) in rows {
      operations.push(RecentOperation {
        id: id as u64,
        source_path,
        destination_path,
        operation_type,
        rule_name,
        status,
        error_message,
        created_at,
      });
    }

    Ok(operations)
  }

  pub async fn clear_operation_history(&self) -> Result<()> {
    sqlx::query("DELETE FROM operation_history").execute(&self.pool).await?;
    Ok(())
  }
}
