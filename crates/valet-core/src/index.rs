//! File index: walks scan roots with the planner's walker and default
//! exclusions and stores size, mtime and a short content hash per file.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use blake3::Hasher;

use crate::{
    model::FileRow,
    planner::{Found, scan},
    platform::Platform,
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
    Ok(format!(
        "{:016x}",
        u128::from_le_bytes(h.as_bytes()[..16].try_into().unwrap())
    ))
}

/// Indexes every file the planner would consider: excluded directories,
/// hidden files, reparse points, files being written and cloud placeholders
/// are not indexed. Placeholders are never opened or hashed.
/// Returns the number of indexed files.
pub async fn index_paths(paths: &[PathBuf], db: &Db, platform: &dyn Platform) -> Result<usize> {
    let mut files = Vec::new();
    scan(paths, platform, SystemTime::now(), |found| {
        if let Found::File(f) = found
            && !f.hidden
        {
            files.push(f);
        }
    });

    for f in &files {
        let hash_short = short_hash(&platform.os_path(&f.path)).ok();
        let path_str = f.path.to_string_lossy().to_string();
        let row = FileRow {
            id: FileRow::id_for_path(&path_str),
            path: path_str,
            size: f.size as i64,
            mtime: unix_secs(f.mtime),
            hash_short,
            tags_json: "[]".into(),
        };
        db.upsert_file(&row).await?;
    }
    Ok(files.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::testing::FakePlatform;
    use std::time::Duration;

    fn old_file(path: &Path, body: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
        let f = fs::File::options().write(true).open(path).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
    }

    #[tokio::test]
    async fn index_skips_placeholders_and_exclusions() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("t.sqlite3")).await.unwrap();
        let root = tmp.path().join("root");
        let normal = root.join("a.txt");
        let placeholder = root.join("cloud.txt");
        old_file(&normal, b"a");
        old_file(&placeholder, b"c");
        old_file(&root.join("node_modules").join("x.txt"), b"x");
        old_file(&root.join(".hidden"), b"h");

        let mut platform = FakePlatform::default();
        platform.placeholders.insert(placeholder.clone());

        let n = index_paths(std::slice::from_ref(&root), &db, &platform)
            .await
            .unwrap();
        assert_eq!(n, 1);
        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT path, hash_short FROM files")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, normal.to_string_lossy());
        assert!(rows[0].1.is_some());
        db.pool.close().await;
    }
}
