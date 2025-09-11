use anyhow::Result;
use std::path::PathBuf;
use valet_core::{config, storage::Db, engine::dry_run_for_paths};

#[tokio::main]
async fn main() -> Result<()> {
  let cfg_dir = config::config_dir()?;
  let db_path = cfg_dir.join("valet.sqlite3");
  let db = Db::connect(&db_path).await?;

  // Choose a folder to preview actions for
  let target = dirs::download_dir().unwrap_or(PathBuf::from("~/Downloads"));
  let plan = dry_run_for_paths(&[target.clone()], &db).await?;

  println!("Dry run results for {} action(s):", plan.actions.len());
  for a in plan.actions {
    println!("- [{}] {} -> {:?}", a.rule_name, a.file_path, a.op);
  }
  Ok(())
}
