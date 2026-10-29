use crate::model::FileRow;
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
    /// of their group (user or preset) in `list_rules` order. Other rules are
    /// not touched.
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

    /// Every rule in evaluation order (the planner gives a file to the first
    /// matching rule): the user's own rules first, in insertion order, then
    /// preset rules ([`Rule::is_preset`]) in insertion order, which
    /// `presets::install` keeps equal to `evalOrder`. So a rule the user
    /// writes after installing presets still wins over an overlapping preset.
    /// Rows are not renumbered; the order is applied when listing.
    pub async fn list_rules(&self) -> Result<Vec<Rule>> {
        let rows = sqlx::query_scalar::<_, String>(r#"SELECT json FROM rules ORDER BY rowid"#)
            .fetch_all(&self.pool)
            .await?;
        let mut out: Vec<Rule> = Vec::with_capacity(rows.len());
        for j in rows {
            out.push(serde_json::from_str(&j)?);
        }
        // Stable: keeps insertion order within each group.
        out.sort_by_key(Rule::is_preset);
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
    async fn list_rules_puts_user_rules_before_preset_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("valet.sqlite3"))
            .await
            .unwrap();
        let rule = |n: u128, preset: Option<&str>| -> Rule {
            let options = match preset {
                Some(p) => serde_json::json!({ "presetId": p }),
                None => serde_json::json!({}),
            };
            serde_json::from_value(serde_json::json!({
                "id": uuid::Uuid::from_u128(n),
                "name": format!("r{n}"),
                "enabled": true,
                "conditions": [],
                "actions": [{ "type": "report" }],
                "options": options,
            }))
            .unwrap()
        };
        // Insertion order: preset, user, preset, user, preset.
        for (n, preset) in [
            (1, Some("invoices")),
            (2, None),
            (3, Some("screenshots")),
            (4, None),
            (5, Some("invoices")),
        ] {
            db.upsert_rule(&rule(n, preset)).await.unwrap();
        }
        // Editing a rule keeps its place.
        let mut edited = rule(2, None);
        edited.name = "edited".into();
        db.upsert_rule(&edited).await.unwrap();
        let got: Vec<u128> = db
            .list_rules()
            .await
            .unwrap()
            .iter()
            .map(|r| r.id.as_u128())
            .collect();
        assert_eq!(got, [2, 4, 1, 3, 5]);
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

    /// A database at migration 0004 with statistics rows: 0005 drops the
    /// `operation_history` table and its indexes, the journal keeps its rows.
    #[tokio::test]
    async fn migration_0005_drops_operation_history_and_keeps_the_journal() {
        use crate::journal::{BatchKind, Journal, NewOp, OpState, OpType};
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("v4.sqlite3");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let mut v4 = sqlx::migrate!("./migrations");
        v4.migrations = std::borrow::Cow::Owned(
            v4.migrations
                .iter()
                .filter(|m| m.version <= 4)
                .cloned()
                .collect(),
        );
        v4.run(&pool).await.unwrap();
        for n in 0..3 {
            sqlx::query(
                "INSERT INTO operation_history (source_path, destination_path, operation_type, rule_name, status)
                 VALUES (?1, 'b', 'move', 'r', 'success')",
            )
            .bind(format!("a{n}"))
            .execute(&pool)
            .await
            .unwrap();
        }
        let j = Journal::new(pool.clone());
        let batch = j.create_batch(BatchKind::Apply).await.unwrap();
        let op = NewOp {
            op_type: OpType::Move,
            source: "a".into(),
            dest: Some("b".into()),
            size: Some(1),
            undo_of: None,
        };
        let id = j.insert_planned(&batch, 0, &op).await.unwrap();
        j.start(id, Some("h")).await.unwrap();
        j.mark_done(id, Some("h")).await.unwrap();
        pool.close().await;

        let db = Db::connect(&path).await.unwrap();
        let left: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE name LIKE '%operation_history%'",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert!(left.is_empty(), "{left:?}");
        let j = db.journal();
        assert_eq!(j.list_batches(10).await.unwrap().len(), 1);
        let ops = j.list_ops(&batch).await.unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].id, id);
        assert_eq!(ops[0].state, OpState::Done);
        assert_eq!(ops[0].hash_after.as_deref(), Some("h"));
        // A new database never has the table.
        let fresh = Db::connect(&tmp.path().join("new.sqlite3")).await.unwrap();
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'operation_history'",
        )
        .fetch_one(&fresh.pool)
        .await
        .unwrap();
        assert_eq!(n, 0);
        db.pool.close().await;
        fresh.pool.close().await;
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
