//! Criterion bench: `planner::plan` over a 10,000-file fixture with a rule set
//! close to what a user builds from the presets (PLAN.md 2.6: preview of 10k
//! files < 3 s). The pass/fail budget check stays in `tests/perf_plan_10k.rs`.
//!
//! Run: `cargo bench -p valet-core --features test-support --bench plan_10k -- --quick`

use std::collections::BTreeMap;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::json;
use uuid::Uuid;
use valet_core::planner::{PlanContext, plan};
use valet_core::platform::testing::FakePlatform;
use valet_core::rules::Rule;

const FILES: usize = 10_000;

/// Extensions cycled over the fixture; `iso` and `bin` match no rule below
/// except the size catch-all, `json` matches none (left in place).
const EXTS: [&str; 16] = [
    "pdf", "jpg", "png", "docx", "zip", "txt", "mp4", "xlsx", "exe", "msi", "heic", "mp3", "rar",
    "csv", "iso", "json",
];

fn rule(n: u128, name: &str, conditions: serde_json::Value, path: &str) -> Rule {
    serde_json::from_value(json!({
        "id": Uuid::from_u128(n),
        "name": name,
        "enabled": true,
        "conditions": conditions,
        "actions": [{ "type": "moveTo", "params": { "path": path } }],
    }))
    .expect("bench rule parses")
}

/// One nameMatches rule, one typeGroup rule, the rest by extension list,
/// size and age, in the order a user would rank them.
fn rules() -> Vec<Rule> {
    vec![
        rule(
            1,
            "Facturi",
            json!([{ "type": "nameMatches", "value": "(?i)factur[aiă]|invoice|chitan[tț][aă]" }]),
            "Facturi/{year}",
        ),
        rule(
            2,
            "Imagini",
            json!([{ "type": "typeGroup", "value": "image" }]),
            "Imagini/{year}-{month}",
        ),
        rule(
            3,
            "Kituri de instalare",
            json!([{ "type": "extIn", "value": ["exe", "msi", "msix"] }]),
            "Kituri",
        ),
        rule(
            4,
            "Documente",
            json!([{ "type": "extIn", "value": ["pdf", "docx", "xlsx", "txt", "csv"] }]),
            "Documente/{ext}",
        ),
        rule(
            5,
            "Media",
            json!([{ "type": "extIn", "value": ["mp4", "mp3"] }]),
            "Media",
        ),
        rule(
            6,
            "Arhive mari",
            json!([
                { "type": "extIn", "value": ["zip", "rar", "iso"] },
                { "type": "sizeGt", "value": 1000 }
            ]),
            "Arhive",
        ),
    ]
}

/// Writes `FILES` files under `root` in 100 folders, sizes 1..4096 bytes,
/// every tenth named `factura_*`, every 25th `Invoice-*`.
fn fixture(root: &Path) {
    for i in 0..FILES {
        let dir = root.join(format!("d{:03}", i % 100));
        if i < 100 {
            fs::create_dir_all(&dir).expect("fixture dir");
        }
        let ext = EXTS[i % EXTS.len()];
        let name = if i % 10 == 0 {
            format!("factura_{i}.{ext}")
        } else if i % 25 == 0 {
            format!("Invoice-{i}.{ext}")
        } else {
            format!("file_{i}.{ext}")
        };
        let size = 1 + (i * 37) % 4096;
        fs::write(dir.join(name), vec![b'x'; size]).expect("fixture file");
    }
}

fn ctx() -> PlanContext {
    PlanContext {
        // An hour ahead so no fixture file counts as "being written".
        now: SystemTime::now() + Duration::from_secs(3600),
        ..PlanContext::from_system()
    }
}

fn bench_plan_10k(c: &mut Criterion) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root: PathBuf = tmp.path().join("Downloads");
    fixture(&root);
    let rules = rules();
    let platform = FakePlatform::default();
    let ctx = ctx();
    let roots = std::slice::from_ref(&root);

    let p = plan(roots, &rules, &platform, &ctx);
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    for s in &p.skipped {
        *reasons.entry(format!("{:?}", s.reason)).or_default() += 1;
    }
    println!(
        "plan_10k fixture: {FILES} files, {} rules, {} ops, {} skipped {reasons:?}",
        rules.len(),
        p.ops.len(),
        p.skipped.len()
    );
    assert!(
        p.ops.len() > FILES / 2,
        "rule set should move most of the fixture, got {} ops",
        p.ops.len()
    );

    let mut group = c.benchmark_group("plan");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.bench_function("plan_10k_files_6_rules", |b| {
        b.iter(|| black_box(plan(black_box(roots), &rules, &platform, &ctx)))
    });
    group.finish();
}

criterion_group!(benches, bench_plan_10k);
criterion_main!(benches);
