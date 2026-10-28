//! Timing of `index::index_paths` on 100,000 files (PLAN.md 2.6: index of
//! 100k files < 60 s). Writes 100k small files (about 400 MB of disk blocks).
//! Measured 2026-10-28 on the dev VM (release): 70.8 s before batching (80e7213),
//! about 10 s after.
//! Run: `cargo test -p valet-core --release --test perf_index_100k -- --ignored --nocapture`

use std::fs::{self, File};
use std::io::Write;
use std::time::{Duration, Instant, SystemTime};

use valet_core::index::index_paths;
use valet_core::platform::testing::FakePlatform;
use valet_core::storage::Db;

const N: usize = 100_000;

#[test]
#[ignore = "release-only perf test"]
fn index_100k_files_under_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Docs");
    // index_paths uses the real clock: files must be older than the
    // being-written window.
    let old = SystemTime::now() - Duration::from_secs(3600);
    let exts = ["pdf", "jpg", "png", "docx", "zip", "txt", "mp4", "xlsx"];
    let t = Instant::now();
    for i in 0..N {
        let dir = root.join(format!("d{:03}", i % 500));
        if i < 500 {
            fs::create_dir_all(&dir).unwrap();
        }
        let mut f = File::create(dir.join(format!("f_{i}.{}", exts[i % exts.len()]))).unwrap();
        f.write_all(format!("{i}").as_bytes()).unwrap();
        f.set_modified(old).unwrap();
    }
    println!("fixture: {N} files in {:.1} s", t.elapsed().as_secs_f64());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (count, elapsed) = rt.block_on(async {
        let db = Db::connect(&tmp.path().join("state").join("valet.sqlite3"))
            .await
            .unwrap();
        let t = Instant::now();
        let count = index_paths(std::slice::from_ref(&root), &db, &FakePlatform::default())
            .await
            .unwrap();
        let elapsed = t.elapsed();
        db.pool.close().await;
        (count, elapsed)
    });
    println!(
        "index_paths: {count} files in {:.2} s ({:.0} files/s)",
        elapsed.as_secs_f64(),
        count as f64 / elapsed.as_secs_f64()
    );
    assert_eq!(count, N);
    assert!(elapsed < Duration::from_secs(60), "{elapsed:?}");
}
