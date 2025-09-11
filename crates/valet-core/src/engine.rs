use anyhow::{Context, Result};
use blake3::Hasher;
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use walkdir::WalkDir;

use crate::{
  matcher::{matching_rules, FileFacts},
  model::{DryRunAction, DryRunPlan, FileRow, Op},
  rules::Action,
  storage::Db,
};

fn unix_secs(t: SystemTime) -> i64 {
  t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

fn short_hash(path: &Path) -> Result<String> {
  let mut hasher = Hasher::new();
  let mut file = fs::File::open(path)?;
  std::io::copy(&mut file, &mut hasher)?;
  let h = hasher.finalize();
  Ok(format!("{:016x}", u128::from_le_bytes(h.as_bytes()[..16].try_into().unwrap())))
}

pub async fn index_paths(paths: &[PathBuf], db: &Db) -> Result<usize> {
  let mut count = 0usize;

  for root in paths {
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
      if !entry.file_type().is_file() { continue; }
      let path = entry.path();

      let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
      let size = meta.len() as i64;
      let mtime = meta.modified().ok().map(unix_secs).unwrap_or(0);

      // Hash can be expensive; compute it but you can later make this optional or sampled.
      let hash_short = short_hash(path).ok();

      let path_str = path.to_string_lossy().to_string();
      let row = FileRow {
        id: FileRow::id_for_path(&path_str),
        path: path_str,
        size,
        mtime,
        hash_short,
        tags_json: "[]".into(),
      };

      db.upsert_file(&row).await?;
      count += 1;
    }
  }
  Ok(count)
}

pub async fn dry_run_for_paths(paths: &[PathBuf], db: &Db) -> Result<DryRunPlan> {
  let rules = db.list_rules().await?;
  let mut plan = DryRunPlan::default();

  for root in paths {
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
      if !entry.file_type().is_file() { continue; }
      let p = entry.path();
      let meta = fs::metadata(p)?;
      let facts = FileFacts { path: &p.to_string_lossy(), size: meta.len() };

      for rule in matching_rules(&rules, &facts) {
        for act in &rule.actions {
          if let Some(op) = map_action(act) {
            plan.actions.push(DryRunAction {
              file_path: facts.path.to_string(),
              op,
              rule_id: rule.id,
              rule_name: rule.name.clone(),
            });
          }
        }
      }
    }
  }
  Ok(plan)
}

fn map_action(a: &Action) -> Option<Op> {
  match a.r#type.as_str() {
    "moveTo" => Some(Op::MoveTo { path: a.params.get("path")?.as_str()?.to_string() }),
    "copyTo" => Some(Op::CopyTo { path: a.params.get("path")?.as_str()?.to_string() }),
    "tag"    => {
      let tags = a.params.get("tags")?.as_array()?
        .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
      Some(Op::Tag { tags })
    }
    "rename" => Some(Op::Rename { pattern: a.params.get("pattern")?.as_str()?.to_string() }),
    "quarantine" => Some(Op::Quarantine),
    _ => None,
  }
}