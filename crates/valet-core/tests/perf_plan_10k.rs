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

#[test]
#[ignore = "timing; run in release with --ignored"]
fn plan_10k_files_under_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Downloads");
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
    let ctx = PlanContext {
        now: SystemTime::now() + Duration::from_secs(3600),
        home_dir: None,
        env: Box::new(|_| None),
    };

    let start = Instant::now();
    let p = plan(
        std::slice::from_ref(&root),
        &rules,
        &FakePlatform::default(),
        &ctx,
    );
    let elapsed = start.elapsed();
    println!(
        "plan: 10000 files, {} ops, {} skipped, {:.3} s",
        p.ops.len(),
        p.skipped.len(),
        elapsed.as_secs_f64()
    );
    assert_eq!(p.ops.len(), 10_000);
    assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
}
