use anyhow::Result;
use sqlx::{
  sqlite::{SqliteConnectOptions, SqlitePoolOptions},
  SqlitePool,
};
use std::path::Path;
use crate::rules::Rule;
use crate::model::FileRow;

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
}
