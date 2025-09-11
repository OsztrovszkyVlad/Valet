use anyhow::Result;
use std::path::PathBuf;
use valet_core::{config, storage::Db, engine::index_paths};

#[tokio::main]
async fn main() -> Result<()> {
  let cfg_dir = config::config_dir()?;
  let db_path = cfg_dir.join("valet.sqlite3");
  let db = Db::connect(&db_path).await?;

  // Change this to any path you want to index (Downloads as a default)
  let paths = vec![dirs::download_dir().unwrap_or(PathBuf::from("~/Downloads"))];

  let n = index_paths(&paths, &db).await?;
  println!("Indexed {} files.", n);
  Ok(())
}
