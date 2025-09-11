use anyhow::Result;
use valet_core::{config, storage::Db};

#[tokio::main]
async fn main() -> Result<()> {
  let cfg = config::load()?;
  println!("Loaded config: {:?}", cfg);

  let db_path = config::config_dir()?.join("valet.sqlite3");
  let db = Db::connect(&db_path).await?;
  db.load_sample_rules_if_empty().await?;
  let rules = db.list_rules().await?;
  println!("Rules loaded: {}", rules.len());
  Ok(())
}
