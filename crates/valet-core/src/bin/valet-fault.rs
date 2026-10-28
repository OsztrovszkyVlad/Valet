//! Test-support driver for the fault-injection harness. Not shipped
//! (built only with `--features test-support`).
//!
//! ```text
//! valet-fault apply   <db> <rules.json> <root>...   plan + execute one batch
//! valet-fault recover <db>                          startup recovery
//! valet-fault undo    <db> <batch_id> [--force]     undo a batch
//! ```
//!
//! Uses `FakePlatform` with its Recycle Bin at `<db dir>/trash`.
//! `VALET_FORCE_CROSS_VOLUME=1` makes every move take the copy + verify +
//! trash path; `VALET_FAULT_SLOW_MS=<n>` sleeps after each journal state
//! change so the harness can kill the process mid-batch. Prints one JSON
//! report on stdout; exit code 2 on usage errors, 1 on other errors.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::json;
use valet_core::{
    executor,
    planner::{self, PlanContext},
    platform::testing::FakePlatform,
    rules::Rule,
    storage::Db,
};

const USAGE: &str = "usage: valet-fault apply <db> <rules.json> <root>... | recover <db> | undo <db> <batch_id> [--force]";

fn platform(db: &Path) -> FakePlatform {
    let dir = db.parent().unwrap_or_else(|| Path::new("."));
    let mut p = FakePlatform::with_trash_dir(dir.join("trash"));
    if std::env::var("VALET_FORCE_CROSS_VOLUME").is_ok_and(|v| v == "1") {
        p.same_volume = false;
    }
    p
}

async fn run(args: &[String]) -> Result<serde_json::Value> {
    match args {
        [cmd, db, rules, roots @ ..] if cmd == "apply" && !roots.is_empty() => {
            let db_path = PathBuf::from(db);
            let rules: Vec<Rule> = serde_json::from_str(
                &std::fs::read_to_string(rules).with_context(|| format!("reading {rules}"))?,
            )?;
            let roots: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
            let p = platform(&db_path);
            let plan = planner::plan(&roots, &rules, &p, &PlanContext::from_system());
            let db = Db::connect(&db_path).await?;
            let report = executor::execute(&plan.ops, &db.journal(), &p).await?;
            Ok(json!({
                "batch_id": report.batch_id,
                "planned": plan.ops.len(),
                "skipped": plan.skipped.len(),
                "done": report.done_count(),
                "failed": report.failed_count(),
                "results": report.results,
            }))
        }
        [cmd, db] if cmd == "recover" => {
            let db_path = PathBuf::from(db);
            let db = Db::connect(&db_path).await?;
            let report = executor::recover(&db.journal(), &platform(&db_path)).await?;
            Ok(serde_json::to_value(report)?)
        }
        [cmd, db, batch, rest @ ..] if cmd == "undo" && rest.len() <= 1 => {
            let force = match rest {
                [] => false,
                [f] if f == "--force" => true,
                _ => bail!("{USAGE}"),
            };
            let db_path = PathBuf::from(db);
            let db = Db::connect(&db_path).await?;
            let report =
                executor::undo_batch(batch, &db.journal(), &platform(&db_path), force).await?;
            Ok(serde_json::to_value(report)?)
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args).await {
        Ok(v) => println!("{v}"),
        Err(e) => {
            eprintln!("valet-fault: {e:#}");
            std::process::exit(1);
        }
    }
}
