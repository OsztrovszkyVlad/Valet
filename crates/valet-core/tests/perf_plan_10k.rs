//! Timing of `planner::plan` on 10,000 files (PLAN.md 2.6: preview of 10k files < 3 s).
//! Run: `cargo test -p valet-core --release --test perf_plan_10k -- --ignored --nocapture`

use std::fs;
use std::time::{Duration, Instant, SystemTime};

use serde_json::json;
use uuid::Uuid;
use valet_core::planner::{PlanContext, plan};
use valet_core::platform::testing::FakePlatform;
use valet_core::rules::Rule;

fn rule(n: u128, name: &str, conditions: serde_json::Value, path: &str) -> Rule {
    serde_json::from_value(json!({
        "id": Uuid::from_u128(n),
        "name": name,
        "enabled": true,
        "conditions": conditions,
        "actions": [{ "type": "moveTo", "params": { "path": path } }],
    }))
    .unwrap()
}

/// Writes 10,000 one-byte files under `root` (100 folders, 8 extensions,
/// every tenth named `factura_*`).
fn fixture(root: &std::path::Path) {
    let exts = ["pdf", "jpg", "png", "docx", "zip", "txt", "mp4", "xlsx"];
    for i in 0..10_000 {
        let dir = root.join(format!("d{:03}", i % 100));
        if i < 100 {
            fs::create_dir_all(&dir).unwrap();
        }
        let name = if i % 10 == 0 {
            format!("factura_{i}.{}", exts[i % exts.len()])
        } else {
            format!("file_{i}.{}", exts[i % exts.len()])
        };
        fs::write(dir.join(name), b"x").unwrap();
    }
}

fn ctx() -> PlanContext {
    PlanContext {
        now: SystemTime::now() + Duration::from_secs(3600),
        home_dir: None,
        env: Box::new(|_| None),
    }
}

fn timed_plan(label: &str, root: &std::path::PathBuf, rules: &[Rule]) -> (usize, Duration) {
    let start = Instant::now();
    let p = plan(
        std::slice::from_ref(root),
        rules,
        &FakePlatform::default(),
        &ctx(),
    );
    let elapsed = start.elapsed();
    println!(
        "plan ({label}): 10000 files, {} ops, {} skipped, {:.3} s",
        p.ops.len(),
        p.skipped.len(),
        elapsed.as_secs_f64()
    );
    (p.ops.len(), elapsed)
}

#[test]
#[ignore = "timing; run in release with --ignored"]
fn plan_10k_files_under_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Downloads");
    fixture(&root);
    let rules = vec![
        rule(
            1,
            "Facturi",
            json!([{ "type": "nameMatches", "value": "(?i)factura|invoice" }]),
            "Facturi/{year}",
        ),
        rule(
            2,
            "Imagini",
            json!([{ "type": "ext", "value": "jpg" }]),
            "Imagini",
        ),
        rule(
            3,
            "PNG",
            json!([{ "type": "ext", "value": "png" }]),
            "Imagini",
        ),
        rule(
            4,
            "Docs",
            json!([{ "type": "ext", "value": "docx" }]),
            "Documente",
        ),
        rule(
            5,
            "Rest",
            json!([{ "type": "sizeLt", "value": 100 }]),
            "Altele/{ext}",
        ),
    ];
    let (ops, elapsed) = timed_plan("1 nameMatches", &root, &rules);
    assert_eq!(ops, 10_000);
    assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
}

#[test]
#[ignore = "timing; run in release with --ignored"]
fn plan_10k_files_three_name_rules_under_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Downloads");
    fixture(&root);
    let rules = vec![
        rule(
            1,
            "Contracte",
            json!([{ "type": "nameMatches", "value": "(?i)contract|acord" }]),
            "Contracte",
        ),
        rule(
            2,
            "Scanari",
            json!([{ "type": "nameMatches", "value": "(?i)^scan_\\d+" }]),
            "Scanari",
        ),
        rule(
            3,
            "Facturi",
            json!([{ "type": "nameMatches", "value": "(?i)factura|invoice" }]),
            "Facturi/{year}",
        ),
        rule(
            4,
            "Rest",
            json!([{ "type": "sizeLt", "value": 100 }]),
            "Altele/{ext}",
        ),
    ];
    let (ops, elapsed) = timed_plan("3 nameMatches", &root, &rules);
    assert_eq!(ops, 10_000);
    assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
}
