use anyhow::Result;
use std::path::PathBuf;
use valet_core::{config, engine::dry_run_for_paths, storage::Db};

#[tokio::main]
async fn main() -> Result<()> {
    let cfg_dir = config::config_dir()?;
    let db_path = cfg_dir.join("valet.sqlite3");
    let db = Db::connect(&db_path).await?;

    // Choose a folder to preview actions for
    let target = directories::UserDirs::new()
        .and_then(|u| u.download_dir().map(|p| p.to_path_buf()))
        .unwrap_or(PathBuf::from("~/Downloads"));
    let plan = dry_run_for_paths(std::slice::from_ref(&target), &db).await?;

    println!("Dry run results for {} action(s):", plan.actions.len());
    for a in plan.actions {
        println!("- [{}] {} -> {:?}", a.rule_name, a.file_path, a.op);
    }
    Ok(())
}
