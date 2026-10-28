use crate::model::{FileRow, OperationStats, RecentOperation};
use crate::rules::Rule;
use anyhow::Result;
use sqlx::{
    Sqlite, SqlitePool,
    query::Query,
    sqlite::{SqliteArguments, SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::path::Path;

pub struct Db {
    pub pool: SqlitePool,
}

impl Db {
    pub async fn connect(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Use connect options instead of URL string to avoid issues with spaces, etc.
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            // WAL: the operation journal is written while the UI reads history.
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    /// The write-ahead operation journal stored in this database.
    pub fn journal(&self) -> crate::journal::Journal {
        crate::journal::Journal::new(self.pool.clone())
    }

    pub async fn upsert_file(&self, f: &FileRow) -> Result<()> {
        upsert_file_query(f).execute(&self.pool).await?;
        Ok(())
    }

    /// Upserts all rows in one transaction (one WAL commit instead of one per
    /// row). Either every row is written or none is. Used by the indexer;
    /// the operation journal does not use this.
    pub async fn upsert_files(&self, rows: &[FileRow]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for f in rows {
            upsert_file_query(f).execute(&mut *tx).await?;
        }
        tx.commit().await?;
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

    /// Removes the rows of `rules` (by id) and inserts them again in the
    /// given order, in one transaction, so they come after every other rule
    /// in `list_rules` order. Other rules are not touched.
    pub async fn reinsert_rules(&self, rules: &[Rule]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for rule in rules {
            sqlx::query(r#"DELETE FROM rules WHERE id = ?1"#)
                .bind(rule.id.to_string())
                .execute(&mut *tx)
                .await?;
        }
        for rule in rules {
            sqlx::query(r#"INSERT INTO rules(id, json, version, enabled) VALUES(?1, ?2, ?3, ?4)"#)
                .bind(rule.id.to_string())
                .bind(serde_json::to_string(rule)?)
                .bind(rule.version)
                .bind(i32::from(rule.enabled))
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_rules(&self) -> Result<Vec<Rule>> {
        let rows = sqlx::query_scalar::<_, String>(r#"SELECT json FROM rules ORDER BY rowid"#)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for j in rows {
            out.push(serde_json::from_str(&j)?);
        }
        Ok(out)
    }

    /// The stored rule with this id, if any.
    pub async fn get_rule(&self, id: uuid::Uuid) -> Result<Option<Rule>> {
        let json = sqlx::query_scalar::<_, String>(r#"SELECT json FROM rules WHERE id = ?1"#)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(match json {
            Some(j) => Some(serde_json::from_str(&j)?),
            None => None,
        })
    }

    /// Removes the rule row. Touches no file and no journal entry (journal
    /// rows keep the rule name). Returns false when no rule had this id.
    pub async fn delete_rule(&self, id: uuid::Uuid) -> Result<bool> {
        let res = sqlx::query(r#"DELETE FROM rules WHERE id = ?1"#)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    // Operation History & Statistics
    pub async fn record_operation(
        &self,
        source_path: &str,
        destination_path: &str,
        operation_type: &str,
        rule_name: &str,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<()> {
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
        let (total_operations,): (i64,) =
            sqlx::query_as(&total_query).fetch_one(&self.pool).await?;

        let success_query = format!(
            "SELECT COUNT(*) FROM operation_history {} {}",
            date_filter,
            if date_filter.is_empty() {
                "WHERE"
            } else {
                "AND"
            }
        );
        let success_query = format!("{} status = 'success'", success_query);
        let (successful_operations,): (i64,) =
            sqlx::query_as(&success_query).fetch_one(&self.pool).await?;

        let failed_operations = total_operations - successful_operations;

        // Operation type counts
        let move_query = format!(
            "SELECT COUNT(*) FROM operation_history {} {} operation_type = 'move' AND status = 'success'",
            date_filter,
            if date_filter.is_empty() {
                "WHERE"
            } else {
                "AND"
            }
        );
        let (files_moved,): (i64,) = sqlx::query_as(&move_query).fetch_one(&self.pool).await?;

        let copy_query = format!(
            "SELECT COUNT(*) FROM operation_history {} {} operation_type = 'copy' AND status = 'success'",
            date_filter,
            if date_filter.is_empty() {
                "WHERE"
            } else {
                "AND"
            }
        );
        let (files_copied,): (i64,) = sqlx::query_as(&copy_query).fetch_one(&self.pool).await?;

        // Rules applied count
        let rules_query = format!(
            "SELECT rule_name, COUNT(*) as count FROM operation_history {} {} status = 'success' GROUP BY rule_name",
            date_filter,
            if date_filter.is_empty() {
                "WHERE"
            } else {
                "AND"
            }
        );
        let rules_rows: Vec<(String, i64)> =
            sqlx::query_as(&rules_query).fetch_all(&self.pool).await?;
        let mut rules_applied_count = std::collections::HashMap::new();
        for (rule_name, count) in rules_rows {
            rules_applied_count.insert(rule_name, count as u64);
        }

        // File types organized
        let file_types_query = format!(
            "SELECT LOWER(SUBSTR(source_path, INSTR(source_path, '.') + 1)) as ext, COUNT(*) as count FROM operation_history {} {} status = 'success' AND INSTR(source_path, '.') > 0 GROUP BY ext",
            date_filter,
            if date_filter.is_empty() {
                "WHERE"
            } else {
                "AND"
            }
        );
        let file_types_rows: Vec<(String, i64)> = sqlx::query_as(&file_types_query)
            .fetch_all(&self.pool)
            .await?;
        let mut file_types_organized = std::collections::HashMap::new();
        for (ext, count) in file_types_rows {
            file_types_organized.insert(ext, count as u64);
        }

        // Average operations per day
        let days_query = match days_back {
            Some(days) => format!(
                "SELECT COUNT(DISTINCT DATE(created_at)) FROM operation_history WHERE created_at >= datetime('now', '-{} days')",
                days
            ),
            None => "SELECT COUNT(DISTINCT DATE(created_at)) FROM operation_history".to_string(),
        };
        let (active_days,): (i64,) = sqlx::query_as(&days_query).fetch_one(&self.pool).await?;
        let average_operations_per_day = if active_days > 0 {
            total_operations as f64 / active_days as f64
        } else {
            0.0
        };

        // Last operation date
        let last_op_query = format!(
            "SELECT created_at FROM operation_history {} ORDER BY created_at DESC LIMIT 1",
            date_filter
        );
        let last_operation_date: Option<String> = sqlx::query_scalar(&last_op_query)
            .fetch_optional(&self.pool)
            .await?;

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
        for (
            id,
            source_path,
            destination_path,
            operation_type,
            rule_name,
            status,
            error_message,
            created_at,
        ) in rows
        {
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
        sqlx::query("DELETE FROM operation_history")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn upsert_file_query(f: &FileRow) -> Query<'_, Sqlite, SqliteArguments<'_>> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_database_gets_no_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("valet.sqlite3"))
            .await
            .unwrap();
        assert!(db.list_rules().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn reinsert_rules_moves_them_after_the_others_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("valet.sqlite3"))
            .await
            .unwrap();
        let rule = |n: u128| -> Rule {
            serde_json::from_value(serde_json::json!({
                "id": uuid::Uuid::from_u128(n),
                "name": format!("r{n}"),
                "enabled": true,
                "conditions": [],
                "actions": [{ "type": "report" }],
            }))
            .unwrap()
        };
        for n in 1..=3 {
            db.upsert_rule(&rule(n)).await.unwrap();
        }
        let mut changed = rule(1);
        changed.name = "new".into();
        db.reinsert_rules(&[rule(2), changed]).await.unwrap();
        let got: Vec<(u128, String)> = db
            .list_rules()
            .await
            .unwrap()
            .into_iter()
            .map(|r| (r.id.as_u128(), r.name))
            .collect();
        assert_eq!(got, [(3, "r3".into()), (2, "r2".into()), (1, "new".into())]);
    }

    #[tokio::test]
    async fn get_and_delete_rule_by_id() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("valet.sqlite3"))
            .await
            .unwrap();
        let rule: Rule = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::from_u128(5),
            "name": "r",
            "enabled": true,
            "conditions": [],
            "actions": [{ "type": "report" }],
            "scope": ["{downloads}"],
        }))
        .unwrap();
        let other = uuid::Uuid::from_u128(6);
        assert!(db.get_rule(rule.id).await.unwrap().is_none());
        db.upsert_rule(&rule).await.unwrap();
        let got = db.get_rule(rule.id).await.unwrap().unwrap();
        assert_eq!(got.scope, vec!["{downloads}".to_string()]);
        assert!(!db.delete_rule(other).await.unwrap());
        assert!(db.delete_rule(rule.id).await.unwrap());
        assert!(db.get_rule(rule.id).await.unwrap().is_none());
        assert!(db.list_rules().await.unwrap().is_empty());
        assert!(!db.delete_rule(rule.id).await.unwrap());
    }

    #[tokio::test]
    async fn clearing_history_keeps_the_journal() {
        use crate::journal::{BatchKind, NewOp, OpType};
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("valet.sqlite3"))
            .await
            .unwrap();
        db.record_operation("a", "b", "move", "r", "success", None)
            .await
            .unwrap();
        let j = db.journal();
        let batch = j.create_batch(BatchKind::Apply).await.unwrap();
        j.insert_planned(
            &batch,
            0,
            &NewOp {
                op_type: OpType::Move,
                source: "a".into(),
                dest: Some("b".into()),
                size: Some(1),
                undo_of: None,
            },
        )
        .await
        .unwrap();

        db.clear_operation_history().await.unwrap();
        assert!(db.get_recent_operations(10).await.unwrap().is_empty());
        assert_eq!(j.list_batches(10).await.unwrap().len(), 1);
        assert_eq!(j.list_ops(&batch).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn upsert_files_inserts_then_updates_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("valet.sqlite3"))
            .await
            .unwrap();
        let row = |path: &str, size: i64| FileRow {
            id: FileRow::id_for_path(path),
            path: path.into(),
            size,
            mtime: 1,
            hash_short: Some(format!("h{size}")),
            tags_json: "[]".into(),
        };
        db.upsert_files(&[]).await.unwrap();
        db.upsert_files(&[row("a", 1), row("b", 2)]).await.unwrap();
        db.upsert_files(&[row("a", 10), row("c", 3)]).await.unwrap();
        let rows: Vec<(String, i64, Option<String>)> =
            sqlx::query_as("SELECT path, size, hash_short FROM files ORDER BY path")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(
            rows,
            vec![
                ("a".into(), 10, Some("h10".into())),
                ("b".into(), 2, Some("h2".into())),
                ("c".into(), 3, Some("h3".into())),
            ]
        );
        db.pool.close().await;
    }
}
